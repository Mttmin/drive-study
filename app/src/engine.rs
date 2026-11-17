use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use drive_study_core::{
    choices_readout, clip_key, is_due, review_card, ExamId, ItemKind, ReviewOutcome, SessionItem, SessionMode, SessionPlan, SrsCard, SrsMeta, VOICE_ID,
};
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use rexie::{ObjectStore, Rexie, TransactionMode};
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
    pub voice_uri: Option<String>,
    pub seen_intro: bool,
    pub completed_lessons: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            plan: "BOTH".into(),
            answer_mode: "spoken".into(),
            voice_source: "downloaded".into(),
            rate: 1.0,
            pause_seconds: 6,
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
pub struct ProgressFile {
    pub version: u32,
    pub exported_at: String,
    pub settings: Settings,
    pub srs: Vec<SrsCard>,
    pub attempts: Vec<AttemptRecord>,
    pub days: Vec<DayRecord>,
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
    pub sample: bool,
    pub card_mode: bool,
    pub remaining: Option<u32>,
    pub paused: bool,
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
            sample: false,
            card_mode: false,
            remaining: None,
            paused: false,
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
}

#[derive(Clone)]
pub struct Control {
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    generation: Arc<AtomicU32>,
    pending: Arc<Mutex<Option<Act>>>,
    guessing: Arc<AtomicBool>,
}

impl Control {
    pub fn new() -> Self {
        Self {
            stop: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
            generation: Arc::new(AtomicU32::new(0)),
            pending: Arc::new(Mutex::new(None)),
            guessing: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn interrupt(&self, act: Act) {
        if matches!(act, Act::Pause) {
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
        self.paused.store(false, Ordering::Relaxed);
        resume_audio();
    }

    pub fn set_guessing(&self, value: bool) {
        self.guessing.store(value, Ordering::Relaxed);
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

const SILENCE: &str = "data:audio/wav;base64,UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQAAAAA=";

pub fn arm_audio() {
    let Ok(audio) = web_sys::HtmlAudioElement::new_with_src(SILENCE) else { return };
    audio.set_loop(true);
    let _ = audio.set_volume(0.05);
    let _ = audio.play();
    KEEPER.with(|slot| *slot.borrow_mut() = Some(audio));
    if let Some(synth) = synthesis() {
        if let Ok(utterance) = web_sys::SpeechSynthesisUtterance::new_with_text(" ") {
            utterance.set_volume(0.0);
            synth.speak(&utterance);
        }
    }
}

fn pause_audio() {
    CURRENT.with(|slot| {
        if let Some(audio) = slot.borrow().as_ref() {
            let _ = audio.pause();
        }
    });
    if let Some(synth) = synthesis() {
        if synth.speaking() {
            let _ = synth.pause();
        }
    }
}

fn resume_audio() {
    CURRENT.with(|slot| {
        if let Some(audio) = slot.borrow().as_ref() {
            let _ = audio.play();
        }
    });
    if let Some(synth) = synthesis() {
        let _ = synth.resume();
    }
}

fn stop_audio() {
    CURRENT.with(|slot| {
        if let Some(audio) = slot.borrow().as_ref() {
            let _ = audio.pause();
        }
    });
    if let Some(synth) = synthesis() {
        synth.cancel();
    }
}

pub fn install_media_session(control: Control) {
    let Some(window) = web_sys::window() else { return };
    let Ok(session) = js_sys::Reflect::get(&window.navigator(), &JsValue::from_str("mediaSession")) else { return };
    let Ok(setter) = js_sys::Reflect::get(&session, &JsValue::from_str("setActionHandler")).and_then(|value| value.dyn_into::<js_sys::Function>()) else { return };
    for name in ["play", "pause", "nexttrack", "previoustrack"] {
        let control = control.clone();
        let name_owned = name.to_string();
        let callback = Closure::wrap(Box::new(move || match name_owned.as_str() {
            "play" => control.resume(),
            "pause" => control.interrupt(Act::Pause),
            "nexttrack" => control.interrupt(Act::Skip),
            _ => control.interrupt(Act::Repeat),
        }) as Box<dyn FnMut()>);
        let _ = setter.call2(&session, &JsValue::from_str(name), callback.as_ref().unchecked_ref());
        callback.forget();
    }
}

pub fn acquire_wake_lock() {
    let Some(window) = web_sys::window() else { return };
    let Ok(lock) = js_sys::Reflect::get(&window.navigator(), &JsValue::from_str("wakeLock")) else { return };
    let Ok(request) = js_sys::Reflect::get(&lock, &JsValue::from_str("request")).and_then(|value| value.dyn_into::<js_sys::Function>()) else { return };
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
    manifest: drive_study_core::AudioManifest,
    summary: RwSignal<Option<Summary>>,
) {
    arm_audio();
    install_media_session(control.clone());
    acquire_wake_lock();
    let mut index = 0;
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
    let percent = if graded == 0 { 0.0 } else { (correct as f64 / graded as f64 * 1000.0).round() / 10.0 };
    let passed = plan.pass_percent.and_then(|mark| {
        if responses.is_empty() {
            None
        } else {
            let ratio = correct as f64 / graded.max(1) as f64;
            Some(ratio + 1e-12 >= mark / 100.0)
        }
    });
    if let Some(lesson_id) = plan.lesson_id.clone() {
        if index >= plan.items.len() {
            settings.update(|settings| {
                if !settings.completed_lessons.iter().any(|id| id == &lesson_id) {
                    settings.completed_lessons.push(lesson_id);
                }
            });
            let snapshot = settings.get_untracked();
            let _ = save_settings(&snapshot).await;
        }
    }
    summary.set(Some(Summary {
        title: plan.title,
        correct,
        wrong,
        skipped,
        percent,
        passed,
    }));
    let _ = web_sys::window().unwrap().location().set_hash("#/summary");
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
    manifest: &drive_study_core::AudioManifest,
    model: RwSignal<PlayerModel>,
) -> bool {
    let item = &plan.items[index];
    control.guessing.store(false, Ordering::Relaxed);
    match item {
        SessionItem::Narration { title, text, .. } => {
            set_model(model, plan, index, total, title, text, &[], None, "", false, false, control, correct, wrong);
            speak(text, settings, manifest, control, model).await;
            matches!(control.take(), Some(Act::Repeat))
        }
        SessionItem::Card(card) => {
            set_model(model, plan, index, total, &format!("{} · {}", card.exam.as_str(), card.section), &card.front, &[], None, "", card.sample, true, control, correct, wrong);
            speak(&card.front, settings, manifest, control, model).await;
            if matches!(control.take(), Some(Act::Repeat)) {
                return true;
            }
            let pause = settings.get_untracked().pause_seconds * 1000;
            let arrived = wait_for(control, if plan.listen_only { Some(pause) } else { None }).await;
            let act = control.take().or(arrived);
            match act {
                Some(Act::Repeat) => return true,
                Some(Act::Stop) => {
                    control.stop.store(true, Ordering::Relaxed);
                    return false;
                }
                Some(Act::Skip) | None if plan.listen_only && act.is_none() => {}
                Some(Act::Know) => {
                    grade(cards, &card.id, ItemKind::Flashcard, card.exam, &card.section, None, ReviewOutcome::Correct, false, plan);
                    *correct += 1;
                }
                Some(Act::DontKnow) => {
                    grade(cards, &card.id, ItemKind::Flashcard, card.exam, &card.section, None, ReviewOutcome::Wrong, false, plan);
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
            model.set(shown);
            speak(&card.spoken, settings, manifest, control, model).await;
            false
        }
        SessionItem::Question(question) => {
            let choices: Vec<(String, String)> = question.choices.iter().map(|choice| (choice.key.clone(), choice.text.clone())).collect();
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
                false,
                control,
                correct,
                wrong,
            );
            speak(&question.spoken_stem, settings, manifest, control, model).await;
            if handle_early(control, question, plan, cards, correct, wrong, skipped, responses, model).await {
                return finish_early(question, settings, manifest, control, model).await;
            }
            let readout = choices_readout(&question.kind, &choices);
            speak(&readout, settings, manifest, control, model).await;
            if handle_early(control, question, plan, cards, correct, wrong, skipped, responses, model).await {
                return finish_early(question, settings, manifest, control, model).await;
            }
            let pause = settings.get_untracked().pause_seconds * 1000;
            let limit = if plan.listen_only { Some(pause) } else { None };
            let _ = wait_for(control, limit).await;
            let act = control.take();
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
                    speak(&question.spoken_explanation, settings, manifest, control, model).await;
                    hold_explanation(control).await;
                    false
                }
                Some(Act::Explain) | Some(Act::DontKnow) | None => {
                    grade(cards, &question.id, ItemKind::Question, question.exam, &question.section, Some(question.topic.clone()), ReviewOutcome::Wrong, false, plan);
                    *wrong += 1;
                    responses.push((question.section.clone(), false));
                    reveal(model, question, None, control);
                    speak(&question.spoken_explanation, settings, manifest, control, model).await;
                    hold_explanation(control).await;
                    false
                }
                Some(Act::Answer(key)) => {
                    score_answer(question, &key, control, cards, plan, correct, wrong, responses, model);
                    speak(&question.spoken_explanation, settings, manifest, control, model).await;
                    hold_explanation(control).await;
                    false
                }
                _ => false,
            }
        }
    }
}

impl Control {
    fn peek_repeat(&self) -> bool {
        matches!(self.pending.lock().expect("pending").as_ref(), Some(Act::Repeat))
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
        speak(&question.spoken_explanation, settings, manifest, control, model).await;
        hold_explanation(control).await;
    }
    false
}

async fn hold_explanation(control: &Control) {
    for _ in 0..8 {
        if control.stop.load(Ordering::Relaxed) || control.pending.lock().expect("pending").is_some() {
            return;
        }
        TimeoutFuture::new(100).await;
    }
}

async fn handle_early(
    control: &Control,
    question: &drive_study_core::Question,
    plan: &SessionPlan,
    cards: RwSignal<Vec<SrsCard>>,
    correct: &mut u32,
    wrong: &mut u32,
    skipped: &mut u32,
    responses: &mut Vec<(String, bool)>,
    model: RwSignal<PlayerModel>,
) -> bool {
    let Some(act) = control.take() else { return false };
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
        Act::Explain | Act::DontKnow => {
            grade(cards, &question.id, ItemKind::Question, question.exam, &question.section, Some(question.topic.clone()), ReviewOutcome::Wrong, false, plan);
            *wrong += 1;
            responses.push((question.section.clone(), false));
            reveal(model, question, None, control);
            true
        }
        Act::Answer(key) => {
            score_answer(question, &key, control, cards, plan, correct, wrong, responses, model);
            true
        }
        _ => false,
    }
}

fn score_answer(
    question: &drive_study_core::Question,
    key: &str,
    control: &Control,
    cards: RwSignal<Vec<SrsCard>>,
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
    grade(cards, &question.id, ItemKind::Question, question.exam, &question.section, Some(question.topic.clone()), outcome, guessed && right, plan);
    if right {
        *correct += 1;
    } else {
        *wrong += 1;
    }
    responses.push((question.section.clone(), right));
    reveal(model, question, Some(key.to_string()), control);
}

fn reveal(model: RwSignal<PlayerModel>, question: &drive_study_core::Question, selected: Option<String>, control: &Control) {
    let mut shown = model.get_untracked();
    shown.answer_key = Some(question.answer.clone());
    shown.selected = selected.clone();
    shown.explanation = question.explanation.clone();
    shown.guessing = control.guessing.load(Ordering::Relaxed);
    shown.status = if selected.as_deref() == Some(question.answer.as_str()) {
        if shown.guessing { "Marked as a guess. It will come back.".into() } else { "Correct.".into() }
    } else if selected.is_some() {
        "Not this one.".into()
    } else {
        "Here is the answer.".into()
    };
    model.set(shown);
}

fn grade(
    cards: RwSignal<Vec<SrsCard>>,
    id: &str,
    kind: ItemKind,
    exam: ExamId,
    section: &str,
    topic: Option<String>,
    outcome: ReviewOutcome,
    guessed: bool,
    plan: &SessionPlan,
) {
    let meta = SrsMeta { id: id.into(), kind, exam, section: section.into() };
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
    leptos::task::spawn_local(async move {
        let _ = save_card(&next).await;
        let _ = save_attempt(&attempt).await;
        let _ = touch_day(&today).await;
    });
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
        status: if plan.listen_only { "Answers after the pause. You can tap sooner.".into() } else { "Listening, or tap an answer.".into() },
        source: model.get_untracked().source,
        correct: *correct,
        wrong: *wrong,
        guessing: control.guessing.load(Ordering::Relaxed),
        sample,
        card_mode,
        remaining: plan.limit_seconds,
        paused: control.paused.load(Ordering::Relaxed),
    });
}

async fn wait_for(control: &Control, ms: Option<u32>) -> Option<Act> {
    let mut left = ms.unwrap_or(u32::MAX);
    let endless = ms.is_none();
    while endless || left > 0 {
        if control.stop.load(Ordering::Relaxed) || control.pending.lock().expect("pending").is_some() {
            return None;
        }
        if !control.paused.load(Ordering::Relaxed) && !endless {
            let step = 80.min(left);
            TimeoutFuture::new(step).await;
            left = left.saturating_sub(step);
        } else {
            TimeoutFuture::new(80).await;
        }
    }
    None
}

async fn speak(text: &str, settings: RwSignal<Settings>, manifest: &drive_study_core::AudioManifest, control: &Control, model: RwSignal<PlayerModel>) {
    let generation = control.generation.load(Ordering::Relaxed);
    let prefs = settings.get_untracked();
    let key = clip_key(VOICE_ID, text);
    if prefs.voice_source != "phone" {
        if let Some(clip) = manifest.clips.get(&key) {
            if let Some(url) = resolve_clip(&clip.path, &prefs.voice_source).await {
                if play_url(&url, prefs.rate, control, generation).await {
                    let mut shown = model.get_untracked();
                    shown.source = if url.starts_with("blob:") { "Downloaded clip".into() } else { "Streamed clip".into() };
                    model.set(shown);
                    return;
                }
            }
        }
    }
    if !control.live(generation) {
        return;
    }
    let mut shown = model.get_untracked();
    shown.source = "Phone voice".into();
    model.set(shown);
    speak_phone(text, prefs.rate, prefs.voice_uri.as_deref(), control, generation).await;
}

async fn resolve_clip(path: &str, mode: &str) -> Option<String> {
    let url = absolute_url(path)?;
    let cached = cached_blob(&url).await;
    let online = web_sys::window().map(|window| window.navigator().on_line()).unwrap_or(false);
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
    web_sys::Url::new_with_base(path, &href).ok().map(|url| url.href())
}

async fn cached_blob(url: &str) -> Option<String> {
    let window = web_sys::window()?;
    let caches = window.caches().ok()?;
    let cache = JsFuture::from(caches.open("drive-study-audio-v1")).await.ok()?;
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

async fn play_url(url: &str, rate: f64, control: &Control, generation: u32) -> bool {
    let Ok(audio) = web_sys::HtmlAudioElement::new_with_src(url) else { return false };
    audio.set_playback_rate(rate);
    CURRENT.with(|slot| *slot.borrow_mut() = Some(audio.clone()));
    let Ok(promise) = audio.play() else { return false };
    if JsFuture::from(promise).await.is_err() {
        return false;
    }
    let started = js_sys::Date::now();
    while !audio.ended() && js_sys::Date::now() - started < 120_000.0 {
        if !control.live(generation) {
            let _ = audio.pause();
            return false;
        }
        if control.paused.load(Ordering::Relaxed) {
            let _ = audio.pause();
            while control.paused.load(Ordering::Relaxed) && control.live(generation) {
                TimeoutFuture::new(80).await;
            }
            if control.live(generation) {
                let _ = audio.play();
            }
        }
        TimeoutFuture::new(40).await;
    }
    control.live(generation) || audio.ended()
}

async fn speak_phone(text: &str, rate: f64, voice_uri: Option<&str>, control: &Control, generation: u32) {
    let Some(synth) = synthesis() else {
        TimeoutFuture::new(400).await;
        return;
    };
    let Ok(utterance) = web_sys::SpeechSynthesisUtterance::new_with_text(text) else { return };
    utterance.set_rate(rate as f32);
    if let Some(uri) = voice_uri {
        let voices = synth.get_voices();
        for index in 0..voices.length() {
            if let Ok(voice) = voices.get(index).dyn_into::<web_sys::SpeechSynthesisVoice>() {
                if voice.voice_uri() == uri {
                    utterance.set_voice(Some(&voice));
                }
            }
        }
    }
    let done = Arc::new(AtomicBool::new(false));
    let flag = done.clone();
    let on_end = Closure::wrap(Box::new(move || flag.store(true, Ordering::Relaxed)) as Box<dyn FnMut()>);
    utterance.set_onend(Some(on_end.as_ref().unchecked_ref()));
    synth.cancel();
    synth.speak(&utterance);
    let started = js_sys::Date::now();
    while !done.load(Ordering::Relaxed) && js_sys::Date::now() - started < 90_000.0 {
        if !control.live(generation) {
            synth.cancel();
            break;
        }
        TimeoutFuture::new(40).await;
    }
}

pub fn today_key() -> String {
    let date = js_sys::Date::new_0();
    format!("{:04}-{:02}-{:02}", date.get_full_year() as i32, date.get_month() as i32 + 1, date.get_date() as i32)
}

async fn open_db() -> Result<Rexie, String> {
    Rexie::builder("drive-study")
        .version(1)
        .add_object_store(ObjectStore::new("settings"))
        .add_object_store(ObjectStore::new("srs").key_path("id"))
        .add_object_store(ObjectStore::new("attempts").auto_increment(true))
        .add_object_store(ObjectStore::new("days"))
        .build()
        .await
        .map_err(|err| format!("{err:?}"))
}

pub async fn load_state() -> (Settings, Vec<SrsCard>, Vec<AttemptRecord>, Vec<String>) {
    let Ok(db) = open_db().await else { return (Settings::default(), Vec::new(), Vec::new(), Vec::new()) };
    let Ok(tx) = db.transaction(&["settings", "srs", "attempts", "days"], TransactionMode::ReadOnly) else {
        return (Settings::default(), Vec::new(), Vec::new(), Vec::new());
    };
    let settings = match tx.store("settings").ok().and_then(|store| Some(store)) {
        Some(store) => match store.get(JsValue::from_str("app")).await.ok().flatten() {
            Some(value) => serde_wasm_bindgen::from_value(value).unwrap_or_default(),
            None => Settings::default(),
        },
        None => Settings::default(),
    };
    let cards = read_all(&tx, "srs").await;
    let attempts = read_all(&tx, "attempts").await;
    let days: Vec<DayRecord> = read_all(&tx, "days").await;
    let _ = tx.done().await;
    (settings, cards, attempts, days.into_iter().map(|day| day.date).collect())
}

async fn read_all<T: for<'de> Deserialize<'de>>(tx: &rexie::Transaction, store: &str) -> Vec<T> {
    let Ok(store) = tx.store(store) else { return Vec::new() };
    let Ok(values) = store.get_all(None, None).await else { return Vec::new() };
    values.into_iter().filter_map(|value| serde_wasm_bindgen::from_value(value).ok()).collect()
}

pub async fn save_settings(settings: &Settings) -> Result<(), String> {
    let db = open_db().await?;
    let tx = db.transaction(&["settings"], TransactionMode::ReadWrite).map_err(|err| format!("{err:?}"))?;
    let store = tx.store("settings").map_err(|err| format!("{err:?}"))?;
    let value = serde_wasm_bindgen::to_value(settings).map_err(|err| err.to_string())?;
    store.put(&value, Some(&JsValue::from_str("app"))).await.map_err(|err| format!("{err:?}"))?;
    tx.done().await.map(|_| ()).map_err(|err| format!("{err:?}"))
}

async fn save_card(card: &SrsCard) -> Result<(), String> {
    let db = open_db().await?;
    let tx = db.transaction(&["srs"], TransactionMode::ReadWrite).map_err(|err| format!("{err:?}"))?;
    let store = tx.store("srs").map_err(|err| format!("{err:?}"))?;
    let value = serde_wasm_bindgen::to_value(card).map_err(|err| err.to_string())?;
    store.put(&value, None).await.map_err(|err| format!("{err:?}"))?;
    tx.done().await.map(|_| ()).map_err(|err| format!("{err:?}"))
}

async fn save_attempt(attempt: &AttemptRecord) -> Result<(), String> {
    let db = open_db().await?;
    let tx = db.transaction(&["attempts"], TransactionMode::ReadWrite).map_err(|err| format!("{err:?}"))?;
    let store = tx.store("attempts").map_err(|err| format!("{err:?}"))?;
    let value = serde_wasm_bindgen::to_value(attempt).map_err(|err| err.to_string())?;
    store.add(&value, None).await.map_err(|err| format!("{err:?}"))?;
    tx.done().await.map(|_| ()).map_err(|err| format!("{err:?}"))
}

async fn touch_day(date: &str) -> Result<(), String> {
    let db = open_db().await?;
    let tx = db.transaction(&["days"], TransactionMode::ReadWrite).map_err(|err| format!("{err:?}"))?;
    let store = tx.store("days").map_err(|err| format!("{err:?}"))?;
    let current: DayRecord = match store.get(JsValue::from_str(date)).await.ok().flatten() {
        Some(value) => serde_wasm_bindgen::from_value(value).unwrap_or(DayRecord { date: date.into(), count: 0 }),
        None => DayRecord { date: date.into(), count: 0 },
    };
    let next = DayRecord { date: date.into(), count: current.count + 1 };
    let value = serde_wasm_bindgen::to_value(&next).map_err(|err| err.to_string())?;
    store.put(&value, Some(&JsValue::from_str(date))).await.map_err(|err| format!("{err:?}"))?;
    tx.done().await.map(|_| ()).map_err(|err| format!("{err:?}"))
}

pub async fn export_progress(settings: &Settings) -> ProgressFile {
    let (_, cards, attempts, dates) = load_state().await;
    ProgressFile {
        version: 1,
        exported_at: js_sys::Date::new_0().to_iso_string().into(),
        settings: settings.clone(),
        srs: cards,
        attempts,
        days: dates.into_iter().map(|date| DayRecord { date, count: 1 }).collect(),
    }
}

pub async fn import_progress(file: &ProgressFile) -> Result<(), String> {
    if file.version != 1 {
        return Err("That file is not a Drive Study progress export.".into());
    }
    let db = open_db().await?;
    let tx = db.transaction(&["settings", "srs", "attempts", "days"], TransactionMode::ReadWrite).map_err(|err| format!("{err:?}"))?;
    let settings_store = tx.store("settings").map_err(|err| format!("{err:?}"))?;
    let value = serde_wasm_bindgen::to_value(&file.settings).map_err(|err| err.to_string())?;
    settings_store.put(&value, Some(&JsValue::from_str("app"))).await.map_err(|err| format!("{err:?}"))?;
    let srs = tx.store("srs").map_err(|err| format!("{err:?}"))?;
    srs.clear().await.map_err(|err| format!("{err:?}"))?;
    for card in &file.srs {
        let value = serde_wasm_bindgen::to_value(card).map_err(|err| err.to_string())?;
        srs.put(&value, None).await.map_err(|err| format!("{err:?}"))?;
    }
    let attempts = tx.store("attempts").map_err(|err| format!("{err:?}"))?;
    attempts.clear().await.map_err(|err| format!("{err:?}"))?;
    for attempt in &file.attempts {
        let value = serde_wasm_bindgen::to_value(attempt).map_err(|err| err.to_string())?;
        attempts.put(&value, None).await.map_err(|err| format!("{err:?}"))?;
    }
    let days = tx.store("days").map_err(|err| format!("{err:?}"))?;
    days.clear().await.map_err(|err| format!("{err:?}"))?;
    for day in &file.days {
        let value = serde_wasm_bindgen::to_value(day).map_err(|err| err.to_string())?;
        days.put(&value, Some(&JsValue::from_str(&day.date))).await.map_err(|err| format!("{err:?}"))?;
    }
    tx.done().await.map(|_| ()).map_err(|err| format!("{err:?}"))
}

pub async fn download_urls(urls: &[String]) -> Result<(), String> {
    let window = web_sys::window().ok_or("no window")?;
    let caches = window.caches().map_err(|_| "This browser cannot store audio offline.")?;
    let cache = JsFuture::from(caches.open("drive-study-audio-v1")).await.map_err(|_| "Could not open the audio cache.")?;
    let cache: web_sys::Cache = cache.dyn_into().map_err(|_| "cache")?;
    for url in urls {
        let response = JsFuture::from(window.fetch_with_str(url)).await.map_err(|_| format!("Could not fetch {url}"))?;
        let response: web_sys::Response = response.dyn_into().map_err(|_| "response")?;
        if !response.ok() {
            return Err(format!("Missing audio file ({})", response.status()));
        }
        JsFuture::from(cache.put_with_str(url, &response)).await.map_err(|_| format!("Could not save {url}"))?;
    }
    Ok(())
}

pub async fn delete_urls(urls: &[String]) {
    let Some(window) = web_sys::window() else { return };
    let Ok(caches) = window.caches() else { return };
    let Ok(cache) = JsFuture::from(caches.open("drive-study-audio-v1")).await else { return };
    let Ok(cache) = cache.dyn_into::<web_sys::Cache>() else { return };
    for url in urls {
        let _ = JsFuture::from(cache.delete_with_str(url)).await;
    }
}

pub fn register_service_worker() {
    let Some(window) = web_sys::window() else { return };
    let container = window.navigator().service_worker();
    let _ = container.register("sw.js");
}

pub fn is_due_card(card: &SrsCard) -> bool {
    is_due(card, js_sys::Date::now() as i64)
}
