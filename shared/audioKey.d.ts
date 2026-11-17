export const VOICE_ID: string;

export function normalizeSpokenText(text: string): string;

export function clipKey(voiceId: string, text: string): string;

export function choicesReadout(question: {
  type: string;
  choices?: { key: string; text: string }[];
}): string;

export interface Utterance {
  kind: "spoken_stem" | "choices" | "spoken_explanation" | "flashcard" | "narration";
  ref: string;
  section: string;
  exam: string;
  text: string;
}

export function questionUtterances(question: {
  id: string;
  section: string;
  exam: string;
  type: string;
  stem: string;
  spoken_stem?: string;
  explanation?: string;
  spoken_explanation?: string;
  choices?: { key: string; text: string }[];
}): Utterance[];

export function flashcardUtterance(card: {
  id: string;
  section: string;
  exam: string;
  spoken: string;
}): Utterance;

export function lessonUtterances(lesson: {
  id: string;
  section: string;
  exam: string;
  segments?: { type: string; text?: string }[];
}): Utterance[];

export function clipPath(key: string): string;
