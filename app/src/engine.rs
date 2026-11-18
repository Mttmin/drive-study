use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use drive_study_core::{
    choices_readout, clip_key, is_due, parse_voice, review_card, ExamId, ItemKind, ParseOptions,
    ReviewOutcome, SessionItem, SessionMode, SessionPlan, SrsCard, SrsMeta, VoiceCommand, VOICE_ID,
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
        if matches!(act, Act::MoreTime) {
            let mut slot = self.pending.lock().expect("pending");
            if slot.is_none() {
                *slot = Some(Act::MoreTime);
            }
            return;
        }
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

pub fn arm_audio() {
    let Ok(audio) = web_sys::HtmlAudioElement::new_with_src(SILENCE) else {
        return;
    };
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
            "pause" => control.interrupt(Act::Pause),
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
                    None,
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
            let readout = choices_readout(&question.kind, &choices);
            speak(&readout, settings, manifest, control, model).await;
            if handle_early(
                control, question, plan, cards, attempts, days, correct, wrong, skipped, responses,
                model,
            )
            .await
            {
                return finish_early(question, settings, manifest, control, model).await;
            }
            let act = answer_window(
                control,
                window_ms(&settings.get_untracked(), plan.listen_only),
                model,
                Some(question),
            )
            .await;
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
                Some(Act::Explain) | Some(Act::DontKnow) | None => {
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
        settle(control, 900).await;
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
    settle(control, 1200).await;
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
        Act::Explain | Act::DontKnow => {
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
    });
}

async fn answer_window(
    control: &Control,
    ms: u32,
    model: RwSignal<PlayerModel>,
    question: Option<&drive_study_core::Question>,
) -> Option<Act> {
    chime();
    let mut left = ms;
    let mut listening = false;
    let opened = js_sys::Date::now();
    while left > 0 {
        let seconds = left.div_ceil(1000);
        model.update(|item| item.window_left = Some(seconds));
        if !listening && js_sys::Date::now() - opened >= 650.0 {
            if let Some(question) = question {
                start_listen(question, control.clone());
            }
            listening = true;
        }
        if let Some(act) = control.take() {
            match act {
                Act::MoreTime => {
                    left = left.saturating_add(10_000);
                    chime();
                    continue;
                }
                other => {
                    model.update(|item| item.window_left = None);
                    stop_listen();
                    return Some(other);
                }
            }
        }
        if control.stop.load(Ordering::Relaxed) {
            model.update(|item| item.window_left = None);
            stop_listen();
            return Some(Act::Stop);
        }
        let step = 200.min(left);
        TimeoutFuture::new(step).await;
        if !control.paused.load(Ordering::Relaxed) {
            left = left.saturating_sub(step);
        }
    }
    model.update(|item| item.window_left = None);
    stop_listen();
    None
}

fn chime() {
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

async fn speak(
    text: &str,
    settings: RwSignal<Settings>,
    manifest: &drive_study_core::AudioManifest,
    control: &Control,
    model: RwSignal<PlayerModel>,
) {
    let generation = control.generation.load(Ordering::Relaxed);
    let prefs = settings.get_untracked();
    let key = clip_key(VOICE_ID, text);
    if prefs.voice_source != "phone" {
        if let Some(clip) = manifest.clips.get(&key) {
            if let Some(url) = resolve_clip(&clip.path, &prefs.voice_source).await {
                if play_url(
                    &url,
                    prefs.rate,
                    control,
                    generation,
                    utterance_budget_ms(text, prefs.rate),
                )
                .await
                {
                    let mut shown = model.get_untracked();
                    shown.source = if url.starts_with("blob:") {
                        "Downloaded clip".into()
                    } else {
                        "Streamed clip".into()
                    };
                    model.set(shown);
                    return;
                }
                if control.has_pending() || !control.live(generation) {
                    return;
                }
            }
        }
    }
    if !control.live(generation) || control.has_pending() {
        return;
    }
    let mut shown = model.get_untracked();
    shown.source = match prefs.voice_source.as_str() {
        "phone" => "Phone voice".into(),
        "downloaded" => "Phone voice. No downloaded clip for this line yet.".into(),
        _ => "Phone voice. No streamed clip for this line yet.".into(),
    };
    model.set(shown);
    speak_phone(
        text,
        prefs.rate,
        prefs.voice_uri.as_deref(),
        control,
        generation,
    )
    .await;
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

async fn play_url(
    url: &str,
    rate: f64,
    control: &Control,
    generation: u32,
    budget_ms: f64,
) -> bool {
    let Ok(audio) = web_sys::HtmlAudioElement::new_with_src(url) else {
        return false;
    };
    audio.set_playback_rate(rate);
    CURRENT.with(|slot| *slot.borrow_mut() = Some(audio.clone()));
    let Ok(promise) = audio.play() else {
        return false;
    };
    let started = js_sys::Date::now();
    let play = JsFuture::from(promise);
    let timeout = TimeoutFuture::new(2_500);
    futures::pin_mut!(play, timeout);
    if let futures::future::Either::Left((Ok(_), _)) = futures::future::select(play, timeout).await
    {
    } else {
        let _ = audio.pause();
        return false;
    }
    while !audio.ended() {
        if !control.live(generation) || control.has_pending() {
            let _ = audio.pause();
            return false;
        }
        let elapsed = js_sys::Date::now() - started;
        let duration_ms = audio.duration();
        let cap = if duration_ms.is_finite() && duration_ms > 0.0 {
            duration_ms * 1000.0 / rate.max(0.8) + 1_500.0
        } else {
            budget_ms
        };
        if elapsed > cap {
            let _ = audio.pause();
            break;
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
    audio.current_time() > 0.05 || audio.ended()
}

async fn speak_phone(
    text: &str,
    rate: f64,
    voice_uri: Option<&str>,
    control: &Control,
    generation: u32,
) {
    let Some(synth) = synthesis() else { return };
    if synth.get_voices().length() == 0 {
        return;
    }
    let Ok(utterance) = web_sys::SpeechSynthesisUtterance::new_with_text(text) else {
        return;
    };
    utterance.set_rate(rate as f32);
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
    let done = Arc::new(AtomicBool::new(false));
    let flag = done.clone();
    let on_end =
        Closure::wrap(Box::new(move || flag.store(true, Ordering::Relaxed)) as Box<dyn FnMut()>);
    utterance.set_onend(Some(on_end.as_ref().unchecked_ref()));
    let flag_error = done.clone();
    let on_error = Closure::wrap(
        Box::new(move || flag_error.store(true, Ordering::Relaxed)) as Box<dyn FnMut()>
    );
    utterance.set_onerror(Some(on_error.as_ref().unchecked_ref()));
    synth.cancel();
    synth.speak(&utterance);
    let started = js_sys::Date::now();
    let budget = utterance_budget_ms(text, rate);
    while !done.load(Ordering::Relaxed) {
        if !control.live(generation) || control.has_pending() {
            synth.cancel();
            break;
        }
        if js_sys::Date::now() - started >= budget {
            synth.cancel();
            break;
        }
        TimeoutFuture::new(40).await;
    }
}

thread_local! {
    static REC: std::cell::RefCell<Option<JsValue>> = const { std::cell::RefCell::new(None) };
}

fn stop_listen() {
    REC.with(|slot| {
        if let Some(rec) = slot.borrow_mut().take() {
            if let Ok(abort) = js_sys::Reflect::get(&rec, &"abort".into())
                .and_then(|value| value.dyn_into::<js_sys::Function>())
            {
                let _ = abort.call0(&rec);
            }
        }
    });
}

fn start_listen(question: &drive_study_core::Question, control: Control) {
    stop_listen();
    let Some(window) = web_sys::window() else {
        return;
    };
    let ctor = ["webkitSpeechRecognition", "SpeechRecognition"]
        .into_iter()
        .find_map(|name| {
            js_sys::Reflect::get(&window, &JsValue::from_str(name))
                .ok()
                .filter(|value| value.is_function())
        });
    let Some(ctor) = ctor.and_then(|value| value.dyn_into::<js_sys::Function>().ok()) else {
        return;
    };
    let Ok(rec) = js_sys::Reflect::construct(&ctor, &js_sys::Array::new()) else {
        return;
    };
    let _ = js_sys::Reflect::set(&rec, &"lang".into(), &"en-US".into());
    let _ = js_sys::Reflect::set(&rec, &"interimResults".into(), &JsValue::FALSE);
    let _ = js_sys::Reflect::set(&rec, &"maxAlternatives".into(), &JsValue::from(5));
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
    let on_result = Closure::wrap(Box::new(move |event: JsValue| {
        let Ok(results) = js_sys::Reflect::get(&event, &"results".into()) else {
            return;
        };
        let results = js_sys::Array::from(&results);
        if results.length() == 0 {
            return;
        }
        let alternatives = js_sys::Array::from(&results.get(results.length() - 1));
        for index in 0..alternatives.length() {
            let Ok(transcript) =
                js_sys::Reflect::get(&alternatives.get(index), &"transcript".into())
            else {
                continue;
            };
            let Some(transcript) = transcript.as_string() else {
                continue;
            };
            match parse_voice(&transcript, &options) {
                VoiceCommand::Answer(key) => control.interrupt(Act::Answer(key)),
                VoiceCommand::MoreTime => control.interrupt(Act::MoreTime),
                VoiceCommand::Repeat => control.interrupt(Act::Repeat),
                VoiceCommand::Skip => control.interrupt(Act::Skip),
                VoiceCommand::Explain => control.interrupt(Act::Explain),
                VoiceCommand::DontKnow => control.interrupt(Act::DontKnow),
                VoiceCommand::Pause => control.interrupt(Act::Pause),
                VoiceCommand::Stop => control.interrupt(Act::Stop),
                _ => continue,
            }
            return;
        }
    }) as Box<dyn FnMut(JsValue)>);
    let _ = js_sys::Reflect::set(&rec, &"onresult".into(), on_result.as_ref());
    on_result.forget();
    if let Ok(start) = js_sys::Reflect::get(&rec, &"start".into())
        .and_then(|value| value.dyn_into::<js_sys::Function>())
    {
        let _ = start.call0(&rec);
    }
    REC.with(|slot| *slot.borrow_mut() = Some(rec));
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
