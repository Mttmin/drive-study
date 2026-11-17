import { drawWeighted, shuffle } from "./blueprint";
import type { ExamId, Flashcard, Lesson, PlanId, Question, SectionBlueprint } from "./types";

export interface NarrationItem {
  kind: "narration";
  id: string;
  title: string;
  text: string;
}

export interface QuestionItem {
  kind: "question";
  question: Question;
}

export interface CardItem {
  kind: "flashcard";
  card: Flashcard;
}

export type SessionItem = NarrationItem | QuestionItem | CardItem;

export type SessionMode = "lesson" | "quiz" | "rapid" | "mock" | "mistakes" | "drive";

export interface SessionPlan {
  id: string;
  title: string;
  mode: SessionMode;
  exam: PlanId;
  items: SessionItem[];
  listenOnly: boolean;
  timed: boolean;
  limitSeconds?: number;
  passPercent?: number;
  note?: string;
  estimatedSeconds: number;
  lessonId?: string;
}

const QUESTION_SEC = 50;
const CARD_SEC = 22;
const NARRATION_SEC = 75;
const DRIVE_TARGET_SEC = 20 * 60;

export function buildLesson(lesson: Lesson, questions: Question[], listenOnly: boolean): SessionPlan {
  const items: SessionItem[] = [];
  for (const segment of lesson.segments) {
    if (segment.type === "narration" && segment.text) {
      items.push({ kind: "narration", id: `${lesson.id}-narr`, title: lesson.title, text: segment.text });
    }
    if (segment.type === "check" && segment.question_id) {
      const question = questions.find((item) => item.id === segment.question_id);
      if (question) items.push({ kind: "question", question });
    }
  }
  if (lesson.summary) {
    items.push({ kind: "narration", id: `${lesson.id}-summary`, title: lesson.title, text: lesson.summary });
  }
  return {
    id: `lesson-${lesson.id}`,
    title: lesson.title,
    mode: "lesson",
    exam: lesson.exam,
    items,
    listenOnly,
    timed: false,
    estimatedSeconds: estimate(items),
    lessonId: lesson.id,
    note: lesson.sample ? "Sample lesson" : undefined,
  };
}

export function buildQuiz(options: {
  exam: PlanId;
  sectionId: string | null;
  count: number;
  questions: Question[];
  sections: SectionBlueprint[];
  listenOnly: boolean;
  rng?: () => number;
}): SessionPlan {
  const rng = options.rng ?? Math.random;
  const pool = filterExam(options.questions, options.exam).filter((question) =>
    options.sectionId ? question.section === options.sectionId : true,
  );
  let picked: Question[];
  if (options.sectionId || options.exam === "BOTH") {
    if (options.exam === "BOTH" && !options.sectionId) {
      const sie = pool.filter((question) => question.exam === "SIE");
      const s7 = pool.filter((question) => question.exam === "S7");
      const sieCount = Math.min(sie.length, Math.ceil(options.count * 0.6));
      const s7Count = Math.min(s7.length, options.count - sieCount);
      picked = shuffle(
        [
          ...shuffle([...sie], rng).slice(0, sieCount),
          ...shuffle([...s7], rng).slice(0, s7Count),
        ],
        rng,
      );
    } else {
      picked = shuffle([...pool], rng).slice(0, options.count);
    }
  } else {
    const weights = options.sections.filter((section) => pool.some((question) => question.section === section.id));
    picked = drawWeighted(pool, weights, options.count, rng);
  }
  const items: SessionItem[] = picked.map((question) => ({ kind: "question", question }));
  const label = options.sectionId ?? (options.exam === "BOTH" ? "SIE and Series 7" : options.exam === "S7" ? "Series 7" : "SIE");
  return {
    id: `quiz-${Date.now()}`,
    title: `Quiz · ${label}`,
    mode: "quiz",
    exam: options.exam,
    items,
    listenOnly: options.listenOnly,
    timed: false,
    estimatedSeconds: estimate(items),
    note: picked.length < options.count ? `The library has ${picked.length} matching questions.` : undefined,
  };
}

export function buildRapid(options: {
  exam: PlanId;
  sectionId: string | null;
  count: number;
  cards: Flashcard[];
  listenOnly: boolean;
  rng?: () => number;
}): SessionPlan {
  const rng = options.rng ?? Math.random;
  const pool = filterExam(options.cards, options.exam).filter((card) =>
    options.sectionId ? card.section === options.sectionId : true,
  );
  const picked = shuffle([...pool], rng).slice(0, options.count);
  return {
    id: `rapid-${Date.now()}`,
    title: "Rapid fire",
    mode: "rapid",
    exam: options.exam,
    items: picked.map((card) => ({ kind: "flashcard", card })),
    listenOnly: options.listenOnly,
    timed: false,
    estimatedSeconds: estimate(picked.map((card) => ({ kind: "flashcard", card }))),
  };
}

export function buildMock(options: {
  exam: ExamId;
  timed: boolean;
  questions: Question[];
  blueprint: { questions: number; minutes: number; pass_percent: number; sections: SectionBlueprint[] };
  listenOnly: boolean;
  rng?: () => number;
}): SessionPlan {
  const rng = options.rng ?? Math.random;
  const pool = options.questions.filter((question) => question.exam === options.exam);
  const picked = drawWeighted(pool, options.blueprint.sections, options.blueprint.questions, rng);
  const scaled = Math.max(60, Math.round(options.blueprint.minutes * 60 * (picked.length / options.blueprint.questions)));
  const short = picked.length < options.blueprint.questions;
  return {
    id: `mock-${options.exam}-${Date.now()}`,
    title: options.timed ? `${options.exam} mock · timed` : `${options.exam} mock · car`,
    mode: "mock",
    exam: options.exam,
    items: picked.map((question) => ({ kind: "question", question })),
    listenOnly: options.listenOnly,
    timed: options.timed,
    limitSeconds: options.timed ? scaled : undefined,
    passPercent: options.blueprint.pass_percent,
    estimatedSeconds: options.timed ? scaled : estimate(picked.map((question) => ({ kind: "question", question }))),
    note: short
      ? `Sample pool: ${picked.length} questions. A full ${options.exam} mock is ${options.blueprint.questions} scored questions. FINRA also adds 10 unscored pretest items on the real exam.`
      : "FINRA also adds 10 unscored pretest items on the real exam. This mock uses the scored count.",
  };
}

export function buildMistakes(options: {
  exam: PlanId;
  questions: Question[];
  cards: Flashcard[];
  dueIds: Set<string>;
  listenOnly: boolean;
}): SessionPlan {
  const questions = orderBoth(
    options.questions.filter((question) => options.dueIds.has(question.id) && matchesPlan(question.exam, options.exam)),
    options.exam,
  );
  const cards = orderBoth(
    options.cards.filter((card) => options.dueIds.has(card.id) && matchesPlan(card.exam, options.exam)),
    options.exam,
  );
  const items: SessionItem[] = [
    ...questions.map((question) => ({ kind: "question" as const, question })),
    ...cards.map((card) => ({ kind: "flashcard" as const, card })),
  ];
  return {
    id: `mistakes-${Date.now()}`,
    title: "Mistakes drive",
    mode: "mistakes",
    exam: options.exam,
    items,
    listenOnly: options.listenOnly,
    timed: false,
    estimatedSeconds: estimate(items),
  };
}

export function buildDrive(options: {
  exam: PlanId;
  questions: Question[];
  cards: Flashcard[];
  lessons: Lesson[];
  dueIds: Set<string>;
  weakSectionIds: string[];
  completedLessons: string[];
  listenOnly: boolean;
  rng?: () => number;
}): SessionPlan {
  const rng = options.rng ?? Math.random;
  const questions = options.questions.filter((question) => matchesPlan(question.exam, options.exam));
  const cards = options.cards.filter((card) => matchesPlan(card.exam, options.exam));
  const dueQuestions = orderBoth(
    questions.filter((question) => options.dueIds.has(question.id)),
    options.exam,
  );
  const dueCards = orderBoth(
    cards.filter((card) => options.dueIds.has(card.id)),
    options.exam,
  );
  const used = new Set(dueQuestions.map((question) => question.id));
  const sectionOrder = driveSectionOrder(options.exam, options.weakSectionIds, questions);
  const fresh: Question[] = [];
  for (const sectionId of sectionOrder) {
    const pool = shuffle(
      questions.filter((question) => question.section === sectionId && !used.has(question.id)),
      rng,
    );
    fresh.push(...pool);
  }

  const items: SessionItem[] = [];
  let seconds = 0;
  const narration = miniLesson(options.lessons, options.exam, options.completedLessons);
  if (narration) {
    items.push(narration);
    seconds += NARRATION_SEC;
  }

  let mistakeIndex = 0;
  let freshIndex = 0;
  let cardIndex = 0;
  const pattern = ["mistake", "mistake", "fresh", "card", "mistake", "fresh"] as const;
  let spins = 0;
  while (seconds < DRIVE_TARGET_SEC && spins < 8) {
    spins += 1;
    let added = false;
    for (const step of pattern) {
      if (seconds >= DRIVE_TARGET_SEC) break;
      if (step === "mistake" && mistakeIndex < dueQuestions.length) {
        items.push({ kind: "question", question: dueQuestions[mistakeIndex] });
        mistakeIndex += 1;
        seconds += QUESTION_SEC;
        added = true;
      } else if (step === "fresh" && freshIndex < fresh.length) {
        items.push({ kind: "question", question: fresh[freshIndex] });
        freshIndex += 1;
        seconds += QUESTION_SEC;
        added = true;
      } else if (step === "card" && cardIndex < dueCards.length) {
        items.push({ kind: "flashcard", card: dueCards[cardIndex] });
        cardIndex += 1;
        seconds += CARD_SEC;
        added = true;
      }
    }
    if (!added) break;
  }
  while (seconds < DRIVE_TARGET_SEC && freshIndex < fresh.length) {
    items.push({ kind: "question", question: fresh[freshIndex] });
    freshIndex += 1;
    seconds += QUESTION_SEC;
  }

  return {
    id: `drive-${Date.now()}`,
    title: "Drive, 20 minutes",
    mode: "drive",
    exam: options.exam,
    items,
    listenOnly: options.listenOnly,
    timed: false,
    estimatedSeconds: seconds,
    note: "Due mistakes, weak sections, and a short lesson. SIE comes first when you study both.",
  };
}

function miniLesson(lessons: Lesson[], exam: PlanId, completed: string[]): NarrationItem | null {
  const ordered = [...lessons]
    .filter((lesson) => matchesPlan(lesson.exam, exam))
    .sort((a, b) => Number(b.exam === "SIE") - Number(a.exam === "SIE") || a.order - b.order);
  const next = ordered.find((lesson) => !completed.includes(lesson.id)) ?? null;
  if (!next) return null;
  const segment = next.segments.find((item) => item.type === "narration" && item.text);
  if (!segment?.text) return null;
  return { kind: "narration", id: `${next.id}#mini`, title: next.title, text: segment.text };
}

function driveSectionOrder(exam: PlanId, weak: string[], questions: Question[]): string[] {
  const present = new Set(questions.filter((question) => matchesPlan(question.exam, exam)).map((question) => question.section));
  const orderedWeak = weak.filter((id) => present.has(id));
  const rest = [...present].filter((id) => !orderedWeak.includes(id));
  const combined = [...orderedWeak, ...rest];
  if (exam !== "BOTH") return combined;
  return [...combined.filter((id) => id.startsWith("SIE")), ...combined.filter((id) => id.startsWith("S7"))];
}

function estimate(items: SessionItem[]): number {
  return items.reduce((sum, item) => {
    if (item.kind === "narration") return sum + NARRATION_SEC;
    if (item.kind === "flashcard") return sum + CARD_SEC;
    return sum + QUESTION_SEC;
  }, 0);
}

function matchesPlan(exam: ExamId, plan: PlanId): boolean {
  return plan === "BOTH" || plan === exam;
}

function filterExam<T extends { exam: ExamId }>(items: T[], plan: PlanId): T[] {
  return items.filter((item) => matchesPlan(item.exam, plan));
}

function orderBoth<T extends { exam: ExamId }>(items: T[], plan: PlanId): T[] {
  if (plan !== "BOTH") return items;
  return [...items.filter((item) => item.exam === "SIE"), ...items.filter((item) => item.exam === "S7")];
}
