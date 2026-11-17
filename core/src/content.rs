use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::audio::{clip_key, VOICE_ID};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExamId {
    #[serde(rename = "SIE")]
    Sie,
    #[serde(rename = "S7")]
    S7,
}

impl ExamId {
    pub fn as_str(self) -> &'static str {
        match self {
            ExamId::Sie => "SIE",
            ExamId::S7 => "S7",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub key: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub exam: ExamId,
    pub section: String,
    pub topic: String,
    #[serde(default)]
    pub lesson_id: Option<String>,
    pub difficulty: u8,
    #[serde(rename = "type")]
    pub kind: String,
    pub stem: String,
    pub choices: Vec<Choice>,
    pub answer: String,
    pub spoken_stem: String,
    pub explanation: String,
    pub spoken_explanation: String,
    pub tags: Vec<String>,
    pub outline_ref: String,
    #[serde(default)]
    pub sample: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flashcard {
    pub id: String,
    pub exam: ExamId,
    pub section: String,
    pub front: String,
    pub back: String,
    pub spoken: String,
    #[serde(default)]
    pub sample: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Segment {
    Narration { text: String },
    Check { question_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyTerm {
    pub term: String,
    pub definition: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lesson {
    pub id: String,
    pub exam: ExamId,
    pub section: String,
    pub section_title: String,
    pub title: String,
    pub order: u32,
    pub est_minutes: u32,
    pub segments: Vec<Segment>,
    pub key_terms: Vec<KeyTerm>,
    pub mnemonics: Vec<String>,
    pub summary: String,
    #[serde(default)]
    pub sample: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionBlueprint {
    pub id: String,
    pub title: String,
    pub weight: f64,
    pub questions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExamBlueprint {
    pub name: String,
    #[serde(rename = "scored_questions", alias = "questions")]
    pub scored_questions: u32,
    #[serde(default)]
    pub unscored_questions: u32,
    #[serde(default)]
    pub total_questions: u32,
    #[serde(rename = "time_limit_minutes", alias = "minutes")]
    pub time_limit_minutes: u32,
    #[serde(rename = "passing_score", alias = "pass_percent")]
    pub passing_score: f64,
    pub sections: Vec<SectionBlueprint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaFiles {
    pub lessons: Vec<String>,
    pub questions: Vec<String>,
    pub flashcards: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub disclaimer: String,
    #[serde(default)]
    pub voice_id: String,
    #[serde(default)]
    pub files: Option<MetaFiles>,
    pub exams: HashMap<String, ExamBlueprint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipEntry {
    pub path: String,
    pub voice: String,
    #[serde(default)]
    pub bytes: Option<u64>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioManifest {
    pub version: u32,
    #[serde(default)]
    pub algorithm: String,
    pub voice_id: String,
    #[serde(default)]
    pub clips: HashMap<String, ClipEntry>,
}

#[derive(Debug, Clone)]
pub struct Library {
    pub meta: Meta,
    pub lessons: Vec<Lesson>,
    pub questions: Vec<Question>,
    pub cards: Vec<Flashcard>,
}

pub fn question_texts(question: &Question) -> Vec<String> {
    vec![
        question.spoken_stem.clone(),
        crate::audio::choices_readout(
            &question.kind,
            &question.choices.iter().map(|c| (c.key.clone(), c.text.clone())).collect::<Vec<_>>(),
        ),
        question.spoken_explanation.clone(),
    ]
}

pub fn flashcard_text(card: &Flashcard) -> String {
    card.spoken.clone()
}

pub fn lesson_texts(lesson: &Lesson) -> Vec<String> {
    lesson
        .segments
        .iter()
        .filter_map(|segment| match segment {
            Segment::Narration { text } => Some(text.clone()),
            Segment::Check { .. } => None,
        })
        .collect()
}

pub fn validate_repo(root: &Path) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let meta_path = root.join("content/meta.json");
    let meta: Meta = match fs::read_to_string(&meta_path).ok().and_then(|text| serde_json::from_str(&text).ok()) {
        Some(meta) => meta,
        None => {
            errors.push("content/meta.json is missing or invalid".into());
            return Err(errors);
        }
    };
    for exam_id in ["SIE", "S7"] {
        let Some(exam) = meta.exams.get(exam_id) else {
            errors.push(format!("meta.exams.{exam_id} missing"));
            continue;
        };
        let weight: f64 = exam.sections.iter().map(|section| section.weight).sum();
        let expected = if weight < 2.0 { 1.0 } else { 100.0 };
        if (weight - expected).abs() > 0.02 {
            errors.push(format!("{exam_id} section weights sum to {weight}, expected {expected}"));
        }
        let counted: u32 = exam.sections.iter().map(|section| section.questions).sum();
        if counted != exam.scored_questions {
            errors.push(format!("{exam_id} section question counts sum to {counted}, expected {}", exam.scored_questions));
        }
    }
    let lesson_paths = content_files(root, "lessons");
    let question_paths = content_files(root, "questions");
    let card_paths = content_files(root, "flashcards");
    if let Some(files) = &meta.files {
        let mut listed = Vec::new();
        listed.extend(files.lessons.iter().cloned());
        listed.extend(files.questions.iter().cloned());
        listed.extend(files.flashcards.iter().cloned());
        listed.sort();
        let mut discovered = Vec::new();
        discovered.extend(lesson_paths.iter().cloned());
        discovered.extend(question_paths.iter().cloned());
        discovered.extend(card_paths.iter().cloned());
        discovered.sort();
        if listed != discovered {
            errors.push("meta.files does not match the JSON files on disk".into());
        }
    }

    let plan_ids = lesson_plan_ids(root);
    let mut lessons = HashMap::new();
    let mut questions = HashMap::new();
    let mut cards = HashMap::new();
    for path in &lesson_paths {
        match read_json::<Lesson>(root, path) {
            Ok(lesson) => {
                check_lesson(path, &lesson, &meta, &mut errors);
                lessons.insert(lesson.id.clone(), lesson);
            }
            Err(message) => errors.push(message),
        }
    }
    for path in &question_paths {
        match read_json::<Vec<Question>>(root, path) {
            Ok(rows) => {
                let file_section = path.rsplit('/').next().unwrap_or("").trim_end_matches(".json");
                for question in rows {
                    check_question(path, file_section, &question, &meta, &mut errors);
                    if questions.contains_key(&question.id) {
                        errors.push(format!("duplicate question id {}", question.id));
                    }
                    questions.insert(question.id.clone(), question);
                }
            }
            Err(message) => errors.push(message),
        }
    }
    for path in &card_paths {
        match read_json::<Vec<Flashcard>>(root, path) {
            Ok(rows) => {
                let file_section = path.rsplit('/').next().unwrap_or("").trim_end_matches(".json");
                for card in rows {
                    check_card(path, file_section, &card, &meta, &mut errors);
                    if cards.contains_key(&card.id) {
                        errors.push(format!("duplicate flashcard id {}", card.id));
                    }
                    cards.insert(card.id.clone(), card);
                }
            }
            Err(message) => errors.push(message),
        }
    }
    for lesson in lessons.values() {
        for segment in &lesson.segments {
            if let Segment::Check { question_id } = segment {
                if !questions.contains_key(question_id) {
                    errors.push(format!("lesson {} check {question_id} does not exist", lesson.id));
                }
            }
        }
    }
    for question in questions.values() {
        match &question.lesson_id {
            Some(lesson_id) if !lesson_id.is_empty() && !lessons.contains_key(lesson_id) && !plan_ids.contains(lesson_id) => {
                errors.push(format!("question {} lesson_id {lesson_id} is not a lesson or a planned lesson", question.id));
            }
            Some(lesson_id) if lesson_id.is_empty() => errors.push(format!("{} missing lesson_id", question.id)),
            None => errors.push(format!("{} missing lesson_id", question.id)),
            _ => {}
        }
    }
    if let Ok(text) = fs::read_to_string(root.join("audio/manifest.json")) {
        if let Ok(manifest) = serde_json::from_str::<AudioManifest>(&text) {
            if manifest.version != 1 {
                errors.push("audio manifest version must be 1".into());
            }
            if manifest.voice_id != VOICE_ID {
                errors.push("audio manifest voice_id must be natural-en-us-v1".into());
            }
            let mut known = HashSet::new();
            for question in questions.values() {
                for spoken in question_texts(question) {
                    known.insert(clip_key(&manifest.voice_id, &spoken));
                }
            }
            for card in cards.values() {
                known.insert(clip_key(&manifest.voice_id, &flashcard_text(card)));
            }
            for lesson in lessons.values() {
                for spoken in lesson_texts(lesson) {
                    known.insert(clip_key(&manifest.voice_id, &spoken));
                }
            }
            let hex = Regex::new(r"^[0-9a-f]{40}$").unwrap();
            for (key, clip) in &manifest.clips {
                if !hex.is_match(key) {
                    errors.push(format!("clip key {key} is not sha1 hex"));
                }
                if clip.path.is_empty() {
                    errors.push(format!("clip {key} missing path"));
                }
                if clip.voice != manifest.voice_id {
                    errors.push(format!("clip {key} voice does not match manifest"));
                }
                let _ = &known;
            }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

fn check_lesson(path: &str, lesson: &Lesson, meta: &Meta, errors: &mut Vec<String>) {
    let folder = if path.contains("/sie/") { "SIE" } else { "S7" };
    if lesson.exam.as_str() != folder {
        errors.push(format!("{} exam does not match folder", lesson.id));
    }
    if !section_known(meta, lesson.exam.as_str(), &lesson.section) {
        errors.push(format!("{} section {} is not in the blueprint", lesson.id, lesson.section));
    }
    if lesson.segments.is_empty() {
        errors.push(format!("{} needs segments", lesson.id));
    }
}

fn check_question(path: &str, file_section: &str, question: &Question, meta: &Meta, errors: &mut Vec<String>) {
    let folder = if path.contains("/sie/") { "SIE" } else { "S7" };
    let pattern = if folder == "SIE" { r"^sie-q-\d{4}$" } else { r"^s7-q-\d{4}$" };
    if !Regex::new(pattern).unwrap().is_match(&question.id) {
        errors.push(format!("bad question id {}", question.id));
    }
    if question.exam.as_str() != folder {
        errors.push(format!("{} exam does not match folder", question.id));
    }
    if question.section != file_section {
        errors.push(format!("{} section does not match file", question.id));
    }
    if !section_known(meta, question.exam.as_str(), &question.section) {
        errors.push(format!("{} unknown section", question.id));
    }
    for (field, value, minimum) in [
        ("topic", question.topic.as_str(), 3),
        ("outline_ref", question.outline_ref.as_str(), 4),
        ("stem", question.stem.as_str(), 12),
        ("spoken_stem", question.spoken_stem.as_str(), 12),
        ("explanation", question.explanation.as_str(), 40),
        ("spoken_explanation", question.spoken_explanation.as_str(), 40),
    ] {
        if value.trim().len() < minimum {
            errors.push(format!("{} weak or missing {field}", question.id));
        }
    }
    if !matches!(question.difficulty, 1 | 2 | 3) {
        errors.push(format!("{} difficulty must be 1, 2, or 3", question.id));
    }
    let keys: String = question.choices.iter().map(|choice| choice.key.as_str()).collect();
    if question.kind == "mc" && keys != "ABCD" {
        errors.push(format!("{} multiple choice must be A B C D in order", question.id));
    } else if question.kind == "tf" && keys != "AB" && keys != "TF" {
        errors.push(format!("{} true/false must be A/B (True, False) or T/F", question.id));
    } else if question.kind != "mc" && question.kind != "tf" {
        errors.push(format!("{} type must be mc or tf", question.id));
    }
    if !question.choices.iter().any(|choice| choice.key == question.answer) {
        errors.push(format!("{} answer is not a choice", question.id));
    }
}

fn check_card(path: &str, file_section: &str, card: &Flashcard, meta: &Meta, errors: &mut Vec<String>) {
    let folder = if path.contains("/sie/") { "SIE" } else { "S7" };
    let pattern = if folder == "SIE" { r"^sie-f-\d{4}$" } else { r"^s7-f-\d{4}$" };
    if !Regex::new(pattern).unwrap().is_match(&card.id) {
        errors.push(format!("bad flashcard id {}", card.id));
    }
    if card.exam.as_str() != folder || card.section != file_section || !section_known(meta, card.exam.as_str(), &card.section) {
        errors.push(format!("{} flashcard does not match its file", card.id));
    }
    if card.front.trim().len() < 4 || card.back.trim().len() < 4 || card.spoken.trim().len() < 4 {
        errors.push(format!("{} weak spoken fields", card.id));
    }
}

fn section_known(meta: &Meta, exam: &str, section: &str) -> bool {
    meta.exams.get(exam).is_some_and(|exam| exam.sections.iter().any(|row| row.id == section))
}

fn read_json<T: serde::de::DeserializeOwned>(root: &Path, path: &str) -> Result<T, String> {
    let text = fs::read_to_string(root.join(path)).map_err(|err| format!("{path}: {err}"))?;
    serde_json::from_str(&text).map_err(|err| format!("{path}: {err}"))
}

fn content_files(root: &Path, kind: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for exam in ["sie", "s7"] {
        let dir = root.join("content").join(exam).join(kind);
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        let mut names: Vec<_> = entries.flatten().map(|entry| entry.path()).filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json")).collect();
        names.sort();
        for path in names {
            if let Ok(rel) = path.strip_prefix(root) {
                paths.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    paths
}

fn lesson_plan_ids(root: &Path) -> HashSet<String> {
    let pattern = Regex::new(r"\b((?:sie|s7)-\d+-\d+)\b").unwrap();
    let mut ids = HashSet::new();
    for exam in ["sie", "s7"] {
        let path = root.join("content").join(exam).join("LESSON_PLAN.md");
        let Ok(text) = fs::read_to_string(path) else { continue };
        for found in pattern.captures_iter(&text) {
            ids.insert(found[1].to_string());
        }
    }
    ids
}

