use drive_study_core::{clip_key, clip_path, validate_repo, VOICE_ID};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
struct Fixture {
    text: String,
    key: String,
    path: String,
}

#[test]
fn hashes_match_the_live_audio_manifest() {
    let fixtures: Vec<Fixture> = serde_json::from_str(include_str!("fixtures/live-clips.json")).unwrap();
    assert!(fixtures.len() >= 5, "need several clips from the published manifest");
    for fixture in fixtures {
        assert_eq!(clip_key(VOICE_ID, &fixture.text), fixture.key, "{}", fixture.path);
        assert_eq!(clip_path(&fixture.key), fixture.path);
    }
}

#[test]
fn sample_content_passes_validation() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    if let Err(errors) = validate_repo(&root) {
        panic!("{}", errors.join("\n"));
    }
}
