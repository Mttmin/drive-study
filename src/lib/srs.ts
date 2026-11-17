import type { ExamId } from "./types";

export interface SrsMeta {
  id: string;
  kind: "question" | "flashcard";
  exam: ExamId;
  section: string;
}

export interface SrsCard extends SrsMeta {
  ef: number;
  intervalDays: number;
  reps: number;
  lapses: number;
  due: number;
  correctDates: string[];
  graduated: boolean;
  lastReviewed: number | null;
}

export type ReviewOutcome = "correct" | "wrong" | "guessed";

const DAY_MS = 24 * 60 * 60 * 1000;
const TEN_MIN_MS = 10 * 60 * 1000;
const EF_MIN = 1.3;

export function dateKey(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function nextEf(ef: number, quality: number): number {
  const gap = 5 - quality;
  const value = ef + (0.1 - gap * (0.08 + gap * 0.02));
  return Math.max(EF_MIN, Math.round(value * 100) / 100);
}

export function blankCard(meta: SrsMeta): SrsCard {
  return {
    ...meta,
    ef: 2.5,
    intervalDays: 0,
    reps: 0,
    lapses: 0,
    due: 0,
    correctDates: [],
    graduated: false,
    lastReviewed: null,
  };
}

/**
 * SM-2 intervals, plus a graduation rule: three correct reviews on
 * different local dates. A same-day repeat does not add a day or
 * move the interval. A wrong answer or a flagged guess clears the
 * streak. Wrong answers are due again in 10 minutes. Guesses are due
 * tomorrow and do not count as a confident day.
 */
export function reviewCard(prev: SrsCard | null, meta: SrsMeta, outcome: ReviewOutcome, now: Date): SrsCard {
  const card: SrsCard = prev
    ? { ...prev, correctDates: [...prev.correctDates] }
    : blankCard(meta);
  card.lastReviewed = now.getTime();
  const today = dateKey(now);

  if (outcome === "wrong" || outcome === "guessed") {
    card.correctDates = [];
    card.graduated = false;
    card.reps = 0;
    card.intervalDays = 1;
    if (outcome === "wrong") {
      card.lapses += 1;
      card.ef = nextEf(card.ef, 2);
      card.due = now.getTime() + TEN_MIN_MS;
    } else {
      card.ef = nextEf(card.ef, 3);
      card.due = now.getTime() + DAY_MS;
    }
    return card;
  }

  if (card.correctDates[card.correctDates.length - 1] === today) return card;

  card.ef = nextEf(card.ef, 5);
  card.correctDates.push(today);
  if (card.correctDates.length >= 3) card.graduated = true;
  if (card.reps === 0) card.intervalDays = 1;
  else if (card.reps === 1) card.intervalDays = 3;
  else card.intervalDays = Math.max(1, Math.round(card.intervalDays * card.ef));
  card.reps += 1;
  card.due = now.getTime() + card.intervalDays * DAY_MS;
  return card;
}

export function isDue(card: SrsCard, now: Date): boolean {
  return !card.graduated && card.due <= now.getTime();
}

export function streakFromDates(dates: string[], today: string): number {
  const set = new Set(dates);
  const cursor = new Date(`${today}T12:00:00`);
  if (!set.has(today)) cursor.setDate(cursor.getDate() - 1);
  let count = 0;
  while (set.has(dateKey(cursor))) {
    count += 1;
    cursor.setDate(cursor.getDate() - 1);
  }
  return count;
}
