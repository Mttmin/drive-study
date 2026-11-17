use regex::Regex;
use std::collections::{HashMap, HashSet};
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceCommand {
    Answer(String),
    Repeat,
    Skip,
    Explain,
    DontKnow,
    Pause,
    Stop,
    Know,
    Unrecognized,
}

#[derive(Debug, Clone)]
pub struct ParseOptions {
    pub choice_keys: Vec<String>,
    pub allow_know: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            choice_keys: vec!["A".into(), "B".into(), "C".into(), "D".into()],
            allow_know: false,
        }
    }
}

pub fn answer_grammar(choice_keys: &[String]) -> String {
    let allowed: HashSet<&str> = choice_keys.iter().map(String::as_str).collect();
    let strong = strong_map();
    let letter = letter_map();
    let tokens = [
        "a", "b", "c", "d", "alpha", "alfa", "bravo", "charlie", "charley", "delta", "true", "false",
    ];
    let answers: Vec<&str> = tokens
        .into_iter()
        .filter(|token| {
            let key = strong.get(*token).copied().or_else(|| letter.get(*token).copied());
            key.is_some_and(|key| allowed.contains(key))
        })
        .collect();
    let mut words: Vec<&str> = answers;
    words.extend([
        "repeat",
        "skip",
        "next",
        "explain",
        "pause",
        "stop",
        "i don't know",
        "i dont know",
    ]);
    format!("#JSGF V1.0;\ngrammar drive;\npublic <command> = {};", words.join(" | "))
}

pub fn parse_voice(transcript: &str, options: &ParseOptions) -> VoiceCommand {
    let allowed: HashSet<&str> = options.choice_keys.iter().map(String::as_str).collect();
    let words = tokenize(transcript);
    if words.is_empty() {
        return VoiceCommand::Unrecognized;
    }
    for (command, phrases) in commands() {
        if contains_phrase(&words, phrases) {
            return command;
        }
    }
    if options.allow_know && contains_phrase(&words, know_phrases()) {
        return VoiceCommand::Know;
    }
    if let Some(key) = answer_from_patterns(&words, &allowed) {
        return VoiceCommand::Answer(key);
    }
    let strong = strong_map();
    let mut last = None;
    for word in &words {
        if let Some(key) = strong.get(word.as_str()) {
            if allowed.contains(key) {
                last = Some((*key).to_string());
            }
        }
    }
    if let Some(key) = last {
        return VoiceCommand::Answer(key);
    }
    if words.len() == 1 {
        let letter = letter_map();
        let weak = weak_map();
        let key = letter
            .get(words[0].as_str())
            .or_else(|| weak.get(words[0].as_str()))
            .copied();
        if let Some(key) = key {
            if allowed.contains(key) {
                return VoiceCommand::Answer(key.to_string());
            }
        }
    }
    VoiceCommand::Unrecognized
}

fn answer_from_patterns(words: &[String], allowed: &HashSet<&str>) -> Option<String> {
    let text = words.join(" ");
    let lead = "(?:answer|option|letter|choice)";
    let token = "(a|b|c|d|t|f|ay|eh|be|bee|see|sea|cee|dee|alpha|alfa|bravo|charlie|charley|delta|true|false)";
    let patterns = [
        format!("^(?:the\\s+)?{lead}\\s+(?:is\\s+)?{token}$"),
        format!("{lead}\\s+is\\s+{token}$"),
    ];
    let strong = strong_map();
    let weak = weak_map();
    let letter = letter_map();
    for pattern in patterns {
        let re = Regex::new(&pattern).expect("grammar regex");
        if let Some(caps) = re.captures(&text) {
            let token = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let key = strong
                .get(token)
                .or_else(|| weak.get(token))
                .or_else(|| letter.get(token))
                .copied();
            if let Some(key) = key {
                if allowed.contains(key) {
                    return Some(key.to_string());
                }
            }
        }
    }
    None
}

fn commands() -> Vec<(VoiceCommand, Vec<Vec<&'static str>>)> {
    vec![
        (VoiceCommand::Stop, vec![vec!["stop"], vec!["end", "session"], vec!["quit"]]),
        (VoiceCommand::Pause, vec![vec!["pause"], vec!["hold", "on"]]),
        (
            VoiceCommand::Repeat,
            vec![
                vec!["repeat"],
                vec!["repeat", "that"],
                vec!["say", "that", "again"],
                vec!["say", "again"],
                vec!["previous"],
            ],
        ),
        (
            VoiceCommand::Skip,
            vec![vec!["skip"], vec!["next"], vec!["next", "question"], vec!["pass"]],
        ),
        (VoiceCommand::Explain, vec![vec!["explain"], vec!["explanation"], vec!["why"]]),
        (
            VoiceCommand::DontKnow,
            vec![
                vec!["i", "don't", "know"],
                vec!["i", "dont", "know"],
                vec!["don't", "know"],
                vec!["dont", "know"],
                vec!["do", "not", "know"],
                vec!["no", "idea"],
                vec!["i", "have", "no", "idea"],
                vec!["not", "sure"],
                vec!["idk"],
            ],
        ),
    ]
}

fn know_phrases() -> Vec<Vec<&'static str>> {
    vec![
        vec!["got", "it"],
        vec!["i", "know"],
        vec!["i", "know", "it"],
        vec!["knew", "it"],
        vec!["know"],
    ]
}

fn strong_map() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("alpha", "A"),
        ("alfa", "A"),
        ("bravo", "B"),
        ("charlie", "C"),
        ("charley", "C"),
        ("delta", "D"),
        ("true", "T"),
        ("false", "F"),
    ])
}

fn weak_map() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("ay", "A"),
        ("eh", "A"),
        ("be", "B"),
        ("bee", "B"),
        ("see", "C"),
        ("sea", "C"),
        ("cee", "C"),
        ("dee", "D"),
    ])
}

fn letter_map() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("a", "A"),
        ("b", "B"),
        ("c", "C"),
        ("d", "D"),
        ("t", "T"),
        ("f", "F"),
    ])
}

fn tokenize(raw: &str) -> Vec<String> {
    let lower: String = raw.nfc().collect::<String>().to_lowercase();
    let mut cleaned = String::new();
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() || ch == '\'' || ch.is_whitespace() {
            cleaned.push(ch);
        } else {
            cleaned.push(' ');
        }
    }
    cleaned.split_whitespace().map(str::to_string).collect()
}

fn contains_phrase(words: &[String], phrases: Vec<Vec<&str>>) -> bool {
    phrases.into_iter().any(|phrase| {
        if phrase.len() > words.len() {
            return false;
        }
        (0..=words.len() - phrase.len()).any(|start| {
            phrase
                .iter()
                .enumerate()
                .all(|(offset, token)| words[start + offset] == *token)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mc() -> ParseOptions {
        ParseOptions::default()
    }
    fn tf() -> ParseOptions {
        ParseOptions {
            choice_keys: vec!["T".into(), "F".into()],
            allow_know: false,
        }
    }

    #[test]
    fn letters_and_phonetics() {
        assert_eq!(parse_voice("a", &mc()), VoiceCommand::Answer("A".into()));
        assert_eq!(parse_voice("Ay", &mc()), VoiceCommand::Answer("A".into()));
        assert_eq!(parse_voice("the answer is b", &mc()), VoiceCommand::Answer("B".into()));
        assert_eq!(parse_voice("bravo", &mc()), VoiceCommand::Answer("B".into()));
        assert_eq!(parse_voice("charlie.", &mc()), VoiceCommand::Answer("C".into()));
        assert_eq!(parse_voice("delta", &mc()), VoiceCommand::Answer("D".into()));
        assert_eq!(parse_voice("option see", &mc()), VoiceCommand::Answer("C".into()));
        assert_eq!(parse_voice("be", &mc()), VoiceCommand::Answer("B".into()));
        assert_eq!(parse_voice("dee", &mc()), VoiceCommand::Answer("D".into()));
        assert_eq!(parse_voice("alpha bravo", &mc()), VoiceCommand::Answer("B".into()));
    }

    #[test]
    fn true_false_only_when_allowed() {
        assert_eq!(parse_voice("true", &tf()), VoiceCommand::Answer("T".into()));
        assert_eq!(parse_voice("false", &tf()), VoiceCommand::Answer("F".into()));
        assert_eq!(parse_voice("true", &mc()), VoiceCommand::Unrecognized);
    }

    #[test]
    fn commands_win() {
        assert_eq!(parse_voice("repeat please", &mc()), VoiceCommand::Repeat);
        assert_eq!(parse_voice("skip", &mc()), VoiceCommand::Skip);
        assert_eq!(parse_voice("next", &mc()), VoiceCommand::Skip);
        assert_eq!(parse_voice("explain", &mc()), VoiceCommand::Explain);
        assert_eq!(parse_voice("explain bravo", &mc()), VoiceCommand::Explain);
        assert_eq!(parse_voice("pause", &mc()), VoiceCommand::Pause);
        assert_eq!(parse_voice("stop", &mc()), VoiceCommand::Stop);
        assert_eq!(parse_voice("I don't know", &mc()), VoiceCommand::DontKnow);
        assert_eq!(parse_voice("i dont know", &mc()), VoiceCommand::DontKnow);
        assert_eq!(parse_voice("no idea", &mc()), VoiceCommand::DontKnow);
    }

    #[test]
    fn rejects_noise() {
        assert_eq!(parse_voice("", &mc()), VoiceCommand::Unrecognized);
        assert_eq!(parse_voice("banana", &mc()), VoiceCommand::Unrecognized);
        assert_eq!(parse_voice("I see", &mc()), VoiceCommand::Unrecognized);
    }

    #[test]
    fn know_only_for_cards() {
        let mut know = mc();
        know.allow_know = true;
        assert_eq!(parse_voice("got it", &know), VoiceCommand::Know);
        assert_eq!(parse_voice("got it", &mc()), VoiceCommand::Unrecognized);
        assert_eq!(parse_voice("I don't know", &know), VoiceCommand::DontKnow);
    }

    #[test]
    fn jsgf() {
        let grammar = answer_grammar(&["A".into(), "B".into(), "C".into(), "D".into()]);
        assert!(grammar.starts_with("#JSGF V1.0;"));
        assert!(grammar.contains("alpha"));
        assert!(grammar.contains("i don't know"));
        assert!(!grammar.contains(" true"));
    }
}
