use serde::{Deserialize, Serialize};

use crate::content::ExamId;

const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const TEN_MIN_MS: i64 = 10 * 60 * 1000;
const EF_MIN: f64 = 1.3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    Question,
    Flashcard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewOutcome {
    Correct,
    Wrong,
    Guessed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SrsMeta {
    pub id: String,
    pub kind: ItemKind,
    pub exam: ExamId,
    pub section: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SrsCard {
    pub id: String,
    pub kind: ItemKind,
    pub exam: ExamId,
    pub section: String,
    pub ef: f64,
    pub interval_days: u32,
    pub reps: u32,
    pub lapses: u32,
    pub due: i64,
    pub correct_dates: Vec<String>,
    pub graduated: bool,
    pub last_reviewed: Option<i64>,
}

pub fn next_ef(ef: f64, quality: i32) -> f64 {
    let gap = 5.0 - f64::from(quality);
    let value = ef + (0.1 - gap * (0.08 + gap * 0.02));
    ((value * 100.0).round() / 100.0).max(EF_MIN)
}

pub fn review_card(prev: Option<&SrsCard>, meta: &SrsMeta, outcome: ReviewOutcome, now_ms: i64, today: &str) -> SrsCard {
    let mut card = prev.cloned().unwrap_or_else(|| SrsCard {
        id: meta.id.clone(),
        kind: meta.kind,
        exam: meta.exam,
        section: meta.section.clone(),
        ef: 2.5,
        interval_days: 0,
        reps: 0,
        lapses: 0,
        due: 0,
        correct_dates: Vec::new(),
        graduated: false,
        last_reviewed: None,
    });
    card.last_reviewed = Some(now_ms);

    if matches!(outcome, ReviewOutcome::Wrong | ReviewOutcome::Guessed) {
        card.correct_dates.clear();
        card.graduated = false;
        card.reps = 0;
        card.interval_days = 1;
        if outcome == ReviewOutcome::Wrong {
            card.lapses += 1;
            card.ef = next_ef(card.ef, 2);
            card.due = now_ms + TEN_MIN_MS;
        } else {
            card.ef = next_ef(card.ef, 3);
            card.due = now_ms + DAY_MS;
        }
        return card;
    }

    if card.correct_dates.last().map(String::as_str) == Some(today) {
        return card;
    }

    card.ef = next_ef(card.ef, 5);
    card.correct_dates.push(today.to_string());
    if card.correct_dates.len() >= 3 {
        card.graduated = true;
    }
    if card.reps == 0 {
        card.interval_days = 1;
    } else if card.reps == 1 {
        card.interval_days = 3;
    } else {
        card.interval_days = (card.interval_days as f64 * card.ef).round().max(1.0) as u32;
    }
    card.reps += 1;
    card.due = now_ms + i64::from(card.interval_days) * DAY_MS;
    card
}

pub fn is_due(card: &SrsCard, now_ms: i64) -> bool {
    !card.graduated && card.due <= now_ms
}

pub fn streak_from_dates(dates: &[String], today: &str) -> u32 {
    use chrono::NaiveDate;
    let set: std::collections::HashSet<&str> = dates.iter().map(String::as_str).collect();
    let mut cursor = NaiveDate::parse_from_str(today, "%Y-%m-%d").expect("date");
    if !set.contains(today) {
        cursor = cursor.pred_opt().unwrap_or(cursor);
    }
    let mut count = 0;
    loop {
        let key = cursor.format("%Y-%m-%d").to_string();
        if !set.contains(key.as_str()) {
            break;
        }
        count += 1;
        match cursor.pred_opt() {
            Some(prev) => cursor = prev,
            None => break,
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::ExamId;

    fn meta() -> SrsMeta {
        SrsMeta {
            id: "sie-q-0001".into(),
            kind: ItemKind::Question,
            exam: ExamId::Sie,
            section: "SIE-1".into(),
        }
    }

    #[test]
    fn graduates_on_three_separate_days() {
        let m = meta();
        let mut card = review_card(None, &m, ReviewOutcome::Correct, 1_000, "2026-01-01");
        assert_eq!(card.correct_dates, vec!["2026-01-01"]);
        assert!(!card.graduated);
        assert_eq!(card.interval_days, 1);
        assert_eq!(card.reps, 1);
        assert_eq!(card.ef, 2.6);

        card = review_card(Some(&card), &m, ReviewOutcome::Correct, 2_000, "2026-01-01");
        assert_eq!(card.correct_dates, vec!["2026-01-01"]);
        assert_eq!(card.reps, 1);
        assert_eq!(card.interval_days, 1);
        assert_eq!(card.ef, 2.6);

        card = review_card(Some(&card), &m, ReviewOutcome::Correct, 3_000, "2026-01-02");
        assert_eq!(card.correct_dates, vec!["2026-01-01", "2026-01-02"]);
        assert_eq!(card.interval_days, 3);
        assert_eq!(card.ef, 2.7);
        assert!(!card.graduated);

        card = review_card(Some(&card), &m, ReviewOutcome::Correct, 4_000, "2026-01-05");
        assert_eq!(card.correct_dates.len(), 3);
        assert!(card.graduated);
        assert_eq!(card.interval_days, 8);
        assert_eq!(card.ef, 2.8);
        assert!(!is_due(&card, 4_000 + 86_400_000));
    }

    #[test]
    fn wrong_resets_and_returns_in_ten_minutes() {
        let m = meta();
        let card = review_card(None, &m, ReviewOutcome::Correct, 0, "2026-01-01");
        let card = review_card(Some(&card), &m, ReviewOutcome::Correct, 0, "2026-01-02");
        let now = 50_000;
        let card = review_card(Some(&card), &m, ReviewOutcome::Wrong, now, "2026-01-10");
        assert!(!card.graduated);
        assert!(card.correct_dates.is_empty());
        assert_eq!(card.lapses, 1);
        assert_eq!(card.reps, 0);
        assert_eq!(card.due - now, 10 * 60 * 1000);
        assert!(is_due(&card, now + 11 * 60 * 1000));
    }

    #[test]
    fn guess_returns_tomorrow_without_credit() {
        let m = meta();
        let card = review_card(None, &m, ReviewOutcome::Correct, 0, "2026-01-01");
        let card = review_card(Some(&card), &m, ReviewOutcome::Correct, 0, "2026-01-02");
        let now = 80_000;
        let card = review_card(Some(&card), &m, ReviewOutcome::Guessed, now, "2026-01-03");
        assert!(card.correct_dates.is_empty());
        assert!(!card.graduated);
        assert_eq!(card.due - now, 24 * 60 * 60 * 1000);
        assert_eq!(next_ef(2.5, 5).max(1.3), 2.6);
    }

    #[test]
    fn streak() {
        assert_eq!(streak_from_dates(&["2026-04-01".into(), "2026-04-02".into(), "2026-04-03".into()], "2026-04-03"), 3);
        assert_eq!(streak_from_dates(&["2026-04-01".into(), "2026-04-02".into()], "2026-04-03"), 2);
        assert_eq!(streak_from_dates(&["2026-04-01".into()], "2026-04-03"), 0);
    }
}
