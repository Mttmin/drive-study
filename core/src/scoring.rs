use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ScoredResponse {
    pub section: String,
    pub correct: bool,
}

#[derive(Debug, Clone)]
pub struct SectionScore {
    pub correct: usize,
    pub total: usize,
    pub percent: f64,
}

#[derive(Debug, Clone)]
pub struct ExamScore {
    pub correct: usize,
    pub total: usize,
    pub percent: f64,
    pub passed: bool,
    pub by_section: HashMap<String, SectionScore>,
}

pub fn score_exam(responses: &[ScoredResponse], pass_percent: f64) -> ExamScore {
    let total = responses.len();
    let correct = responses.iter().filter(|row| row.correct).count();
    let ratio = if total == 0 { 0.0 } else { correct as f64 / total as f64 };
    let mut by_section: HashMap<String, SectionScore> = HashMap::new();
    for row in responses {
        let bucket = by_section.entry(row.section.clone()).or_insert(SectionScore {
            correct: 0,
            total: 0,
            percent: 0.0,
        });
        bucket.total += 1;
        if row.correct {
            bucket.correct += 1;
        }
        bucket.percent = round1(bucket.correct as f64 / bucket.total as f64 * 100.0);
    }
    ExamScore {
        correct,
        total,
        percent: round1(ratio * 100.0),
        passed: total > 0 && ratio + 1e-12 >= pass_percent / 100.0,
        by_section,
    }
}

#[derive(Debug, Clone)]
pub struct ReadinessAttempt {
    pub section: String,
    pub topic: Option<String>,
    pub correct: bool,
}

#[derive(Debug, Clone)]
pub struct WeakSection {
    pub id: String,
    pub accuracy: f64,
    pub attempts: usize,
}

#[derive(Debug, Clone)]
pub struct WeakTopic {
    pub topic: String,
    pub accuracy: f64,
    pub attempts: usize,
}

#[derive(Debug, Clone)]
pub struct Readiness {
    pub score: Option<i64>,
    pub coverage: f64,
    pub weak: Vec<WeakSection>,
}

pub fn readiness(sections: &[(String, f64)], attempts: &[ReadinessAttempt], seen: usize, pool: usize, min_attempts: usize) -> Readiness {
    let mut grouped: HashMap<String, (usize, usize)> = HashMap::new();
    for attempt in attempts {
        let bucket = grouped.entry(attempt.section.clone()).or_insert((0, 0));
        bucket.1 += 1;
        if attempt.correct {
            bucket.0 += 1;
        }
    }
    let mut weight_sum = 0.0;
    let mut weighted = 0.0;
    let mut weak = Vec::new();
    for (id, weight) in sections {
        let Some(&(correct, total)) = grouped.get(id) else { continue };
        if total < min_attempts {
            continue;
        }
        let accuracy = correct as f64 / total as f64;
        weight_sum += weight;
        weighted += accuracy * weight;
        if accuracy < 0.7 {
            weak.push(WeakSection {
                id: id.clone(),
                accuracy,
                attempts: total,
            });
        }
    }
    weak.sort_by(|a, b| a.accuracy.partial_cmp(&b.accuracy).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.id.cmp(&b.id)));
    let coverage = if pool == 0 { 0.0 } else { (seen as f64 / pool as f64).min(1.0) };
    if weight_sum == 0.0 {
        return Readiness { score: None, coverage, weak };
    }
    let score = (100.0 * (weighted / weight_sum) * (0.55 + 0.45 * coverage)).round() as i64;
    Readiness {
        score: Some(score),
        coverage,
        weak,
    }
}

pub fn weak_topics(attempts: &[ReadinessAttempt], min_attempts: usize) -> Vec<WeakTopic> {
    let mut grouped: HashMap<String, (usize, usize)> = HashMap::new();
    for attempt in attempts {
        let Some(topic) = &attempt.topic else { continue };
        let bucket = grouped.entry(topic.clone()).or_insert((0, 0));
        bucket.1 += 1;
        if attempt.correct {
            bucket.0 += 1;
        }
    }
    let mut weak = Vec::new();
    for (topic, (correct, total)) in grouped {
        if total < min_attempts {
            continue;
        }
        let accuracy = correct as f64 / total as f64;
        if accuracy < 0.7 {
            weak.push(WeakTopic { topic, accuracy, attempts: total });
        }
    }
    weak.sort_by(|a, b| a.accuracy.partial_cmp(&b.accuracy).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.topic.cmp(&b.topic)));
    weak
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pass_mark_uses_the_unrounded_ratio() {
        let fail: Vec<_> = (0..52)
            .map(|_| ScoredResponse { section: "SIE-1".into(), correct: true })
            .chain((0..23).map(|_| ScoredResponse { section: "SIE-2".into(), correct: false }))
            .collect();
        let pass: Vec<_> = (0..53)
            .map(|_| ScoredResponse { section: "SIE-1".into(), correct: true })
            .chain((0..22).map(|_| ScoredResponse { section: "SIE-2".into(), correct: false }))
            .collect();
        let failed = score_exam(&fail, 70.0);
        assert!(!failed.passed);
        assert_eq!(failed.percent, 69.3);
        let passed = score_exam(&pass, 70.0);
        assert!(passed.passed);
        assert_eq!(passed.percent, 70.7);
        assert_eq!(passed.by_section["SIE-1"].correct, 53);
    }

    #[test]
    fn readiness_waits_for_three_answers() {
        let result = readiness(&[("SIE-1".into(), 16.0)], &[ReadinessAttempt { section: "SIE-1".into(), topic: None, correct: true }], 1, 10, 3);
        assert!(result.score.is_none());
    }

    #[test]
    fn readiness_weights_and_scales() {
        let attempts = vec![
            att("A", "markets", true),
            att("A", "markets", true),
            att("A", "markets", true),
            att("B", "options", false),
            att("B", "options", false),
            att("B", "options", false),
        ];
        let result = readiness(&[("A".into(), 50.0), ("B".into(), 50.0)], &attempts, 6, 10, 3);
        assert_eq!(result.score, Some(41));
        assert_eq!(result.coverage, 0.6);
        assert_eq!(result.weak[0].id, "B");
        let topics = weak_topics(
            &[
                ReadinessAttempt { section: "B".into(), topic: Some("options".into()), correct: false },
                ReadinessAttempt { section: "B".into(), topic: Some("options".into()), correct: false },
            ],
            2,
        );
        assert_eq!(topics[0].topic, "options");
    }

    fn att(section: &str, topic: &str, correct: bool) -> ReadinessAttempt {
        ReadinessAttempt {
            section: section.into(),
            topic: Some(topic.into()),
            correct,
        }
    }
}
