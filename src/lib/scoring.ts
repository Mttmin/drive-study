export interface ScoredResponse {
  section: string;
  correct: boolean;
}

export interface SectionScore {
  correct: number;
  total: number;
  percent: number;
}

export interface ExamScore {
  correct: number;
  total: number;
  percent: number;
  passed: boolean;
  bySection: Record<string, SectionScore>;
}

export function scoreExam(responses: ScoredResponse[], passPercent: number): ExamScore {
  const total = responses.length;
  const correct = responses.reduce((sum, row) => sum + (row.correct ? 1 : 0), 0);
  const ratio = total === 0 ? 0 : correct / total;
  const bySection: Record<string, SectionScore> = {};
  for (const row of responses) {
    const bucket = bySection[row.section] ?? { correct: 0, total: 0, percent: 0 };
    bucket.total += 1;
    if (row.correct) bucket.correct += 1;
    bucket.percent = round1((bucket.correct / bucket.total) * 100);
    bySection[row.section] = bucket;
  }
  return {
    correct,
    total,
    percent: round1(ratio * 100),
    passed: total > 0 && ratio + 1e-12 >= passPercent / 100,
    bySection,
  };
}

export interface ReadinessSection {
  id: string;
  weight: number;
}

export interface ReadinessAttempt {
  section: string;
  topic?: string;
  correct: boolean;
}

export interface WeakSection {
  id: string;
  accuracy: number;
  attempts: number;
}

export interface WeakTopic {
  topic: string;
  accuracy: number;
  attempts: number;
}

export interface Readiness {
  score: number | null;
  coverage: number;
  weak: WeakSection[];
}

/**
 * Readiness blends section accuracy (sections with at least `minAttempts`)
 * by blueprint weight, then scales it by library coverage.
 * score = round(100 * weightedAccuracy * (0.55 + 0.45 * coverage)).
 * Coverage is distinct questions seen divided by questions in the library.
 */
export function readiness(options: {
  sections: ReadinessSection[];
  attempts: ReadinessAttempt[];
  seen: number;
  pool: number;
  minAttempts?: number;
}): Readiness {
  const minAttempts = options.minAttempts ?? 3;
  const grouped = new Map<string, { correct: number; total: number }>();
  for (const attempt of options.attempts) {
    const bucket = grouped.get(attempt.section) ?? { correct: 0, total: 0 };
    bucket.total += 1;
    if (attempt.correct) bucket.correct += 1;
    grouped.set(attempt.section, bucket);
  }
  let weightSum = 0;
  let weighted = 0;
  const weak: WeakSection[] = [];
  for (const section of options.sections) {
    const bucket = grouped.get(section.id);
    if (!bucket || bucket.total < minAttempts) continue;
    const accuracy = bucket.correct / bucket.total;
    weightSum += section.weight;
    weighted += accuracy * section.weight;
    if (accuracy < 0.7) weak.push({ id: section.id, accuracy, attempts: bucket.total });
  }
  weak.sort((a, b) => a.accuracy - b.accuracy || a.id.localeCompare(b.id));
  const coverage = options.pool <= 0 ? 0 : Math.min(1, options.seen / options.pool);
  if (weightSum === 0) return { score: null, coverage, weak };
  const score = Math.round(100 * (weighted / weightSum) * (0.55 + 0.45 * coverage));
  return { score, coverage, weak };
}

export function weakTopics(attempts: ReadinessAttempt[], minAttempts = 2): WeakTopic[] {
  const grouped = new Map<string, { correct: number; total: number }>();
  for (const attempt of attempts) {
    if (!attempt.topic) continue;
    const bucket = grouped.get(attempt.topic) ?? { correct: 0, total: 0 };
    bucket.total += 1;
    if (attempt.correct) bucket.correct += 1;
    grouped.set(attempt.topic, bucket);
  }
  const weak: WeakTopic[] = [];
  for (const [topic, bucket] of grouped) {
    if (bucket.total < minAttempts) continue;
    const accuracy = bucket.correct / bucket.total;
    if (accuracy < 0.7) weak.push({ topic, accuracy, attempts: bucket.total });
  }
  weak.sort((a, b) => a.accuracy - b.accuracy || a.topic.localeCompare(b.topic));
  return weak;
}

function round1(value: number): number {
  return Math.round(value * 10) / 10;
}
