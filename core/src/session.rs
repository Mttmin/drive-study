use crate::blueprint::{draw_weighted, shuffle, DrawItem, WeightedSection};
use crate::content::{ExamId, Flashcard, Lesson, Question, SectionBlueprint, Segment};

#[derive(Debug, Clone)]
pub enum SessionItem {
    Narration { id: String, title: String, text: String },
    Question(Question),
    Card(Flashcard),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanId {
    Sie,
    S7,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMode {
    Lesson,
    Quiz,
    Rapid,
    Mock,
    Mistakes,
    Drive,
}

#[derive(Debug, Clone)]
pub struct SessionPlan {
    pub title: String,
    pub mode: SessionMode,
    pub exam: PlanId,
    pub items: Vec<SessionItem>,
    pub listen_only: bool,
    pub timed: bool,
    pub limit_seconds: Option<u32>,
    pub pass_percent: Option<f64>,
    pub note: Option<String>,
    pub estimated_seconds: u32,
    pub lesson_id: Option<String>,
}

const QUESTION_SEC: u32 = 50;
const CARD_SEC: u32 = 22;
const NARRATION_SEC: u32 = 75;
const DRIVE_TARGET: u32 = 20 * 60;

pub fn build_lesson(lesson: &Lesson, questions: &[Question], listen_only: bool) -> SessionPlan {
    let mut items = Vec::new();
    for segment in &lesson.segments {
        match segment {
            Segment::Narration { text } => items.push(SessionItem::Narration {
                id: format!("{}-narr", lesson.id),
                title: lesson.title.clone(),
                text: text.clone(),
            }),
            Segment::Check { question_id } => {
                if let Some(question) = questions.iter().find(|question| &question.id == question_id) {
                    items.push(SessionItem::Question(question.clone()));
                }
            }
        }
    }
    if !lesson.summary.is_empty() {
        items.push(SessionItem::Narration {
            id: format!("{}-summary", lesson.id),
            title: lesson.title.clone(),
            text: lesson.summary.clone(),
        });
    }
    SessionPlan {
        title: lesson.title.clone(),
        mode: SessionMode::Lesson,
        exam: exam_plan(lesson.exam),
        items: items.clone(),
        listen_only,
        timed: false,
        limit_seconds: None,
        pass_percent: None,
        note: lesson.sample.then(|| "Sample lesson".into()),
        estimated_seconds: estimate(&items),
        lesson_id: Some(lesson.id.clone()),
    }
}

pub fn build_quiz(exam: PlanId, section_id: Option<&str>, count: usize, questions: &[Question], sections: &[SectionBlueprint], listen_only: bool, rng: &mut dyn FnMut() -> f64) -> SessionPlan {
    let pool: Vec<&Question> = questions
        .iter()
        .filter(|question| matches_plan(question.exam, exam) && section_id.is_none_or(|id| question.section == id))
        .collect();
    let picked: Vec<Question> = if section_id.is_some() || exam == PlanId::Both {
        if exam == PlanId::Both && section_id.is_none() {
            let mut sie: Vec<Question> = pool.iter().filter(|q| q.exam == ExamId::Sie).map(|q| (*q).clone()).collect();
            let mut s7: Vec<Question> = pool.iter().filter(|q| q.exam == ExamId::S7).map(|q| (*q).clone()).collect();
            shuffle(&mut sie, rng);
            shuffle(&mut s7, rng);
            let sie_count = sie.len().min(((count as f64) * 0.6).ceil() as usize);
            let s7_count = s7.len().min(count.saturating_sub(sie_count));
            let mut mixed = Vec::new();
            mixed.extend(sie.into_iter().take(sie_count));
            mixed.extend(s7.into_iter().take(s7_count));
            shuffle(&mut mixed, rng);
            mixed
        } else {
            let mut rows: Vec<Question> = pool.into_iter().cloned().collect();
            shuffle(&mut rows, rng);
            rows.into_iter().take(count).collect()
        }
    } else {
        let weights: Vec<WeightedSection> = sections
            .iter()
            .filter(|section| pool.iter().any(|question| question.section == section.id))
            .map(|section| WeightedSection { id: section.id.clone(), weight: section.weight })
            .collect();
        let items: Vec<DrawItem<Question>> = pool
            .iter()
            .map(|question| DrawItem {
                id: question.id.clone(),
                section: question.section.clone(),
                value: (*question).clone(),
            })
            .collect();
        draw_weighted(&items, &weights, count, rng).into_iter().map(|item| item.value).collect()
    };
    let label = section_id.unwrap_or(match exam {
        PlanId::Both => "SIE and Series 7",
        PlanId::S7 => "Series 7",
        PlanId::Sie => "SIE",
    });
    let note = (picked.len() < count).then(|| format!("The library has {} matching questions.", picked.len()));
    let items: Vec<SessionItem> = picked.into_iter().map(SessionItem::Question).collect();
    SessionPlan {
        title: format!("Quiz · {label}"),
        mode: SessionMode::Quiz,
        exam,
        estimated_seconds: estimate(&items),
        items,
        listen_only,
        timed: false,
        limit_seconds: None,
        pass_percent: None,
        note,
        lesson_id: None,
    }
}

pub fn build_rapid(exam: PlanId, section_id: Option<&str>, count: usize, cards: &[Flashcard], listen_only: bool, rng: &mut dyn FnMut() -> f64) -> SessionPlan {
    let mut pool: Vec<Flashcard> = cards
        .iter()
        .filter(|card| matches_plan(card.exam, exam) && section_id.is_none_or(|id| card.section == id))
        .cloned()
        .collect();
    shuffle(&mut pool, rng);
    let items: Vec<SessionItem> = pool.into_iter().take(count).map(SessionItem::Card).collect();
    SessionPlan {
        title: "Rapid fire".into(),
        mode: SessionMode::Rapid,
        exam,
        estimated_seconds: estimate(&items),
        items,
        listen_only,
        timed: false,
        limit_seconds: None,
        pass_percent: None,
        note: None,
        lesson_id: None,
    }
}

pub fn build_mock(exam: ExamId, timed: bool, questions: &[Question], blueprint: &crate::content::ExamBlueprint, listen_only: bool, rng: &mut dyn FnMut() -> f64) -> SessionPlan {
    let items: Vec<DrawItem<Question>> = questions
        .iter()
        .filter(|question| question.exam == exam)
        .map(|question| DrawItem {
            id: question.id.clone(),
            section: question.section.clone(),
            value: question.clone(),
        })
        .collect();
    let weights: Vec<WeightedSection> = blueprint
        .sections
        .iter()
        .map(|section| WeightedSection { id: section.id.clone(), weight: section.weight })
        .collect();
    let picked = draw_weighted(&items, &weights, blueprint.questions as usize, rng);
    let short = picked.len() < blueprint.questions as usize;
    let scaled = (blueprint.minutes as f64 * 60.0 * (picked.len() as f64 / blueprint.questions as f64)).round().max(60.0) as u32;
    let note = if short {
        Some(format!(
            "Sample pool: {} questions. A full {} mock is {} scored questions. FINRA also adds 10 unscored pretest items on the real exam.",
            picked.len(),
            exam.as_str(),
            blueprint.questions
        ))
    } else {
        Some("FINRA also adds 10 unscored pretest items on the real exam. This mock uses the scored count.".into())
    };
    let session_items: Vec<SessionItem> = picked.into_iter().map(|item| SessionItem::Question(item.value)).collect();
    SessionPlan {
        title: if timed { format!("{} mock · timed", exam.as_str()) } else { format!("{} mock · car", exam.as_str()) },
        mode: SessionMode::Mock,
        exam: exam_plan(exam),
        listen_only,
        timed,
        limit_seconds: timed.then_some(scaled),
        pass_percent: Some(blueprint.pass_percent),
        estimated_seconds: if timed { scaled } else { estimate(&session_items) },
        items: session_items,
        note,
        lesson_id: None,
    }
}

pub fn build_mistakes(exam: PlanId, questions: &[Question], cards: &[Flashcard], due: &std::collections::HashSet<String>, listen_only: bool) -> SessionPlan {
    let mut due_questions: Vec<Question> = questions
        .iter()
        .filter(|question| due.contains(&question.id) && matches_plan(question.exam, exam))
        .cloned()
        .collect();
    let mut due_cards: Vec<Flashcard> = cards
        .iter()
        .filter(|card| due.contains(&card.id) && matches_plan(card.exam, exam))
        .cloned()
        .collect();
    order_both(&mut due_questions, exam);
    order_both_cards(&mut due_cards, exam);
    let mut items: Vec<SessionItem> = due_questions.into_iter().map(SessionItem::Question).collect();
    items.extend(due_cards.into_iter().map(SessionItem::Card));
    SessionPlan {
        title: "Mistakes drive".into(),
        mode: SessionMode::Mistakes,
        exam,
        estimated_seconds: estimate(&items),
        items,
        listen_only,
        timed: false,
        limit_seconds: None,
        pass_percent: None,
        note: None,
        lesson_id: None,
    }
}

pub fn build_drive(
    exam: PlanId,
    questions: &[Question],
    cards: &[Flashcard],
    lessons: &[Lesson],
    due: &std::collections::HashSet<String>,
    weak: &[String],
    completed: &[String],
    listen_only: bool,
    rng: &mut dyn FnMut() -> f64,
) -> SessionPlan {
    let questions: Vec<Question> = questions.iter().filter(|question| matches_plan(question.exam, exam)).cloned().collect();
    let cards: Vec<Flashcard> = cards.iter().filter(|card| matches_plan(card.exam, exam)).cloned().collect();
    let mut due_questions: Vec<Question> = questions.iter().filter(|question| due.contains(&question.id)).cloned().collect();
    let mut due_cards: Vec<Flashcard> = cards.iter().filter(|card| due.contains(&card.id)).cloned().collect();
    order_both(&mut due_questions, exam);
    order_both_cards(&mut due_cards, exam);
    let used: std::collections::HashSet<String> = due_questions.iter().map(|question| question.id.clone()).collect();
    let mut fresh = Vec::new();
    for section_id in drive_section_order(exam, weak, &questions) {
        let mut pool: Vec<Question> = questions
            .iter()
            .filter(|question| question.section == section_id && !used.contains(&question.id))
            .cloned()
            .collect();
        shuffle(&mut pool, rng);
        fresh.extend(pool);
    }
    let mut items = Vec::new();
    let mut seconds = 0;
    if let Some(narration) = mini_lesson(lessons, exam, completed) {
        items.push(narration);
        seconds += NARRATION_SEC;
    }
    let mut mistake_index = 0;
    let mut fresh_index = 0;
    let mut card_index = 0;
    let pattern = ["mistake", "mistake", "fresh", "card", "mistake", "fresh"];
    let mut spins = 0;
    while seconds < DRIVE_TARGET && spins < 8 {
        spins += 1;
        let mut added = false;
        for step in pattern {
            if seconds >= DRIVE_TARGET {
                break;
            }
            if step == "mistake" && mistake_index < due_questions.len() {
                items.push(SessionItem::Question(due_questions[mistake_index].clone()));
                mistake_index += 1;
                seconds += QUESTION_SEC;
                added = true;
            } else if step == "fresh" && fresh_index < fresh.len() {
                items.push(SessionItem::Question(fresh[fresh_index].clone()));
                fresh_index += 1;
                seconds += QUESTION_SEC;
                added = true;
            } else if step == "card" && card_index < due_cards.len() {
                items.push(SessionItem::Card(due_cards[card_index].clone()));
                card_index += 1;
                seconds += CARD_SEC;
                added = true;
            }
        }
        if !added {
            break;
        }
    }
    while seconds < DRIVE_TARGET && fresh_index < fresh.len() {
        items.push(SessionItem::Question(fresh[fresh_index].clone()));
        fresh_index += 1;
        seconds += QUESTION_SEC;
    }
    SessionPlan {
        title: "Drive, 20 minutes".into(),
        mode: SessionMode::Drive,
        exam,
        estimated_seconds: seconds,
        items,
        listen_only,
        timed: false,
        limit_seconds: None,
        pass_percent: None,
        note: Some("Due mistakes, weak sections, and a short lesson. SIE comes first when you study both.".into()),
        lesson_id: None,
    }
}

fn mini_lesson(lessons: &[Lesson], exam: PlanId, completed: &[String]) -> Option<SessionItem> {
    let mut ordered: Vec<&Lesson> = lessons.iter().filter(|lesson| matches_plan(lesson.exam, exam)).collect();
    ordered.sort_by(|a, b| (b.exam == ExamId::Sie).cmp(&(a.exam == ExamId::Sie)).then(a.order.cmp(&b.order)));
    let next = ordered.into_iter().find(|lesson| !completed.iter().any(|id| id == &lesson.id))?;
    let text = next.segments.iter().find_map(|segment| match segment {
        Segment::Narration { text } => Some(text.clone()),
        Segment::Check { .. } => None,
    })?;
    Some(SessionItem::Narration {
        id: format!("{}#mini", next.id),
        title: next.title.clone(),
        text,
    })
}

fn drive_section_order(exam: PlanId, weak: &[String], questions: &[Question]) -> Vec<String> {
    let mut present = Vec::new();
    for question in questions {
        if matches_plan(question.exam, exam) && !present.iter().any(|id: &String| id == &question.section) {
            present.push(question.section.clone());
        }
    }
    let ordered_weak: Vec<String> = weak.iter().filter(|id| present.iter().any(|have| have == *id)).cloned().collect();
    let rest: Vec<String> = present.into_iter().filter(|id| !ordered_weak.contains(id)).collect();
    let mut combined = ordered_weak;
    combined.extend(rest);
    if exam != PlanId::Both {
        return combined;
    }
    let mut sie: Vec<String> = combined.iter().filter(|id| id.starts_with("SIE")).cloned().collect();
    let s7: Vec<String> = combined.iter().filter(|id| id.starts_with("S7")).cloned().collect();
    sie.extend(s7);
    sie
}

fn estimate(items: &[SessionItem]) -> u32 {
    items
        .iter()
        .map(|item| match item {
            SessionItem::Narration { .. } => NARRATION_SEC,
            SessionItem::Card(_) => CARD_SEC,
            SessionItem::Question(_) => QUESTION_SEC,
        })
        .sum()
}

fn matches_plan(exam: ExamId, plan: PlanId) -> bool {
    plan == PlanId::Both || exam_plan(exam) == plan
}

fn exam_plan(exam: ExamId) -> PlanId {
    match exam {
        ExamId::Sie => PlanId::Sie,
        ExamId::S7 => PlanId::S7,
    }
}

fn order_both(items: &mut [Question], plan: PlanId) {
    if plan != PlanId::Both {
        return;
    }
    items.sort_by_key(|item| item.exam != ExamId::Sie);
}

fn order_both_cards(items: &mut [Flashcard], plan: PlanId) {
    if plan != PlanId::Both {
        return;
    }
    items.sort_by_key(|item| item.exam != ExamId::Sie);
}
