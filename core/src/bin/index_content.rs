//! Write content/index.json by scanning content/{sie,s7}.
//! A newer wave is a drop-in replacement of the content directory.
//! Trunk runs this before each build so the site can fetch every file.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
struct Index {
    lessons: Vec<String>,
    questions: Vec<String>,
    flashcards: Vec<String>,
    counts: BTreeMap<String, Counts>,
}

#[derive(Default, Serialize)]
struct Counts {
    questions: usize,
    flashcards: usize,
    lessons: usize,
}

fn main() -> ExitCode {
    let content = std::env::args().nth(1).unwrap_or_else(|| "content".into());
    let root = Path::new(&content);
    if !root.join("meta.json").is_file() {
        eprintln!("No {content}/meta.json");
        return ExitCode::from(1);
    }
    let lessons = list(root, "lessons");
    let questions = list(root, "questions");
    let flashcards = list(root, "flashcards");
    let counts = match counts(root, &lessons, &questions, &flashcards) {
        Ok(counts) => counts,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(1);
        }
    };
    let index = Index { lessons, questions, flashcards, counts };
    let path = root.join("index.json");
    let Ok(text) = serde_json::to_string_pretty(&index) else {
        eprintln!("Could not encode the content index");
        return ExitCode::from(1);
    };
    if fs::write(&path, text + "\n").is_err() {
        eprintln!("Could not write {}", path.display());
        return ExitCode::from(1);
    }
    println!(
        "Indexed {} lessons, {} question files, {} flashcard files.",
        index.lessons.len(),
        index.questions.len(),
        index.flashcards.len()
    );
    ExitCode::SUCCESS
}

fn counts(root: &Path, lessons: &[String], questions: &[String], flashcards: &[String]) -> Result<BTreeMap<String, Counts>, String> {
    let mut map = BTreeMap::new();
    tally(root, lessons, &mut map, Kind::Lessons)?;
    tally(root, questions, &mut map, Kind::Questions)?;
    tally(root, flashcards, &mut map, Kind::Flashcards)?;
    Ok(map)
}

enum Kind {
    Lessons,
    Questions,
    Flashcards,
}

fn tally(root: &Path, paths: &[String], map: &mut BTreeMap<String, Counts>, kind: Kind) -> Result<(), String> {
    for path in paths {
        let section = section_of(path).ok_or_else(|| format!("{path} is not a section file"))?;
        let text = fs::read_to_string(root.join(path)).map_err(|_| format!("Could not read {path}"))?;
        let value: Value = serde_json::from_str(&text).map_err(|_| format!("{path} is not JSON"))?;
        let n = if value.is_array() { value.as_array().map(|rows| rows.len()).unwrap_or(0) } else { 1 };
        let row = map.entry(section).or_default();
        match kind {
            Kind::Lessons => row.lessons += n,
            Kind::Questions => row.questions += n,
            Kind::Flashcards => row.flashcards += n,
        }
    }
    Ok(())
}

fn section_of(path: &str) -> Option<String> {
    let name = path.rsplit('/').next()?.trim_end_matches(".json").to_ascii_lowercase();
    let mut parts = name.split('-');
    let exam = parts.next()?;
    let number = parts.next()?;
    if (exam != "sie" && exam != "s7") || !number.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some(format!("{}-{number}", exam.to_ascii_uppercase()))
}

fn list(root: &Path, kind: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for exam in ["sie", "s7"] {
        let dir = root.join(exam).join(kind);
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
