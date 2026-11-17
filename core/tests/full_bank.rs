use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use drive_study_core::{build_mock, ExamId, Question, SessionItem};

#[test]
fn full_bank_mocks_follow_the_blueprint() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../content");
    let meta: drive_study_core::Meta = serde_json::from_str(&fs::read_to_string(root.join("meta.json")).unwrap()).unwrap();
    let mut questions = Vec::new();
    for exam in ["sie", "s7"] {
        let mut paths: Vec<_> = fs::read_dir(root.join(exam).join("questions")).unwrap().map(|entry| entry.unwrap().path()).collect();
        paths.sort();
        for path in paths {
            let rows: Vec<Question> = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
            questions.extend(rows);
        }
    }
    assert_eq!(questions.iter().filter(|question| question.exam == ExamId::Sie).count(), 600);
    assert_eq!(questions.iter().filter(|question| question.exam == ExamId::S7).count(), 1080);
    for (exam, key, expected) in [(ExamId::Sie, "SIE", 600usize), (ExamId::S7, "S7", 1080)] {
        let blueprint = &meta.exams[key];
        let mut cursor = 0.13_f64;
        let plan = build_mock(exam, true, &questions, blueprint, false, &mut || {
            cursor = (cursor * 1.37) % 1.0;
            cursor
        });
        assert_eq!(plan.items.len(), blueprint.scored_questions as usize);
        assert!(plan.items.len() < expected);
        let mut counts = HashMap::new();
        let mut ids = HashSet::new();
        for item in &plan.items {
            let SessionItem::Question(question) = item else { panic!("a mock is questions") };
            assert!(ids.insert(question.id.clone()), "duplicate {}", question.id);
            *counts.entry(question.section.clone()).or_insert(0usize) += 1;
        }
        for section in &blueprint.sections {
            assert_eq!(counts.get(&section.id).copied().unwrap_or(0), section.questions as usize, "{}", section.id);
        }
    }
}
