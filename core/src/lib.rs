//! Study rules shared by the WebAssembly app and the native tests.
//! The audio key matches `shared/audioKey.js` byte for byte.

mod audio;
mod blueprint;
mod content;
mod grammar;
mod scoring;
mod session;
mod srs;

pub use audio::{choices_readout, clip_key, clip_path, normalize_spoken, VOICE_ID};
pub use blueprint::{allocate_questions, draw_weighted, shuffle, DrawItem, WeightedSection};
pub use content::{validate_repo, AudioManifest, ClipEntry, ExamBlueprint, ExamId, Flashcard, Lesson, Library, Meta, Question, SectionBlueprint, Segment};
pub use grammar::{answer_grammar, parse_voice, ParseOptions, VoiceCommand};
pub use scoring::{readiness, score_exam, weak_topics, ExamScore, Readiness, ReadinessAttempt};
pub use session::{
    build_drive, build_lesson, build_mistakes, build_mock, build_quiz, build_rapid, PlanId, SessionItem, SessionMode, SessionPlan,
};
pub use srs::{is_due, next_ef, review_card, streak_from_dates, ItemKind, ReviewOutcome, SrsCard, SrsMeta};
