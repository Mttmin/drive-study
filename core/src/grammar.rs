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
    MoreTime,
    Flip,
    Unrecognized,
}

#[derive(Debug, Clone)]
pub struct ParseOptions {
    pub choice_keys: Vec<String>,
    pub allow_know: bool,
    /// When a true/false item uses A/B, "true" maps here instead of the letter T.
    pub true_key: Option<String>,
    pub false_key: Option<String>,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            choice_keys: vec!["A".into(), "B".into(), "C".into(), "D".into()],
            allow_know: false,
            true_key: None,
            false_key: None,
        }
    }
}

pub fn answer_grammar(choice_keys: &[String]) -> String {
    let allowed: HashSet<&str> = choice_keys.iter().map(String::as_str).collect();
    let strong = strong_map();
    let letter = letter_map();
    let tokens = [
        "a", "b", "c", "d", "alpha", "alfa", "bravo", "charlie", "charley", "delta", "true",
        "false",
    ];
    let answers: Vec<&str> = tokens
        .into_iter()
        .filter(|token| {
            let key = strong
                .get(*token)
                .copied()
                .or_else(|| letter.get(*token).copied());
            key.is_some_and(|key| allowed.contains(key))
        })
        .collect();
    let mut words: Vec<&str> = answers;
    for (word, key) in [
        ("one", "A"),
        ("first", "A"),
        ("two", "B"),
        ("second", "B"),
        ("three", "C"),
        ("third", "C"),
        ("four", "D"),
        ("fourth", "D"),
    ] {
        if allowed.contains(key) {
            words.push(word);
        }
    }
    words.extend([
        "repeat",
        "skip",
        "next",
        "explain",
        "pause",
        "wait",
        "more time",
        "stop",
        "i don't know",
        "i dont know",
    ]);
    format!(
        "#JSGF V1.0;\ngrammar drive;\npublic <command> = {};",
        words.join(" | ")
    )
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
    if let Some(key) = answer_from_patterns(&words, &allowed, options) {
        return VoiceCommand::Answer(key);
    }
    if let Some(key) = answer_from_ordinals(&words, &allowed) {
        return VoiceCommand::Answer(key);
    }
    let mut last = None;
    for word in &words {
        if let Some(key) = mapped_answer(word, options, &allowed) {
            last = Some(key);
        }
    }
    if let Some(key) = last {
        return VoiceCommand::Answer(key);
    }
    if let Some(key) = trailing_letter(&words, &allowed) {
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

fn ordinal_key(token: &str) -> Option<&'static str> {
    Some(match token {
        "1" | "1st" | "one" | "first" => "A",
        "2" | "2nd" | "two" | "second" => "B",
        "3" | "3rd" | "three" | "third" => "C",
        "4" | "4th" | "four" | "fourth" => "D",
        _ => return None,
    })
}

fn answer_from_ordinals(words: &[String], allowed: &HashSet<&str>) -> Option<String> {
    let accept = |token: &str| {
        ordinal_key(token)
            .filter(|key| allowed.contains(key))
            .map(|key| (*key).to_string())
    };
    if words.len() == 1 {
        return accept(&words[0]);
    }
    let body: &[String] = if words.first().map(String::as_str) == Some("the") {
        &words[1..]
    } else {
        words
    };
    if body.len() == 1 {
        return accept(&body[0]);
    }
    if body.len() == 2 && matches!(body[1].as_str(), "one" | "option" | "choice" | "answer") {
        if let Some(key) = accept(&body[0]) {
            return Some(key);
        }
    }
    let head = body.first().map(String::as_str).unwrap_or("");
    if matches!(head, "answer" | "option" | "choice" | "number" | "letter") {
        let token = body.iter().rev().find(|word| ordinal_key(word).is_some())?;
        return accept(token);
    }
    None
}

fn trailing_letter(words: &[String], allowed: &HashSet<&str>) -> Option<String> {
    if words.len() < 2 || words.len() > 6 {
        return None;
    }
    let key = letter_map().get(words.last()?.as_str()).copied()?;
    allowed.contains(key).then(|| key.to_string())
}

fn mapped_answer(word: &str, options: &ParseOptions, allowed: &HashSet<&str>) -> Option<String> {
    if word == "true" {
        if let Some(key) = &options.true_key {
            if allowed.contains(key.as_str()) {
                return Some(key.clone());
            }
        }
    }
    if word == "false" {
        if let Some(key) = &options.false_key {
            if allowed.contains(key.as_str()) {
                return Some(key.clone());
            }
        }
    }
    strong_map()
        .get(word)
        .copied()
        .filter(|key| allowed.contains(key))
        .map(|key| key.to_string())
}

fn answer_from_patterns(
    words: &[String],
    allowed: &HashSet<&str>,
    options: &ParseOptions,
) -> Option<String> {
    let text = words.join(" ");
    let lead = "(?:answer|option|letter|choice)";
    let token = "(a|b|c|d|t|f|ay|eh|be|bee|see|sea|cee|dee|alpha|alfa|bravo|charlie|charley|delta|true|false)";
    let patterns = [
        format!("^(?:the\\s+)?{lead}\\s+(?:is\\s+)?{token}$"),
        format!("{lead}\\s+is\\s+{token}$"),
    ];
    let weak = weak_map();
    let letter = letter_map();
    for pattern in patterns {
        let re = Regex::new(&pattern).expect("grammar regex");
        if let Some(caps) = re.captures(&text) {
            let token = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            if let Some(key) = mapped_answer(token, options, allowed) {
                return Some(key);
            }
            let key = weak.get(token).or_else(|| letter.get(token)).copied();
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
        (
            VoiceCommand::Stop,
            vec![vec!["stop"], vec!["end", "session"], vec!["quit"]],
        ),
        (
            VoiceCommand::MoreTime,
            vec![
                vec!["wait"],
                vec!["more", "time"],
                vec!["give", "me", "a", "second"],
                vec!["a", "second"],
            ],
        ),
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
            vec![
                vec!["skip"],
                vec!["next"],
                vec!["next", "question"],
                vec!["pass"],
            ],
        ),
        (
            VoiceCommand::Explain,
            vec![vec!["explain"], vec!["explanation"], vec!["why"]],
        ),
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
        (
            VoiceCommand::Flip,
            vec![
                vec!["flip"],
                vec!["flip", "it"],
                vec!["flip", "the", "card"],
                vec!["turn", "it", "over"],
                vec!["turn", "over"],
                vec!["show", "the", "answer"],
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
            true_key: None,
            false_key: None,
        }
    }

    #[test]
    fn letters_and_phonetics() {
        assert_eq!(parse_voice("a", &mc()), VoiceCommand::Answer("A".into()));
        assert_eq!(parse_voice("Ay", &mc()), VoiceCommand::Answer("A".into()));
        assert_eq!(
            parse_voice("the answer is b", &mc()),
            VoiceCommand::Answer("B".into())
        );
        assert_eq!(
            parse_voice("bravo", &mc()),
            VoiceCommand::Answer("B".into())
        );
        assert_eq!(
            parse_voice("charlie.", &mc()),
            VoiceCommand::Answer("C".into())
        );
        assert_eq!(
            parse_voice("delta", &mc()),
            VoiceCommand::Answer("D".into())
        );
        assert_eq!(
            parse_voice("option see", &mc()),
            VoiceCommand::Answer("C".into())
        );
        assert_eq!(parse_voice("be", &mc()), VoiceCommand::Answer("B".into()));
        assert_eq!(parse_voice("dee", &mc()), VoiceCommand::Answer("D".into()));
        assert_eq!(
            parse_voice("alpha bravo", &mc()),
            VoiceCommand::Answer("B".into())
        );
    }

    #[test]
    fn true_false_only_when_allowed() {
        assert_eq!(parse_voice("true", &tf()), VoiceCommand::Answer("T".into()));
        assert_eq!(
            parse_voice("false", &tf()),
            VoiceCommand::Answer("F".into())
        );
        assert_eq!(parse_voice("true", &mc()), VoiceCommand::Unrecognized);
        assert_eq!(parse_voice("wait", &mc()), VoiceCommand::MoreTime);
        assert_eq!(parse_voice("more time", &mc()), VoiceCommand::MoreTime);
        assert_eq!(parse_voice("next", &mc()), VoiceCommand::Skip);
        let ab = ParseOptions {
            choice_keys: vec!["A".into(), "B".into()],
            true_key: Some("A".into()),
            false_key: Some("B".into()),
            ..ParseOptions::default()
        };
        assert_eq!(parse_voice("true", &ab), VoiceCommand::Answer("A".into()));
        assert_eq!(parse_voice("false", &ab), VoiceCommand::Answer("B".into()));
        assert_eq!(
            parse_voice("the answer is false", &ab),
            VoiceCommand::Answer("B".into())
        );
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
        assert_eq!(parse_voice("flip", &mc()), VoiceCommand::Flip);
        assert_eq!(parse_voice("flip the card", &mc()), VoiceCommand::Flip);
        assert_eq!(parse_voice("show the answer", &mc()), VoiceCommand::Flip);
    }

    #[test]
    fn rejects_noise() {
        assert_eq!(parse_voice("", &mc()), VoiceCommand::Unrecognized);
        assert_eq!(parse_voice("banana", &mc()), VoiceCommand::Unrecognized);
        assert_eq!(parse_voice("I see", &mc()), VoiceCommand::Unrecognized);
    }

    #[test]
    fn ordinals_numbers_and_short_phrases() {
        assert_eq!(
            parse_voice("the first one", &mc()),
            VoiceCommand::Answer("A".into())
        );
        assert_eq!(
            parse_voice("first one", &mc()),
            VoiceCommand::Answer("A".into())
        );
        assert_eq!(
            parse_voice("number one", &mc()),
            VoiceCommand::Answer("A".into())
        );
        assert_eq!(parse_voice("1", &mc()), VoiceCommand::Answer("A".into()));
        assert_eq!(parse_voice("two", &mc()), VoiceCommand::Answer("B".into()));
        assert_eq!(
            parse_voice("option 2", &mc()),
            VoiceCommand::Answer("B".into())
        );
        assert_eq!(
            parse_voice("the second one", &mc()),
            VoiceCommand::Answer("B".into())
        );
        assert_eq!(
            parse_voice("third", &mc()),
            VoiceCommand::Answer("C".into())
        );
        assert_eq!(
            parse_voice("choice three", &mc()),
            VoiceCommand::Answer("C".into())
        );
        assert_eq!(parse_voice("4", &mc()), VoiceCommand::Answer("D".into()));
        assert_eq!(
            parse_voice("the fourth one", &mc()),
            VoiceCommand::Answer("D".into())
        );
        assert_eq!(
            parse_voice("letter four", &mc()),
            VoiceCommand::Answer("D".into())
        );
        assert_eq!(
            parse_voice("the answer is the first one", &mc()),
            VoiceCommand::Answer("A".into())
        );
        assert_eq!(
            parse_voice("I think b", &mc()),
            VoiceCommand::Answer("B".into())
        );
        assert_eq!(parse_voice("a second", &mc()), VoiceCommand::MoreTime);
        assert_eq!(parse_voice("I see", &mc()), VoiceCommand::Unrecognized);
        assert_eq!(
            parse_voice("I have one account", &mc()),
            VoiceCommand::Unrecognized
        );
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
