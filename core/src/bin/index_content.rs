//! Write content/index.json by scanning content/{sie,s7}.
//! A newer wave is a drop-in replacement of the content directory.
//! Trunk runs this before each build so the site can fetch every file.

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use serde::Serialize;

#[derive(Serialize)]
struct Index {
    lessons: Vec<String>,
    questions: Vec<String>,
    flashcards: Vec<String>,
}

fn main() -> ExitCode {
    let content = std::env::args().nth(1).unwrap_or_else(|| "content".into());
    let root = Path::new(&content);
    if !root.join("meta.json").is_file() {
        eprintln!("No {content}/meta.json");
        return ExitCode::from(1);
    }
    let index = Index {
        lessons: list(root, "lessons"),
        questions: list(root, "questions"),
        flashcards: list(root, "flashcards"),
    };
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
