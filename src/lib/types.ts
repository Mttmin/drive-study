export type ExamId = "SIE" | "S7";
export type PlanId = ExamId | "BOTH";
export type VoiceSource = "downloaded" | "streamed" | "phone";
export type AnswerMode = "spoken" | "listen";

export interface Choice {
  key: string;
  text: string;
}

export interface Question {
  id: string;
  exam: ExamId;
  section: string;
  topic: string;
  lesson_id?: string;
  difficulty: 1 | 2 | 3;
  type: "mc" | "tf";
  stem: string;
  choices: Choice[];
  answer: string;
  spoken_stem: string;
  explanation: string;
  spoken_explanation: string;
  tags: string[];
  outline_ref: string;
  sample?: boolean;
}

export interface Flashcard {
  id: string;
  exam: ExamId;
  section: string;
  front: string;
  back: string;
  spoken: string;
  sample?: boolean;
}

export interface LessonSegment {
  type: "narration" | "check";
  text?: string;
  question_id?: string;
}

export interface Lesson {
  id: string;
  exam: ExamId;
  section: string;
  section_title: string;
  title: string;
  order: number;
  est_minutes: number;
  segments: LessonSegment[];
  key_terms: { term: string; definition: string }[];
  mnemonics: string[];
  summary: string;
  sample?: boolean;
}

export interface SectionBlueprint {
  id: string;
  title: string;
  weight: number;
  questions: number;
}

export interface ExamBlueprint {
  name: string;
  questions: number;
  minutes: number;
  pass_percent: number;
  sections: SectionBlueprint[];
}

export interface Library {
  disclaimer: string;
  voice_id: string;
  exams: Record<ExamId, ExamBlueprint>;
  lessons: Lesson[];
  questions: Question[];
  cards: Flashcard[];
}

export interface Settings {
  plan: PlanId;
  answerMode: AnswerMode;
  voiceSource: VoiceSource;
  rate: number;
  pauseSeconds: number;
  voiceURI: string | null;
  seenIntro: boolean;
  completedLessons: string[];
}

export const DEFAULT_SETTINGS: Settings = {
  plan: "BOTH",
  answerMode: "spoken",
  voiceSource: "downloaded",
  rate: 1,
  pauseSeconds: 6,
  voiceURI: null,
  seenIntro: false,
  completedLessons: [],
};

export interface ClipEntry {
  path: string;
  voice: string;
  bytes?: number;
  duration_ms?: number;
  content_type?: string;
  kind?: string;
  ref?: string;
  section?: string;
}

export interface AudioManifest {
  version: number;
  algorithm: string;
  voice_id: string;
  clips: Record<string, ClipEntry>;
}

export const EMPTY_MANIFEST: AudioManifest = {
  version: 1,
  algorithm: "sha1-voice-newline-text-nfc-collapsed-ws",
  voice_id: "natural-en-us-v1",
  clips: {},
};
