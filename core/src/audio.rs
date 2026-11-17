use sha1::{Digest, Sha1};
use unicode_normalization::UnicodeNormalization;

pub const VOICE_ID: &str = "natural-en-us-v1";

/// NFC, then collapse tab, newline, vertical tab, form feed, carriage return,
/// ASCII space, and NBSP into a single space, then trim.
pub fn normalize_spoken(text: &str) -> String {
    let nfc: String = text.nfc().collect();
    let mut out = String::with_capacity(nfc.len());
    let mut pending = false;
    for ch in nfc.chars() {
        if matches!(ch, '\t' | '\n' | '\u{000b}' | '\u{000c}' | '\r' | ' ' | '\u{00a0}') {
            pending = true;
            continue;
        }
        if pending && !out.is_empty() {
            out.push(' ');
        }
        pending = false;
        out.push(ch);
    }
    out
}

pub fn clip_key(voice_id: &str, text: &str) -> String {
    let payload = format!("{voice_id}\n{}", normalize_spoken(text));
    let digest = Sha1::digest(payload.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn clip_path(key: &str) -> String {
    let shard = key.get(..2).unwrap_or("");
    format!("audio/clips/{shard}/{key}.mp3")
}

pub fn choices_readout(kind: &str, choices: &[(String, String)]) -> String {
    if kind == "tf" {
        return "True, or false.".to_string();
    }
    choices
        .iter()
        .map(|(key, text)| {
            let mut text = normalize_spoken(text);
            if !text.is_empty() && !text.ends_with(['.', '!', '?']) {
                text.push('.');
            }
            format!("{key}. {text}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_vector_and_whitespace() {
        let key = clip_key(VOICE_ID, "Hello, Drive Study.");
        assert_eq!(key, "1335a8d460eaaaf2cc829487fb6ea1b421f7f7cb");
        assert_eq!(clip_key(VOICE_ID, "Hello,   Drive Study.\n"), key);
        assert_eq!(normalize_spoken("a\u{00a0}b\n\nc"), "a b c");
        assert_eq!(
            choices_readout(
                "mc",
                &[
                    ("A".into(), "Common stock".into()),
                    ("B".into(), "Preferred stock.".into())
                ]
            ),
            "A. Common stock. B. Preferred stock."
        );
        assert_eq!(choices_readout("tf", &[]), "True, or false.");
        assert_eq!(
            clip_path("1335a8d460eaaaf2cc829487fb6ea1b421f7f7cb"),
            "audio/clips/13/1335a8d460eaaaf2cc829487fb6ea1b421f7f7cb.mp3"
        );
    }
}
