use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use drive_study_core::{
    answer_grammar, choices_readout, clip_key, is_due, parse_voice, review_card, ExamId, ItemKind,
    ParseOptions, ReviewOutcome, SessionItem, SessionMode, SessionPlan, SrsCard, SrsMeta,
    VoiceCommand, VOICE_ID,
};
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub plan: String,
    pub answer_mode: String,
    pub voice_source: String,
    pub rate: f64,
    pub pause_seconds: u32,
    #[serde(default = "default_answer_seconds")]
    pub answer_seconds: u32,
    #[serde(default = "default_explain_correct")]
    pub explain_correct: String,
    pub voice_uri: Option<String>,
    pub seen_intro: bool,
    pub completed_lessons: Vec<String>,
}

fn default_answer_seconds() -> u32 {
    10
}

fn default_explain_correct() -> String {
    "short".into()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            plan: "BOTH".into(),
            answer_mode: "spoken".into(),
            voice_source: "downloaded".into(),
            rate: 1.0,
            pause_seconds: 6,
            answer_seconds: default_answer_seconds(),
            explain_correct: default_explain_correct(),
            voice_uri: None,
            seen_intro: false,
            completed_lessons: Vec::new(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptRecord {
    pub item_id: String,
    pub kind: ItemKind,
    pub exam: ExamId,
    pub section: String,
    pub topic: Option<String>,
    pub correct: bool,
    pub guessed: bool,
    pub mode: String,
    pub at: i64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DayRecord {
    pub date: String,
    pub count: u32,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonProgress {
    pub lesson_id: String,
    pub status: String,
    pub index: u32,
    pub total: u32,
    pub updated_at: i64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressFile {
    pub version: u32,
    pub exported_at: String,
    #[serde(default)]
    pub profile_id: String,
    #[serde(default)]
    pub profile_name: String,
    pub settings: Settings,
    pub srs: Vec<SrsCard>,
    pub attempts: Vec<AttemptRecord>,
    pub days: Vec<DayRecord>,
    #[serde(default)]
    pub lessons: Vec<LessonProgress>,
}

#[derive(Clone)]
pub struct PlayerModel {
    pub title: String,
    pub index: usize,
    pub total: usize,
    pub kicker: String,
    pub prompt: String,
    pub choices: Vec<(String, String)>,
    pub answer_key: Option<String>,
    pub selected: Option<String>,
    pub explanation: String,
    pub status: String,
    pub source: String,
    pub correct: u32,
    pub wrong: u32,
    pub guessing: bool,
    pub window_left: Option<u32>,
    pub explain_style: String,
    pub sample: bool,
    pub yearly: bool,
    pub card_mode: bool,
    pub remaining: Option<u32>,
    pub paused: bool,
    pub listening: bool,
    pub listen_cue: String,
    pub voice_fallback: bool,
}

impl Default for PlayerModel {
    fn default() -> Self {
        Self {
            title: "Drive Study".into(),
            index: 0,
            total: 0,
            kicker: String::new(),
            prompt: "Starting the session.".into(),
            choices: Vec::new(),
            answer_key: None,
            selected: None,
            explanation: String::new(),
            status: String::new(),
            source: "Phone voice".into(),
            correct: 0,
            wrong: 0,
            guessing: false,
            window_left: None,
            explain_style: String::new(),
            sample: false,
            yearly: false,
            card_mode: false,
            remaining: None,
            paused: false,
            listening: false,
            listen_cue: String::new(),
            voice_fallback: false,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Act {
    Answer(String),
    Repeat,
    Skip,
    Explain,
    DontKnow,
    Pause,
    Stop,
    Know,
    Guess,
    MoreTime,
    Flip,
}

#[derive(Clone)]
pub struct Control {
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    generation: Arc<AtomicU32>,
    pending: Arc<Mutex<Option<Act>>>,
    guessing: Arc<AtomicBool>,
    user_paused: Arc<AtomicBool>,
    voice_epoch: Arc<AtomicU32>,
}

impl Control {
    pub fn new() -> Self {
        Self {
            stop: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
            generation: Arc::new(AtomicU32::new(0)),
            pending: Arc::new(Mutex::new(None)),
            guessing: Arc::new(AtomicBool::new(false)),
            user_paused: Arc::new(AtomicBool::new(false)),
            voice_epoch: Arc::new(AtomicU32::new(0)),
        }
    }

    pub fn user_paused(&self) -> bool {
        self.user_paused.load(Ordering::Relaxed)
    }

    pub fn voice_epoch(&self) -> u32 {
        self.voice_epoch.load(Ordering::Relaxed)
    }

    pub fn retune(&self) {
        self.voice_epoch.fetch_add(1, Ordering::Relaxed);
        retune_output();
    }

    /// Screen lock and the car's pause key. Coming back to the page resumes this.
    /// The on-screen Pause button and the word "pause" stay paused.
    pub fn system_pause(&self) {
        if self.user_paused.load(Ordering::Relaxed) || media_quiet() {
            return;
        }
        self.paused.store(true, Ordering::Relaxed);
        pause_audio();
    }

    pub fn interrupt(&self, act: Act) {
        if matches!(act, Act::MoreTime) {
            let mut slot = self.pending.lock().expect("pending");
            if slot.is_none() {
                *slot = Some(Act::MoreTime);
            }
            return;
        }
        if matches!(act, Act::Pause) {
            self.user_paused.store(true, Ordering::Relaxed);
            self.paused.store(true, Ordering::Relaxed);
            pause_audio();
            return;
        }
        if matches!(act, Act::Stop) {
            self.stop.store(true, Ordering::Relaxed);
        }
        *self.pending.lock().expect("pending") = Some(act);
        self.generation.fetch_add(1, Ordering::Relaxed);
        stop_audio();
    }

    pub fn resume(&self) {
        self.user_paused.store(false, Ordering::Relaxed);
        self.paused.store(false, Ordering::Relaxed);
        resume_audio();
        revive_output();
    }

    pub fn set_guessing(&self, value: bool) {
        self.guessing.store(value, Ordering::Relaxed);
    }

    fn has_pending(&self) -> bool {
        self.pending.lock().expect("pending").is_some()
    }

    pub fn halt(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Relaxed);
        stop_audio();
    }

    fn take(&self) -> Option<Act> {
        self.pending.lock().expect("pending").take()
    }

    fn live(&self, generation: u32) -> bool {
        !self.stop.load(Ordering::Relaxed) && self.generation.load(Ordering::Relaxed) == generation
    }
}

thread_local! {
    static KEEPER: std::cell::RefCell<Option<web_sys::HtmlAudioElement>> = const { std::cell::RefCell::new(None) };
    static CURRENT: std::cell::RefCell<Option<web_sys::HtmlAudioElement>> = const { std::cell::RefCell::new(None) };
}

const SILENCE: &str =
    "data:audio/wav;base64,UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQAAAAA=";

thread_local! {
    static SYNTH_OURS: Cell<bool> = const { Cell::new(false) };
    static SYNTH_HELD: Cell<bool> = const { Cell::new(false) };
    static CLIP_ACTIVE: Cell<bool> = const { Cell::new(false) };
    static UNLOCKED: Cell<bool> = const { Cell::new(false) };
    static VOICES_WAITED: Cell<bool> = const { Cell::new(false) };
    static PAGE_HIDDEN: Cell<bool> = const { Cell::new(false) };
    static MEDIA_QUIET_UNTIL: Cell<f64> = const { Cell::new(0.0) };
    static WATCHING: Cell<bool> = const { Cell::new(false) };
    static SESSION: RefCell<Option<Control>> = const { RefCell::new(None) };
}

fn synth_ours() -> bool {
    SYNTH_OURS.with(Cell::get)
}

fn set_synth_ours(value: bool) {
    SYNTH_OURS.with(|slot| slot.set(value));
}

fn media_quiet() -> bool {
    MEDIA_QUIET_UNTIL.with(|slot| js_sys::Date::now() < slot.get())
}

fn quiet_media_keys(ms: f64) {
    let until = js_sys::Date::now() + ms;
    MEDIA_QUIET_UNTIL.with(|slot| {
        if until > slot.get() {
            slot.set(until);
        }
    });
}

fn page_is_hidden() -> bool {
    PAGE_HIDDEN.with(Cell::get)
}

fn document_hidden() -> bool {
    web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| {
            js_sys::Reflect::get(&document, &JsValue::from_str("hidden"))
                .ok()
                .and_then(|value| value.as_bool())
        })
        .unwrap_or(false)
}

pub fn arm_audio() {
    quiet_media_keys(900.0);
    let existing = KEEPER.with(|slot| slot.borrow().clone());
    if let Some(audio) = existing {
        if audio.paused() {
            let _ = audio.play();
        }
    } else if let Ok(audio) = web_sys::HtmlAudioElement::new_with_src(SILENCE) {
        audio.set_loop(true);
        let _ = audio.set_volume(0.05);
        let _ = audio.play();
        KEEPER.with(|slot| *slot.borrow_mut() = Some(audio));
    }
    // One silent utterance inside the tap unlocks speechSynthesis on iOS.
    // Repeating it on later taps queues a ghost utterance and the next line plays twice.
    if UNLOCKED.with(Cell::get) {
        return;
    }
    if let Some(synth) = synthesis() {
        if let Ok(utterance) = web_sys::SpeechSynthesisUtterance::new_with_text(" ") {
            utterance.set_volume(0.0);
            synth.speak(&utterance);
            UNLOCKED.with(|slot| slot.set(true));
        }
    }
}

fn pause_audio() {
    if CLIP_ACTIVE.with(Cell::get) {
        CURRENT.with(|slot| {
            if let Some(audio) = slot.borrow().as_ref() {
                let _ = audio.pause();
            }
        });
    }
    // `speaking` stays true after a line has finished on iOS and Chrome.
    // Pausing on that flag pauses the next line before it starts.
    if synth_ours() {
        if let Some(synth) = synthesis() {
            let _ = synth.pause();
            SYNTH_HELD.with(|slot| slot.set(true));
        }
    }
}

fn resume_audio() {
    if CLIP_ACTIVE.with(Cell::get) {
        CURRENT.with(|slot| {
            if let Some(audio) = slot.borrow().as_ref() {
                if audio.paused() && !audio.ended() {
                    let _ = audio.play();
                }
            }
        });
    }
    if SYNTH_HELD.with(Cell::get) {
        if let Some(synth) = synthesis() {
            let _ = synth.resume();
        }
        SYNTH_HELD.with(|slot| slot.set(false));
    }
}

fn stop_audio() {
    CLIP_ACTIVE.with(|slot| slot.set(false));
    CURRENT.with(|slot| {
        if let Some(audio) = slot.borrow_mut().take() {
            silence_element(&audio);
        }
    });
    if let Some(synth) = synthesis() {
        if synth_ours() || synth.paused() {
            // cancel() then an immediate speak() makes iOS play the line twice.
            // Resume so a paused queue does not swallow the next line.
            synth.cancel();
            let _ = synth.resume();
        }
    }
    set_synth_ours(false);
    SYNTH_HELD.with(|slot| slot.set(false));
}

fn retune_output() {
    CLIP_ACTIVE.with(|slot| slot.set(false));
    CURRENT.with(|slot| {
        if let Some(audio) = slot.borrow().as_ref() {
            silence_element(audio);
        }
    });
    if synth_ours() {
        if let Some(synth) = synthesis() {
            synth.cancel();
            let _ = synth.resume();
        }
        set_synth_ours(false);
    }
}

fn silence_element(audio: &web_sys::HtmlAudioElement) {
    let _ = audio.pause();
    audio.set_src("");
    let _ = audio.load();
}

fn revive_output() {
    arm_audio();
    resume_audio();
    if let Some(synth) = synthesis() {
        if synth.paused() {
            let _ = synth.resume();
            SYNTH_HELD.with(|slot| slot.set(false));
        }
    }
}

fn install_session_watch(control: Control) {
    SESSION.with(|slot| *slot.borrow_mut() = Some(control));
    if WATCHING.with(Cell::get) {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    WATCHING.with(|slot| slot.set(true));
    let handler = Closure::wrap(Box::new(|| {
        PAGE_HIDDEN.with(|slot| slot.set(document_hidden()));
        if document_hidden() {
            return;
        }
        let Some(control) = SESSION.with(|slot| slot.borrow().clone()) else {
            return;
        };
        if control.stop.load(Ordering::Relaxed) {
            return;
        }
        acquire_wake_lock();
        if control.paused.load(Ordering::Relaxed) && !control.user_paused() {
            control.paused.store(false, Ordering::Relaxed);
            revive_output();
        } else if !control.user_paused() {
            revive_output();
        }
    }) as Box<dyn FnMut()>);
    for name in ["pageshow", "focus", "orientationchange"] {
        let _ = window.add_event_listener_with_callback(name, handler.as_ref().unchecked_ref());
    }
    if let Some(document) = window.document() {
        let _ = document
            .add_event_listener_with_callback("visibilitychange", handler.as_ref().unchecked_ref());
    }
    handler.forget();
}

pub fn install_media_session(control: Control) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(session) = js_sys::Reflect::get(&window.navigator(), &JsValue::from_str("mediaSession"))
    else {
        return;
    };
    let Ok(setter) = js_sys::Reflect::get(&session, &JsValue::from_str("setActionHandler"))
        .and_then(|value| value.dyn_into::<js_sys::Function>())
    else {
        return;
    };
    for name in ["play", "pause", "nexttrack", "previoustrack"] {
        let control = control.clone();
        let name_owned = name.to_string();
        let callback = Closure::wrap(Box::new(move || match name_owned.as_str() {
            "play" => control.resume(),
            "pause" => control.system_pause(),
            "nexttrack" => control.interrupt(Act::Skip),
            _ => control.interrupt(Act::Repeat),
        }) as Box<dyn FnMut()>);
        let _ = setter.call2(
            &session,
            &JsValue::from_str(name),
            callback.as_ref().unchecked_ref(),
        );
        callback.forget();
    }
}

pub fn acquire_wake_lock() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(lock) = js_sys::Reflect::get(&window.navigator(), &JsValue::from_str("wakeLock")) else {
        return;
    };
    let Ok(request) = js_sys::Reflect::get(&lock, &JsValue::from_str("request"))
        .and_then(|value| value.dyn_into::<js_sys::Function>())
    else {
        return;
    };
    let _ = request.call1(&lock, &JsValue::from_str("screen"));
}

fn synthesis() -> Option<web_sys::SpeechSynthesis> {
    web_sys::window()?.speech_synthesis().ok()
}

pub async fn run_session(
    plan: SessionPlan,
    model: RwSignal<PlayerModel>,
    control: Control,
    settings: RwSignal<Settings>,
    cards: RwSignal<Vec<SrsCard>>,
    attempts: RwSignal<Vec<AttemptRecord>>,
    days: RwSignal<Vec<String>>,
    manifest: drive_study_core::AudioManifest,
    summary: RwSignal<Option<Summary>>,
) {
    arm_audio();
    install_media_session(control.clone());
    install_session_watch(control.clone());
    acquire_wake_lock();
    let mut index = plan.start_index.min(plan.items.len());
    let mut correct = 0u32;
    let mut wrong = 0u32;
    let mut skipped = 0u32;
    let mut responses: Vec<(String, bool)> = Vec::new();
    let total = plan.items.len();
    let mut remaining = plan.limit_seconds;
    while index < plan.items.len() && !control.stop.load(Ordering::Relaxed) {
        if let Some(left) = remaining {
            if left == 0 {
                break;
            }
        }
        if let Some(lesson_id) = &plan.lesson_id {
            progress::note_lesson(lesson_id, index, plan.items.len(), false);
            progress::flush_progress().await;
        }
        let again = present(
            &plan,
            index,
            total,
            &mut correct,
            &mut wrong,
            &mut skipped,
            &mut responses,
            &control,
            settings,
            cards,
            attempts,
            days,
            &manifest,
            model,
        )
        .await;
        if control.stop.load(Ordering::Relaxed) {
            break;
        }
        if !again {
            index += 1;
        }
        if let Some(left) = remaining.as_mut() {
            *left = left.saturating_sub(1);
        }
    }
    if plan.timed && remaining == Some(0) {
        for item in plan.items.iter().skip(index) {
            if let SessionItem::Question(question) = item {
                responses.push((question.section.clone(), false));
                wrong += 1;
            }
        }
    }
    let graded = correct + wrong;
    let percent = if graded == 0 {
        0.0
    } else {
        (correct as f64 / graded as f64 * 1000.0).round() / 10.0
    };
    let passed = plan.pass_percent.and_then(|mark| {
        if responses.is_empty() {
            None
        } else {
            let ratio = correct as f64 / graded.max(1) as f64;
            Some(ratio + 1e-12 >= mark / 100.0)
        }
    });
    if let Some(lesson_id) = &plan.lesson_id {
        let done = index >= plan.items.len() && !plan.items.is_empty();
        progress::note_lesson(lesson_id, index, plan.items.len(), done);
        progress::flush_progress().await;
    }
    summary.set(Some(Summary {
        title: plan.title,
        correct,
        wrong,
        skipped,
        percent,
        passed,
    }));
    if let Some(window) = web_sys::window() {
        let hash = window.location().hash().unwrap_or_default();
        if hash.contains("/play") {
            let _ = window.location().set_hash("#/summary");
        }
    }
}

#[derive(Clone)]
pub struct Summary {
    pub title: String,
    pub correct: u32,
    pub wrong: u32,
    pub skipped: u32,
    pub percent: f64,
    pub passed: Option<bool>,
}

async fn present(
    plan: &SessionPlan,
    index: usize,
    total: usize,
    correct: &mut u32,
    wrong: &mut u32,
    skipped: &mut u32,
    responses: &mut Vec<(String, bool)>,
    control: &Control,
    settings: RwSignal<Settings>,
    cards: RwSignal<Vec<SrsCard>>,
    attempts: RwSignal<Vec<AttemptRecord>>,
    days: RwSignal<Vec<String>>,
    manifest: &drive_study_core::AudioManifest,
    model: RwSignal<PlayerModel>,
) -> bool {
    let item = &plan.items[index];
    control.guessing.store(false, Ordering::Relaxed);
    match item {
        SessionItem::Narration { title, text, .. } => {
            set_model(
                model,
                plan,
                index,
                total,
                title,
                text,
                &[],
                None,
                "",
                false,
                false,
                false,
                control,
                correct,
                wrong,
            );
            speak(text, settings, manifest, control, model).await;
            matches!(control.take(), Some(Act::Repeat))
        }
        SessionItem::Card(card) => {
            set_model(
                model,
                plan,
                index,
                total,
                &format!("{} · {}", card.exam.as_str(), card.section),
                &card.front,
                &[],
                None,
                "",
                card.sample,
                false,
                true,
                control,
                correct,
                wrong,
            );
            speak(&card.front, settings, manifest, control, model).await;
            let early = control.take();
            if matches!(early, Some(Act::Repeat)) {
                return true;
            }
            let act = if early.is_some() {
                early
            } else {
                answer_window(
                    control,
                    window_ms(&settings.get_untracked(), plan.listen_only),
                    model,
                    ListenFor::Card,
                    true,
                )
                .await
            };
            match act {
                Some(Act::Repeat) => return true,
                Some(Act::Stop) => {
                    control.stop.store(true, Ordering::Relaxed);
                    return false;
                }
                None if plan.listen_only => {}
                Some(Act::Know) => {
                    grade(
                        cards,
                        attempts,
                        days,
                        &card.id,
                        ItemKind::Flashcard,
                        card.exam,
                        &card.section,
                        None,
                        ReviewOutcome::Correct,
                        false,
                        plan,
                    );
                    *correct += 1;
                }
                Some(Act::DontKnow) => {
                    grade(
                        cards,
                        attempts,
                        days,
                        &card.id,
                        ItemKind::Flashcard,
                        card.exam,
                        &card.section,
                        None,
                        ReviewOutcome::Wrong,
                        false,
                        plan,
                    );
                    *wrong += 1;
                }
                Some(Act::Skip) => {
                    *skipped += 1;
                    return false;
                }
                _ => {}
            }
            let mut shown = model.get_untracked();
            shown.explanation = card.back.clone();
            shown.answer_key = Some("back".into());
            shown.status = "Definition.".into();
            shown.correct = *correct;
            shown.wrong = *wrong;
            model.set(shown);
            speak(&card.spoken, settings, manifest, control, model).await;
            false
        }
        SessionItem::Question(question) => {
            let choices: Vec<(String, String)> = question
                .choices
                .iter()
                .map(|choice| (choice.key.clone(), choice.text.clone()))
                .collect();
            set_model(
                model,
                plan,
                index,
                total,
                &format!("{} · {}", question.exam.as_str(), question.section),
                &question.stem,
                &choices,
                None,
                "",
                question.sample,
                question.tags.iter().any(|tag| tag == "changes-yearly"),
                false,
                control,
                correct,
                wrong,
            );
            speak(&question.spoken_stem, settings, manifest, control, model).await;
            if handle_early(
                control, question, plan, cards, attempts, days, correct, wrong, skipped, responses,
                model,
            )
            .await
            {
                return finish_early(question, settings, manifest, control, model).await;
            }
            let cue = answer_cue(question);
            // The spoken stem already reads "A, … B, …". Speaking the choice list
            // again is the question (and the prompt) a second time.
            if stem_lists_choices(&question.spoken_stem, &choices, &question.kind) {
                if !plan.listen_only {
                    speak(&cue, settings, manifest, control, model).await;
                }
            } else {
                let readout = choices_readout(&question.kind, &choices);
                speak(&readout, settings, manifest, control, model).await;
            }
            if handle_early(
                control, question, plan, cards, attempts, days, correct, wrong, skipped, responses,
                model,
            )
            .await
            {
                return finish_early(question, settings, manifest, control, model).await;
            }
            let mut act = answer_window(
                control,
                window_ms(&settings.get_untracked(), plan.listen_only),
                model,
                ListenFor::Question(question),
                !plan.listen_only,
            )
            .await;
            if act.is_none() && !plan.listen_only && ear_should_retry() {
                let mut shown = model.get_untracked();
                shown.status = "Didn't catch that.".into();
                shown.listening = false;
                shown.window_left = None;
                model.set(shown);
                speak(&cue, settings, manifest, control, model).await;
                act =
                    answer_window(control, 6_000, model, ListenFor::Question(question), true).await;
            }
            match act {
                Some(Act::Repeat) => true,
                Some(Act::Stop) => {
                    control.stop.store(true, Ordering::Relaxed);
                    false
                }
                Some(Act::Skip) => {
                    *skipped += 1;
                    if plan.mode == SessionMode::Mock {
                        *wrong += 1;
                        responses.push((question.section.clone(), false));
                    }
                    false
                }
                None if plan.listen_only => {
                    reveal(model, question, None, control);
                    explain_after(question, false, settings, manifest, control, model).await;
                    false
                }
                Some(Act::Explain) | Some(Act::DontKnow) | Some(Act::Flip) | None => {
                    grade(
                        cards,
                        attempts,
                        days,
                        &question.id,
                        ItemKind::Question,
                        question.exam,
                        &question.section,
                        Some(question.topic.clone()),
                        ReviewOutcome::Wrong,
                        false,
                        plan,
                    );
                    *wrong += 1;
                    responses.push((question.section.clone(), false));
                    reveal(model, question, None, control);
                    explain_after(question, false, settings, manifest, control, model).await;
                    false
                }
                Some(Act::Answer(key)) => {
                    let right = key == question.answer;
                    score_answer(
                        question, &key, control, cards, attempts, days, plan, correct, wrong,
                        responses, model,
                    );
                    explain_after(question, right, settings, manifest, control, model).await;
                    false
                }
                _ => false,
            }
        }
    }
}

impl Control {
    fn peek_repeat(&self) -> bool {
        matches!(
            self.pending.lock().expect("pending").as_ref(),
            Some(Act::Repeat)
        )
    }
}

async fn finish_early(
    question: &drive_study_core::Question,
    settings: RwSignal<Settings>,
    manifest: &drive_study_core::AudioManifest,
    control: &Control,
    model: RwSignal<PlayerModel>,
) -> bool {
    if control.peek_repeat() {
        return true;
    }
    if model.get_untracked().answer_key.is_some() {
        let right = model.get_untracked().selected.as_deref() == Some(question.answer.as_str());
        explain_after(question, right, settings, manifest, control, model).await;
    }
    false
}

async fn explain_after(
    question: &drive_study_core::Question,
    correct: bool,
    settings: RwSignal<Settings>,
    manifest: &drive_study_core::AudioManifest,
    control: &Control,
    model: RwSignal<PlayerModel>,
) {
    let mode = settings.get_untracked().explain_correct;
    let full = !correct || mode == "always";
    if correct && mode == "never" {
        let mut shown = model.get_untracked();
        shown.explanation.clear();
        shown.explain_style = "never".into();
        model.set(shown);
        settle(control, 200).await;
        return;
    }
    if correct && mode == "short" {
        let line = first_sentence(&question.explanation);
        let mut shown = model.get_untracked();
        shown.explanation = line.clone();
        shown.explain_style = "short".into();
        model.set(shown);
        speak(&line, settings, manifest, control, model).await;
    } else if full {
        let mut shown = model.get_untracked();
        shown.explanation = question.explanation.clone();
        shown.explain_style = "full".into();
        model.set(shown);
        speak(
            &question.spoken_explanation,
            settings,
            manifest,
            control,
            model,
        )
        .await;
    }
    if control.has_pending() {
        let _ = control.take();
        return;
    }
    settle(control, 280).await;
}

fn first_sentence(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(index) = trimmed.find(". ") {
        return trimmed[..=index].trim().to_string();
    }
    if let Some(index) = trimmed.find('.') {
        return trimmed[..=index].trim().to_string();
    }
    trimmed.chars().take(140).collect()
}

fn window_ms(settings: &Settings, listen_only: bool) -> u32 {
    let base = settings.answer_seconds.clamp(6, 30) * 1000;
    if listen_only {
        base
    } else {
        base + 2_000
    }
}

async fn settle(control: &Control, ms: u32) {
    let mut left = ms;
    while left > 0 {
        if control.stop.load(Ordering::Relaxed) || control.has_pending() {
            let _ = control.take();
            return;
        }
        let step = 100.min(left);
        TimeoutFuture::new(step).await;
        left = left.saturating_sub(step);
    }
}

async fn handle_early(
    control: &Control,
    question: &drive_study_core::Question,
    plan: &SessionPlan,
    cards: RwSignal<Vec<SrsCard>>,
    attempts: RwSignal<Vec<AttemptRecord>>,
    days: RwSignal<Vec<String>>,
    correct: &mut u32,
    wrong: &mut u32,
    skipped: &mut u32,
    responses: &mut Vec<(String, bool)>,
    model: RwSignal<PlayerModel>,
) -> bool {
    let Some(act) = control.take() else {
        return false;
    };
    match act {
        Act::Repeat => {
            *control.pending.lock().expect("pending") = Some(Act::Repeat);
            true
        }
        Act::Stop => {
            control.stop.store(true, Ordering::Relaxed);
            true
        }
        Act::Skip => {
            *skipped += 1;
            if plan.mode == SessionMode::Mock {
                *wrong += 1;
                responses.push((question.section.clone(), false));
            }
            true
        }
        Act::Explain | Act::DontKnow | Act::Flip => {
            grade(
                cards,
                attempts,
                days,
                &question.id,
                ItemKind::Question,
                question.exam,
                &question.section,
                Some(question.topic.clone()),
                ReviewOutcome::Wrong,
                false,
                plan,
            );
            *wrong += 1;
            responses.push((question.section.clone(), false));
            reveal(model, question, None, control);
            true
        }
        Act::Answer(key) => {
            score_answer(
                question, &key, control, cards, attempts, days, plan, correct, wrong, responses,
                model,
            );
            true
        }
        Act::MoreTime => {
            control.interrupt(Act::MoreTime);
            false
        }
        _ => false,
    }
}

fn score_answer(
    question: &drive_study_core::Question,
    key: &str,
    control: &Control,
    cards: RwSignal<Vec<SrsCard>>,
    attempts: RwSignal<Vec<AttemptRecord>>,
    days: RwSignal<Vec<String>>,
    plan: &SessionPlan,
    correct: &mut u32,
    wrong: &mut u32,
    responses: &mut Vec<(String, bool)>,
    model: RwSignal<PlayerModel>,
) {
    let right = key == question.answer;
    let guessed = control.guessing.load(Ordering::Relaxed);
    let outcome = if right && guessed {
        ReviewOutcome::Guessed
    } else if right {
        ReviewOutcome::Correct
    } else {
        ReviewOutcome::Wrong
    };
    grade(
        cards,
        attempts,
        days,
        &question.id,
        ItemKind::Question,
        question.exam,
        &question.section,
        Some(question.topic.clone()),
        outcome,
        guessed && right,
        plan,
    );
    if right {
        *correct += 1;
    } else {
        *wrong += 1;
    }
    responses.push((question.section.clone(), right));
    model.update(|item| {
        item.correct = *correct;
        item.wrong = *wrong;
    });
    reveal(model, question, Some(key.to_string()), control);
}

fn reveal(
    model: RwSignal<PlayerModel>,
    question: &drive_study_core::Question,
    selected: Option<String>,
    control: &Control,
) {
    let mut shown = model.get_untracked();
    shown.answer_key = Some(question.answer.clone());
    shown.selected = selected.clone();
    shown.explanation = question.explanation.clone();
    shown.yearly = question.tags.iter().any(|tag| tag == "changes-yearly");
    shown.guessing = control.guessing.load(Ordering::Relaxed);
    shown.status = if selected.as_deref() == Some(question.answer.as_str()) {
        if shown.guessing {
            "Marked as a guess. It will come back.".into()
        } else {
            "Correct.".into()
        }
    } else if selected.is_some() {
        "Not this one.".into()
    } else {
        "Here is the answer.".into()
    };
    model.set(shown);
}

fn grade(
    cards: RwSignal<Vec<SrsCard>>,
    attempts: RwSignal<Vec<AttemptRecord>>,
    days: RwSignal<Vec<String>>,
    id: &str,
    kind: ItemKind,
    exam: ExamId,
    section: &str,
    topic: Option<String>,
    outcome: ReviewOutcome,
    guessed: bool,
    plan: &SessionPlan,
) {
    let meta = SrsMeta {
        id: id.into(),
        kind,
        exam,
        section: section.into(),
    };
    let today = today_key();
    let now = js_sys::Date::now() as i64;
    let existing = cards.get_untracked().into_iter().find(|card| card.id == id);
    let next = review_card(existing.as_ref(), &meta, outcome, now, &today);
    cards.update(|list| {
        if let Some(slot) = list.iter_mut().find(|card| card.id == next.id) {
            *slot = next.clone();
        } else {
            list.push(next.clone());
        }
    });
    let attempt = AttemptRecord {
        item_id: id.into(),
        kind,
        exam,
        section: section.into(),
        topic,
        correct: outcome != ReviewOutcome::Wrong,
        guessed,
        mode: format!("{:?}", plan.mode),
        at: now,
    };
    attempts.update(|list| list.push(attempt.clone()));
    let today_for_list = today.clone();
    days.update(|list| {
        if !list.iter().any(|day| day == &today_for_list) {
            list.push(today_for_list);
        }
    });
    progress::persist_progress();
}

fn set_model(
    model: RwSignal<PlayerModel>,
    plan: &SessionPlan,
    index: usize,
    total: usize,
    kicker: &str,
    prompt: &str,
    choices: &[(String, String)],
    answer: Option<String>,
    explanation: &str,
    sample: bool,
    yearly: bool,
    card_mode: bool,
    control: &Control,
    correct: &u32,
    wrong: &u32,
) {
    model.set(PlayerModel {
        title: plan.title.clone(),
        index: index + 1,
        total,
        kicker: kicker.into(),
        prompt: prompt.into(),
        choices: choices.to_vec(),
        answer_key: answer,
        selected: None,
        explanation: explanation.into(),
        status: "Reading the question.".into(),
        source: model.get_untracked().source,
        correct: *correct,
        wrong: *wrong,
        guessing: control.guessing.load(Ordering::Relaxed),
        window_left: None,
        explain_style: String::new(),
        sample,
        yearly,
        card_mode,
        remaining: plan.limit_seconds,
        paused: control.paused.load(Ordering::Relaxed),
        listening: false,
        listen_cue: String::new(),
        voice_fallback: false,
    });
}

enum ListenFor<'a> {
    Question(&'a drive_study_core::Question),
    Card,
}

fn answer_cue(question: &drive_study_core::Question) -> String {
    if question.kind == "tf" {
        return "Say true or false.".into();
    }
    let keys: Vec<&str> = question
        .choices
        .iter()
        .map(|choice| choice.key.as_str())
        .collect();
    match keys.as_slice() {
        [] => "Say the letter.".into(),
        [only] => format!("Say {only}."),
        [head @ .., last] => format!("Say {}, or {last}.", head.join(", ")),
    }
}

fn stem_lists_choices(stem: &str, choices: &[(String, String)], kind: &str) -> bool {
    if kind == "tf" {
        let lower = stem.to_lowercase();
        return lower.contains("true") && lower.contains("false");
    }
    if choices.is_empty() {
        return false;
    }
    let lower = stem.to_lowercase();
    choices.iter().all(|(key, _)| {
        let key = key.to_lowercase();
        lower.contains(&format!("{key},")) || lower.contains(&format!("{key}."))
    })
}

async fn answer_window(
    control: &Control,
    ms: u32,
    model: RwSignal<PlayerModel>,
    ear: ListenFor<'_>,
    spoken: bool,
) -> Option<Act> {
    chime();
    let cue = match ear {
        ListenFor::Question(question) => answer_cue(question),
        ListenFor::Card => "Say flip, know it, or don't know.".into(),
    };
    model.update(|item| {
        item.listen_cue = cue;
        item.listening = false;
        item.paused = control.paused.load(Ordering::Relaxed);
    });
    let mut left = ms;
    let mut mic = false;
    let mut shown_seconds = u32::MAX;
    let wall_start = js_sys::Date::now();
    let wall_cap = ms as f64 + 2_500.0;
    while left > 0 {
        let seconds = left.div_ceil(1000);
        if seconds != shown_seconds {
            shown_seconds = seconds;
            model.update(|item| {
                item.window_left = Some(seconds);
                item.paused = control.paused.load(Ordering::Relaxed);
            });
        }
        if !mic && !synth_ours() {
            match ear {
                ListenFor::Question(question) => start_listen_question(question, control.clone()),
                ListenFor::Card => start_listen_card(control.clone()),
            }
            if spoken {
                model.update(|item| {
                    item.listening = true;
                    item.status = "Listening.".into();
                });
            }
            mic = true;
        }
        if mic {
            pump_listen();
        }
        if let Some(act) = control.take() {
            if matches!(act, Act::MoreTime) {
                left = left.saturating_add(10_000);
                chime();
                continue;
            }
            close_window(model);
            stop_listen();
            return Some(act);
        }
        if control.stop.load(Ordering::Relaxed) {
            close_window(model);
            stop_listen();
            return Some(Act::Stop);
        }
        let step = 50.min(left);
        TimeoutFuture::new(step).await;
        if page_is_hidden() {
            continue;
        }
        if js_sys::Date::now() - wall_start > wall_cap {
            break;
        }
        // A chime or clip can make the phone send a media pause while the
        // screen is still on. That is not the driver asking to wait.
        if control.paused.load(Ordering::Relaxed) && !control.user_paused() {
            control.paused.store(false, Ordering::Relaxed);
            revive_output();
        }
        if !control.user_paused() {
            left = left.saturating_sub(step);
        }
    }
    close_window(model);
    stop_listen();
    None
}

fn close_window(model: RwSignal<PlayerModel>) {
    model.update(|item| {
        item.window_left = None;
        item.listening = false;
    });
}

fn chime() {
    quiet_media_keys(700.0);
    let Some(url) = beep_url() else { return };
    let Ok(audio) = web_sys::HtmlAudioElement::new_with_src(&url) else {
        return;
    };
    audio.set_volume(0.45);
    let _ = audio.play();
}

fn beep_url() -> Option<String> {
    thread_local! {
        static URL: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
    }
    let cached = URL.with(|slot| slot.borrow().clone());
    if cached.is_some() {
        return cached;
    }
    let url = format!("data:audio/wav;base64,{}", base64(&tone_wav()));
    URL.with(|slot| *slot.borrow_mut() = Some(url.clone()));
    Some(url)
}

fn tone_wav() -> Vec<u8> {
    let sample_rate = 22_050u32;
    let samples = (sample_rate as f32 * 0.14) as usize;
    let mut pcm = Vec::with_capacity(samples * 2);
    for index in 0..samples {
        let time = index as f32 / sample_rate as f32;
        let env = (1.0 - time / 0.14).max(0.0);
        let wave = (time * 880.0 * std::f32::consts::TAU).sin();
        let value = (wave * env * 0.45 * i16::MAX as f32) as i16;
        pcm.extend_from_slice(&value.to_le_bytes());
    }
    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    out.extend(pcm);
    out
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let mut value = (chunk[0] as u32) << 16;
        if chunk.len() > 1 {
            value |= (chunk[1] as u32) << 8;
        }
        if chunk.len() > 2 {
            value |= chunk[2] as u32;
        }
        out.push(TABLE[((value >> 18) & 63) as usize] as char);
        out.push(TABLE[((value >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

enum Pass {
    Finished,
    Stopped,
    Retune,
}

async fn speak(
    text: &str,
    settings: RwSignal<Settings>,
    manifest: &drive_study_core::AudioManifest,
    control: &Control,
    model: RwSignal<PlayerModel>,
) {
    for _ in 0..4 {
        if control.stop.load(Ordering::Relaxed) {
            return;
        }
        wait_until_foreground(control).await;
        let epoch = control.voice_epoch();
        let generation = control.generation.load(Ordering::Relaxed);
        match speak_once(text, settings, manifest, control, model, generation, epoch).await {
            Pass::Retune => continue,
            _ => return,
        }
    }
}

async fn wait_until_foreground(control: &Control) {
    while page_is_hidden() && !control.stop.load(Ordering::Relaxed) && !control.has_pending() {
        TimeoutFuture::new(80).await;
    }
    if !page_is_hidden() && !control.user_paused() {
        revive_output();
    }
}

async fn speak_once(
    text: &str,
    settings: RwSignal<Settings>,
    manifest: &drive_study_core::AudioManifest,
    control: &Control,
    model: RwSignal<PlayerModel>,
    generation: u32,
    epoch: u32,
) -> Pass {
    if control.voice_epoch() != epoch {
        return Pass::Retune;
    }
    let prefs = settings.get_untracked();
    if prefs.voice_source != "phone" {
        let key = clip_key(VOICE_ID, text);
        let had_clip = manifest.clips.contains_key(&key);
        if let Some(clip) = manifest.clips.get(&key) {
            if let Some(url) = resolve_clip(&clip.path, &prefs.voice_source).await {
                match play_url(
                    &url,
                    prefs.rate,
                    control,
                    generation,
                    utterance_budget_ms(text, prefs.rate),
                    epoch,
                )
                .await
                {
                    ClipEnd::Played => {
                        let mut shown = model.get_untracked();
                        shown.voice_fallback = false;
                        shown.source =
                            if url.starts_with("blob:") || prefs.voice_source == "downloaded" {
                                "Downloaded clip".into()
                            } else {
                                "Streamed clip".into()
                            };
                        model.set(shown);
                        return if control.voice_epoch() != epoch {
                            Pass::Retune
                        } else {
                            Pass::Finished
                        };
                    }
                    ClipEnd::Retune => return Pass::Retune,
                    ClipEnd::Stopped => return Pass::Stopped,
                    ClipEnd::Missed => {}
                }
            }
        }
        if !control.live(generation) || control.has_pending() {
            return Pass::Stopped;
        }
        if control.voice_epoch() != epoch {
            return Pass::Retune;
        }
        let mut shown = model.get_untracked();
        shown.voice_fallback = had_clip;
        shown.source = if had_clip {
            match prefs.voice_source.as_str() {
                "downloaded" => "Phone voice. The saved clip didn't play.".into(),
                _ => "Phone voice. No streamed clip played, so this is the phone voice.".into(),
            }
        } else {
            match prefs.voice_source.as_str() {
                "downloaded" => "Phone voice. No downloaded clip for this line yet.".into(),
                _ => "Phone voice. No streamed clip for this line yet.".into(),
            }
        };
        model.set(shown);
    } else {
        let mut shown = model.get_untracked();
        shown.voice_fallback = false;
        shown.source = "Phone voice".into();
        model.set(shown);
    }
    speak_phone(
        text,
        prefs.rate,
        prefs.voice_uri.as_deref(),
        control,
        generation,
        epoch,
        model,
    )
    .await
}

async fn resolve_clip(path: &str, mode: &str) -> Option<String> {
    let url = absolute_url(path)?;
    let cached = cached_blob(&url).await;
    let online = web_sys::window()
        .map(|window| window.navigator().on_line())
        .unwrap_or(false);
    if mode == "downloaded" {
        if cached.is_some() {
            return cached;
        }
        if online {
            return Some(url);
        }
        return None;
    }
    if online {
        return Some(url);
    }
    cached
}

fn absolute_url(path: &str) -> Option<String> {
    let href = web_sys::window()?.location().href().ok()?;
    web_sys::Url::new_with_base(path, &href)
        .ok()
        .map(|url| url.href())
}

async fn cached_blob(url: &str) -> Option<String> {
    let window = web_sys::window()?;
    let caches = window.caches().ok()?;
    let cache = JsFuture::from(caches.open("drive-study-audio-v1"))
        .await
        .ok()?;
    let cache: web_sys::Cache = cache.dyn_into().ok()?;
    let hit = JsFuture::from(cache.match_with_str(url)).await.ok()?;
    if hit.is_undefined() || hit.is_null() {
        return None;
    }
    let response: web_sys::Response = hit.dyn_into().ok()?;
    let blob = JsFuture::from(response.blob().ok()?).await.ok()?;
    let blob: web_sys::Blob = blob.dyn_into().ok()?;
    web_sys::Url::create_object_url_with_blob(&blob).ok()
}

fn utterance_budget_ms(text: &str, rate: f64) -> f64 {
    let words = text.split_whitespace().count().max(1) as f64;
    (words / (2.2 * rate.max(0.8)) * 1000.0 + 1_200.0).clamp(1_500.0, 90_000.0)
}

enum ClipEnd {
    Played,
    Stopped,
    Missed,
    Retune,
}

async fn play_url(
    url: &str,
    rate: f64,
    control: &Control,
    generation: u32,
    budget_ms: f64,
    epoch: u32,
) -> ClipEnd {
    quiet_media_keys(800.0);
    let Ok(audio) = web_sys::HtmlAudioElement::new_with_src(url) else {
        return ClipEnd::Missed;
    };
    audio.set_playback_rate(rate);
    CURRENT.with(|slot| *slot.borrow_mut() = Some(audio.clone()));
    CLIP_ACTIVE.with(|slot| slot.set(true));
    let Ok(promise) = audio.play() else {
        silence_element(&audio);
        CLIP_ACTIVE.with(|slot| slot.set(false));
        return ClipEnd::Missed;
    };
    let mut started = js_sys::Date::now();
    let play = JsFuture::from(promise);
    let timeout = TimeoutFuture::new(1_600);
    futures::pin_mut!(play, timeout);
    match futures::future::select(play, timeout).await {
        futures::future::Either::Left((Ok(_), _)) => {}
        futures::future::Either::Left((Err(_), _)) => {
            silence_element(&audio);
            CLIP_ACTIVE.with(|slot| slot.set(false));
            return ClipEnd::Missed;
        }
        futures::future::Either::Right(_) => {
            if audio.current_time() <= 0.02 && audio.paused() {
                silence_element(&audio);
                CLIP_ACTIVE.with(|slot| slot.set(false));
                return ClipEnd::Missed;
            }
        }
    }
    while !audio.ended() && !element_failed(&audio) {
        if control.voice_epoch() != epoch {
            silence_element(&audio);
            CLIP_ACTIVE.with(|slot| slot.set(false));
            return ClipEnd::Retune;
        }
        if !control.live(generation) || control.has_pending() {
            silence_element(&audio);
            CLIP_ACTIVE.with(|slot| slot.set(false));
            return ClipEnd::Stopped;
        }
        if page_is_hidden() {
            let mark = js_sys::Date::now();
            while page_is_hidden() && control.live(generation) && !control.has_pending() {
                TimeoutFuture::new(80).await;
            }
            started += js_sys::Date::now() - mark;
            if !control.user_paused() {
                let _ = audio.play();
            }
        }
        let elapsed = js_sys::Date::now() - started;
        let duration_ms = audio.duration();
        let cap = if duration_ms.is_finite() && duration_ms > 0.0 {
            duration_ms * 1000.0 / rate.max(0.8) + 1_200.0
        } else {
            budget_ms
        };
        if elapsed > cap {
            break;
        }
        if control.paused.load(Ordering::Relaxed) {
            let _ = audio.pause();
            while control.paused.load(Ordering::Relaxed)
                && control.live(generation)
                && control.voice_epoch() == epoch
            {
                TimeoutFuture::new(80).await;
            }
            if control.live(generation) && !control.user_paused() {
                let _ = audio.play();
            }
        }
        TimeoutFuture::new(40).await;
    }
    let played = audio.current_time() > 0.05 || audio.ended();
    CLIP_ACTIVE.with(|slot| slot.set(false));
    if element_failed(&audio) && audio.current_time() <= 0.05 {
        silence_element(&audio);
        return ClipEnd::Missed;
    }
    if played {
        ClipEnd::Played
    } else {
        silence_element(&audio);
        ClipEnd::Missed
    }
}

fn element_failed(audio: &web_sys::HtmlAudioElement) -> bool {
    js_sys::Reflect::get(audio.as_ref(), &JsValue::from_str("error"))
        .ok()
        .is_some_and(|value| !value.is_null() && !value.is_undefined())
}

async fn speak_phone(
    text: &str,
    rate: f64,
    voice_uri: Option<&str>,
    control: &Control,
    generation: u32,
    epoch: u32,
    model: RwSignal<PlayerModel>,
) -> Pass {
    let Some(synth) = synthesis() else {
        mark_voice_down(model, "This browser has no speaking voice.");
        return Pass::Finished;
    };
    ensure_voices(&synth).await;
    if !control.live(generation) || control.has_pending() {
        return Pass::Stopped;
    }
    if control.voice_epoch() != epoch {
        return Pass::Retune;
    }
    // A paused or queued synthesis swallows the next speak() and never fires onend.
    // Cancel, let that settle, then resume. Doing it in the same turn makes iOS
    // speak the line twice, so the new utterance is queued on the next tick.
    if synth.paused() || synth.pending() {
        synth.cancel();
        let _ = synth.resume();
        TimeoutFuture::new(60).await;
        if !control.live(generation) || control.has_pending() {
            return Pass::Stopped;
        }
        if control.voice_epoch() != epoch {
            return Pass::Retune;
        }
    }
    let Ok(utterance) = web_sys::SpeechSynthesisUtterance::new_with_text(text) else {
        mark_voice_down(model, "Could not start the phone voice.");
        return Pass::Finished;
    };
    utterance.set_rate(rate as f32);
    utterance.set_lang("en-US");
    if let Some(uri) = voice_uri {
        let voices = synth.get_voices();
        for index in 0..voices.length() {
            if let Ok(voice) = voices
                .get(index)
                .dyn_into::<web_sys::SpeechSynthesisVoice>()
            {
                if voice.voice_uri() == uri {
                    utterance.set_voice(Some(&voice));
                }
            }
        }
    }
    let words = text.split_whitespace().count();
    let budget = utterance_budget_ms(text, rate);
    let begun = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));
    let begun_flag = begun.clone();
    let on_start = Closure::wrap(Box::new(move || {
        begun_flag.store(true, Ordering::Relaxed);
    }) as Box<dyn FnMut()>);
    let started_wall = js_sys::Date::now();
    let done_flag = done.clone();
    let begun_for_end = begun.clone();
    let on_end = Closure::wrap(Box::new(move |event: JsValue| {
        let started = begun_for_end.load(Ordering::Relaxed);
        if end_is_real(&event, words, started_wall, budget, started) {
            done_flag.store(true, Ordering::Relaxed);
        }
    }) as Box<dyn FnMut(JsValue)>);
    let done_error = done.clone();
    let on_error = Closure::wrap(Box::new(move || {
        done_error.store(true, Ordering::Relaxed);
    }) as Box<dyn FnMut()>);
    utterance.set_onstart(Some(on_start.as_ref().unchecked_ref()));
    utterance.set_onend(Some(on_end.as_ref().unchecked_ref()));
    utterance.set_onerror(Some(on_error.as_ref().unchecked_ref()));
    set_synth_ours(true);
    synth.speak(&utterance);
    let mut clock = js_sys::Date::now();
    let mut retried = false;
    let mut restarted_after_hide = false;
    while !done.load(Ordering::Relaxed) {
        if !control.live(generation) || control.has_pending() {
            if synth_ours() {
                synth.cancel();
                let _ = synth.resume();
                set_synth_ours(false);
            }
            return Pass::Stopped;
        }
        if control.voice_epoch() != epoch {
            if synth_ours() {
                synth.cancel();
                let _ = synth.resume();
                set_synth_ours(false);
            }
            return Pass::Retune;
        }
        if page_is_hidden() {
            let mark = js_sys::Date::now();
            while page_is_hidden() && control.live(generation) && !control.has_pending() {
                TimeoutFuture::new(80).await;
            }
            clock += js_sys::Date::now() - mark;
            if !done.load(Ordering::Relaxed) && !restarted_after_hide && !control.user_paused() {
                restarted_after_hide = true;
                synth.cancel();
                let _ = synth.resume();
                TimeoutFuture::new(60).await;
                if control.live(generation)
                    && !control.has_pending()
                    && control.voice_epoch() == epoch
                {
                    set_synth_ours(true);
                    synth.speak(&utterance);
                    begun.store(false, Ordering::Relaxed);
                }
            }
            continue;
        }
        if control.paused.load(Ordering::Relaxed) && control.user_paused() {
            while control.user_paused() && control.live(generation) {
                TimeoutFuture::new(80).await;
            }
            if SYNTH_HELD.with(Cell::get) {
                let _ = synth.resume();
                SYNTH_HELD.with(|slot| slot.set(false));
            }
            continue;
        }
        if !begun.load(Ordering::Relaxed) && !retried && js_sys::Date::now() - clock > 1_000.0 {
            retried = true;
            synth.cancel();
            let _ = synth.resume();
            TimeoutFuture::new(60).await;
            if control.live(generation) && !control.has_pending() && control.voice_epoch() == epoch
            {
                set_synth_ours(true);
                synth.speak(&utterance);
            }
            continue;
        }
        // Headless WebKit never starts the utterance and never fires onend.
        // Waiting out a long question here is what left the screen on
        // "Reading the question."
        let silent = js_sys::Date::now() - clock;
        if !begun.load(Ordering::Relaxed) && retried && silent > 2_500.0 {
            break;
        }
        if begun.load(Ordering::Relaxed) && !synth.speaking() && silent > 500.0 {
            break;
        }
        if js_sys::Date::now() - clock >= budget {
            if begun.load(Ordering::Relaxed) && synth_ours() {
                synth.cancel();
                let _ = synth.resume();
            }
            break;
        }
        TimeoutFuture::new(40).await;
    }
    set_synth_ours(false);
    let elapsed = js_sys::Date::now() - clock;
    if begun.load(Ordering::Relaxed)
        && elapsed > 80.0
        && control.live(generation)
        && !control.has_pending()
        && control.voice_epoch() == epoch
    {
        TimeoutFuture::new(180).await;
    }
    if !begun.load(Ordering::Relaxed) && !done.load(Ordering::Relaxed) {
        mark_voice_down(
            model,
            "Phone voice didn't start. Tap Play, or switch voice.",
        );
    }
    if control.voice_epoch() != epoch {
        Pass::Retune
    } else {
        Pass::Finished
    }
}

fn mark_voice_down(model: RwSignal<PlayerModel>, note: &str) {
    let mut shown = model.get_untracked();
    shown.voice_fallback = true;
    shown.source = note.into();
    model.set(shown);
}

fn end_is_real(
    event: &JsValue,
    words: usize,
    started_wall: f64,
    budget: f64,
    started: bool,
) -> bool {
    let elapsed = js_sys::Reflect::get(event, &JsValue::from_str("elapsedTime"))
        .ok()
        .and_then(|value| value.as_f64());
    match elapsed {
        None => true,
        Some(time) if time > 0.05 => true,
        // WebKit reports elapsedTime 0 on a real end. A spurious end arrives
        // before onstart; once speech has started, this end is the real one.
        Some(_) if started => true,
        Some(_) => words <= 3 || js_sys::Date::now() - started_wall > budget * 0.55,
    }
}

async fn ensure_voices(synth: &web_sys::SpeechSynthesis) {
    if synth.get_voices().length() > 0 {
        return;
    }
    // Voices on iOS arrive in one voiceschanged, often after the first call.
    // Waiting on every line would stall a whole lesson.
    if VOICES_WAITED.with(Cell::get) {
        return;
    }
    VOICES_WAITED.with(|slot| slot.set(true));
    let ready = Arc::new(AtomicBool::new(false));
    let flag = ready.clone();
    let on_voices = Closure::wrap(Box::new(move || {
        flag.store(true, Ordering::Relaxed);
    }) as Box<dyn FnMut()>);
    synth.set_onvoiceschanged(Some(on_voices.as_ref().unchecked_ref()));
    let started = js_sys::Date::now();
    while !ready.load(Ordering::Relaxed) && js_sys::Date::now() - started < 700.0 {
        if synth.get_voices().length() > 0 {
            break;
        }
        TimeoutFuture::new(40).await;
    }
    synth.set_onvoiceschanged(None);
}

struct Ear {
    token: u32,
    rec_id: u32,
    available: bool,
    accepted: bool,
    active: bool,
    needs_restart: bool,
    restarts: u8,
    last_start: f64,
    options: ParseOptions,
    control: Control,
}

thread_local! {
    static EAR: RefCell<Option<Ear>> = const { RefCell::new(None) };
    static REC: RefCell<Option<JsValue>> = const { RefCell::new(None) };
    static LISTEN_TOKEN: Cell<u32> = const { Cell::new(0) };
}

fn ear_should_retry() -> bool {
    EAR.with(|slot| {
        slot.borrow()
            .as_ref()
            .is_some_and(|ear| ear.available && !ear.accepted)
    })
}

fn start_listen_question(question: &drive_study_core::Question, control: Control) {
    let options = ParseOptions {
        choice_keys: question
            .choices
            .iter()
            .map(|choice| choice.key.clone())
            .collect(),
        allow_know: false,
        true_key: question
            .choices
            .iter()
            .find(|choice| choice.text.eq_ignore_ascii_case("true"))
            .map(|choice| choice.key.clone()),
        false_key: question
            .choices
            .iter()
            .find(|choice| choice.text.eq_ignore_ascii_case("false"))
            .map(|choice| choice.key.clone()),
    };
    begin_ear(options, control);
}

fn start_listen_card(control: Control) {
    begin_ear(
        ParseOptions {
            choice_keys: Vec::new(),
            allow_know: true,
            true_key: None,
            false_key: None,
        },
        control,
    );
}

fn begin_ear(options: ParseOptions, control: Control) {
    let token = LISTEN_TOKEN.with(|slot| {
        let next = slot.get().wrapping_add(1);
        slot.set(next);
        next
    });
    EAR.with(|slot| {
        *slot.borrow_mut() = Some(Ear {
            token,
            rec_id: 0,
            available: false,
            accepted: false,
            active: true,
            needs_restart: false,
            restarts: 0,
            last_start: 0.0,
            options,
            control,
        });
    });
    if !open_recognition(token) {
        EAR.with(|slot| {
            if let Some(ear) = slot.borrow_mut().as_mut() {
                if ear.token == token {
                    ear.available = false;
                    ear.active = false;
                }
            }
        });
    }
}

fn open_recognition(token: u32) -> bool {
    let prepared = EAR.with(|slot| {
        let mut borrow = slot.borrow_mut();
        let ear = borrow.as_mut()?;
        if ear.token != token || !ear.active {
            return None;
        }
        ear.rec_id = ear.rec_id.wrapping_add(1);
        ear.last_start = js_sys::Date::now();
        ear.available = true;
        ear.needs_restart = false;
        Some((ear.options.clone(), ear.control.clone(), ear.rec_id))
    });
    let Some((options, control, rec_id)) = prepared else {
        return false;
    };
    abort_rec_only();
    let Some(window) = web_sys::window() else {
        return false;
    };
    let ctor = ["webkitSpeechRecognition", "SpeechRecognition"]
        .into_iter()
        .find_map(|name| {
            js_sys::Reflect::get(&window, &JsValue::from_str(name))
                .ok()
                .filter(|value| value.is_function())
        });
    let Some(ctor) = ctor.and_then(|value| value.dyn_into::<js_sys::Function>().ok()) else {
        return false;
    };
    let Ok(rec) = js_sys::Reflect::construct(&ctor, &js_sys::Array::new()) else {
        return false;
    };
    let _ = js_sys::Reflect::set(&rec, &"lang".into(), &"en-US".into());
    // iOS ignores continuous=true and often ends the recognizer immediately.
    let _ = js_sys::Reflect::set(&rec, &"continuous".into(), &JsValue::FALSE);
    let _ = js_sys::Reflect::set(&rec, &"interimResults".into(), &JsValue::TRUE);
    let _ = js_sys::Reflect::set(&rec, &"maxAlternatives".into(), &JsValue::from(5));
    attach_grammar(&rec, &options);
    let options_result = options.clone();
    let control_result = control.clone();
    let on_result = Closure::wrap(Box::new(move |event: JsValue| {
        on_recognition_result(&event, token, rec_id, &options_result, &control_result);
    }) as Box<dyn FnMut(JsValue)>);
    let _ = js_sys::Reflect::set(&rec, &"onresult".into(), on_result.as_ref());
    on_result.forget();
    let on_end =
        Closure::wrap(Box::new(move || note_recognition_end(token, rec_id)) as Box<dyn FnMut()>);
    let _ = js_sys::Reflect::set(&rec, &"onend".into(), on_end.as_ref());
    on_end.forget();
    let on_error = Closure::wrap(Box::new(move |event: JsValue| {
        let name = js_sys::Reflect::get(&event, &JsValue::from_str("error"))
            .ok()
            .and_then(|value| value.as_string())
            .unwrap_or_default();
        note_recognition_error(token, rec_id, &name);
    }) as Box<dyn FnMut(JsValue)>);
    let _ = js_sys::Reflect::set(&rec, &"onerror".into(), on_error.as_ref());
    on_error.forget();
    let Ok(start) = js_sys::Reflect::get(&rec, &"start".into())
        .and_then(|value| value.dyn_into::<js_sys::Function>())
    else {
        return false;
    };
    if start.call0(&rec).is_err() {
        return false;
    }
    REC.with(|slot| *slot.borrow_mut() = Some(rec));
    true
}

fn attach_grammar(rec: &JsValue, options: &ParseOptions) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(ctor) = js_sys::Reflect::get(&window, &JsValue::from_str("SpeechGrammarList")) else {
        return;
    };
    if !ctor.is_function() {
        return;
    }
    let Ok(ctor) = ctor.dyn_into::<js_sys::Function>() else {
        return;
    };
    let Ok(list) = js_sys::Reflect::construct(&ctor, &js_sys::Array::new()) else {
        return;
    };
    let source = answer_grammar(&options.choice_keys);
    let Ok(add) = js_sys::Reflect::get(&list, &JsValue::from_str("addFromString"))
        .and_then(|value| value.dyn_into::<js_sys::Function>())
    else {
        return;
    };
    if add
        .call2(&list, &JsValue::from_str(&source), &JsValue::from(1))
        .is_ok()
    {
        let _ = js_sys::Reflect::set(rec, &JsValue::from_str("grammars"), &list);
    }
}

fn on_recognition_result(
    event: &JsValue,
    token: u32,
    rec_id: u32,
    options: &ParseOptions,
    control: &Control,
) {
    let Ok(results) = js_sys::Reflect::get(event, &JsValue::from_str("results")) else {
        return;
    };
    let results = js_sys::Array::from(&results);
    if results.length() == 0 {
        return;
    }
    let result = results.get(results.length() - 1);
    let is_final = js_sys::Reflect::get(&result, &JsValue::from_str("isFinal"))
        .ok()
        .and_then(|value| value.as_bool())
        .unwrap_or(true);
    let alternatives = js_sys::Array::from(&result);
    for index in 0..alternatives.length() {
        let alternative = alternatives.get(index);
        let Ok(transcript) = js_sys::Reflect::get(&alternative, &JsValue::from_str("transcript"))
        else {
            continue;
        };
        let Some(transcript) = transcript.as_string() else {
            continue;
        };
        if transcript.trim().is_empty() {
            continue;
        }
        let confidence = js_sys::Reflect::get(&alternative, &JsValue::from_str("confidence"))
            .ok()
            .and_then(|value| value.as_f64())
            .unwrap_or(1.0);
        if !is_final && confidence < 0.75 {
            continue;
        }
        let command = parse_voice(&transcript, options);
        if matches!(command, VoiceCommand::Unrecognized) {
            continue;
        }
        if !claim_recognition(token, rec_id) {
            return;
        }
        apply_voice(command, control);
        return;
    }
}

fn claim_recognition(token: u32, rec_id: u32) -> bool {
    EAR.with(|slot| {
        let mut borrow = slot.borrow_mut();
        let Some(ear) = borrow.as_mut() else {
            return false;
        };
        if ear.token != token || ear.rec_id != rec_id || !ear.active || ear.accepted {
            return false;
        }
        ear.accepted = true;
        ear.active = false;
        ear.needs_restart = false;
        true
    })
}

fn note_recognition_end(token: u32, rec_id: u32) {
    EAR.with(|slot| {
        let mut borrow = slot.borrow_mut();
        let Some(ear) = borrow.as_mut() else {
            return;
        };
        if ear.token != token || ear.rec_id != rec_id || !ear.active || ear.accepted {
            return;
        }
        ear.needs_restart = true;
    });
}

fn note_recognition_error(token: u32, rec_id: u32, name: &str) {
    if name == "not-allowed" || name == "service-not-allowed" {
        EAR.with(|slot| {
            if let Some(ear) = slot.borrow_mut().as_mut() {
                if ear.token == token {
                    ear.active = false;
                    ear.needs_restart = false;
                }
            }
        });
        return;
    }
    if name == "aborted" {
        return;
    }
    note_recognition_end(token, rec_id);
}

fn pump_listen() {
    let token = EAR.with(|slot| {
        let mut borrow = slot.borrow_mut();
        let Some(ear) = borrow.as_mut() else {
            return None;
        };
        if !ear.active || !ear.needs_restart || ear.accepted {
            return None;
        }
        let now = js_sys::Date::now();
        if now - ear.last_start < 280.0 {
            return None;
        }
        if ear.restarts >= 5 {
            ear.needs_restart = false;
            ear.active = false;
            return None;
        }
        ear.restarts += 1;
        ear.needs_restart = false;
        Some(ear.token)
    });
    if let Some(token) = token {
        let _ = open_recognition(token);
    }
}

fn apply_voice(command: VoiceCommand, control: &Control) {
    match command {
        VoiceCommand::Answer(key) => control.interrupt(Act::Answer(key)),
        VoiceCommand::MoreTime => control.interrupt(Act::MoreTime),
        VoiceCommand::Repeat => control.interrupt(Act::Repeat),
        VoiceCommand::Skip => control.interrupt(Act::Skip),
        VoiceCommand::Explain => control.interrupt(Act::Explain),
        VoiceCommand::DontKnow => control.interrupt(Act::DontKnow),
        VoiceCommand::Pause => control.interrupt(Act::Pause),
        VoiceCommand::Stop => control.interrupt(Act::Stop),
        VoiceCommand::Know => control.interrupt(Act::Know),
        VoiceCommand::Flip => control.interrupt(Act::Flip),
        VoiceCommand::Unrecognized => {}
    }
}

fn stop_listen() {
    EAR.with(|slot| {
        if let Some(ear) = slot.borrow_mut().as_mut() {
            ear.active = false;
            ear.needs_restart = false;
        }
    });
    abort_rec_only();
}

fn abort_rec_only() {
    REC.with(|slot| {
        if let Some(rec) = slot.borrow_mut().take() {
            if let Ok(abort) = js_sys::Reflect::get(&rec, &JsValue::from_str("abort"))
                .and_then(|value| value.dyn_into::<js_sys::Function>())
            {
                let _ = abort.call0(&rec);
            }
        }
    });
}

pub fn today_key() -> String {
    let date = js_sys::Date::new_0();
    format!(
        "{:04}-{:02}-{:02}",
        date.get_full_year() as i32,
        date.get_month() as i32 + 1,
        date.get_date() as i32
    )
}

#[path = "progress.rs"]
mod progress;
pub use progress::{
    bind_progress, boot_state, create_profile, export_progress, import_progress, import_replaces,
    mark_progress_ready, parse_transfer, persist_note, persist_progress, switch_profile, sync_code,
    LoadedState, ProfileHead,
};

#[derive(Clone, Default)]
pub struct DownloadProgress {
    pub label: String,
    pub done: u32,
    pub total: u32,
    pub bytes: u64,
    pub failed: u32,
    pub message: String,
    pub finished: bool,
}

pub async fn download_urls(
    clips: &[(String, u64)],
    label: &str,
    progress: RwSignal<DownloadProgress>,
) -> Result<(), String> {
    let window = web_sys::window().ok_or("no window")?;
    let caches = window
        .caches()
        .map_err(|_| "This browser cannot store audio offline.")?;
    let cache = JsFuture::from(caches.open("drive-study-audio-v1"))
        .await
        .map_err(|_| "Could not open the audio cache.")?;
    let cache: web_sys::Cache = cache.dyn_into().map_err(|_| "cache")?;
    let total = clips.len() as u32;
    let mut done = 0u32;
    let mut failed = 0u32;
    let mut bytes = 0u64;
    let mut message = String::new();
    progress.set(DownloadProgress {
        label: label.into(),
        done,
        total,
        bytes,
        failed,
        message: "Starting.".into(),
        finished: false,
    });
    for (url, size) in clips {
        let hit = JsFuture::from(cache.match_with_str(url)).await.ok();
        let already = hit
            .as_ref()
            .is_some_and(|value| !value.is_undefined() && !value.is_null());
        if already {
            bytes += *size;
        } else {
            match fetch_into_cache(&window, &cache, url).await {
                Ok(saved) => bytes += if saved == 0 { *size } else { saved },
                Err(error) => {
                    failed += 1;
                    message = error;
                }
            }
        }
        done += 1;
        progress.set(DownloadProgress {
            label: label.into(),
            done,
            total,
            bytes,
            failed,
            message: if message.is_empty() {
                format!("{done} of {total}")
            } else {
                message.clone()
            },
            finished: false,
        });
    }
    let summary = if failed == 0 {
        "Saved on this phone. Already stored clips were skipped.".into()
    } else {
        format!("{failed} clips failed. {message} Tap download again to resume.")
    };
    progress.set(DownloadProgress {
        label: label.into(),
        done,
        total,
        bytes,
        failed,
        message: summary,
        finished: true,
    });
    if failed == 0 {
        Ok(())
    } else {
        Err(format!("{failed} clips failed"))
    }
}

async fn fetch_into_cache(
    window: &web_sys::Window,
    cache: &web_sys::Cache,
    url: &str,
) -> Result<u64, String> {
    let response = JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(|_| format!("Could not fetch {url}"))?;
    let response: web_sys::Response = response
        .dyn_into()
        .map_err(|_| "Bad response".to_string())?;
    if !response.ok() {
        return Err(format!("Clip missing ({})", response.status()));
    }
    let length = response
        .headers()
        .get("content-length")
        .ok()
        .flatten()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    JsFuture::from(cache.put_with_str(url, &response))
        .await
        .map_err(|_| format!("Could not store {url}"))?;
    Ok(length)
}

pub async fn delete_urls(urls: &[String]) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(caches) = window.caches() else { return };
    let Ok(cache) = JsFuture::from(caches.open("drive-study-audio-v1")).await else {
        return;
    };
    let Ok(cache) = cache.dyn_into::<web_sys::Cache>() else {
        return;
    };
    for url in urls {
        let _ = JsFuture::from(cache.delete_with_str(url)).await;
    }
}

pub fn register_service_worker() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let container = window.navigator().service_worker();
    let had_controller = js_sys::Reflect::get(&container, &JsValue::from_str("controller"))
        .ok()
        .is_some_and(|value| !value.is_null() && !value.is_undefined());
    let _ = container.register("sw.js");
    if !had_controller {
        return;
    }
    let closure = Closure::wrap(Box::new(move || {
        let Some(window) = web_sys::window() else {
            return;
        };
        if let Some(storage) = window.session_storage().ok().flatten() {
            if storage.get_item("ds-sw-reload").ok().flatten().is_some() {
                return;
            }
            let _ = storage.set_item("ds-sw-reload", "1");
        }
        let _ = window.location().reload();
    }) as Box<dyn FnMut()>);
    let _ = container
        .add_event_listener_with_callback("controllerchange", closure.as_ref().unchecked_ref());
    closure.forget();
}

pub fn is_due_card(card: &SrsCard) -> bool {
    is_due(card, js_sys::Date::now() as i64)
}
