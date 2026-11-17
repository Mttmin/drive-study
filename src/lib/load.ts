import { EMPTY_MANIFEST, type AudioManifest, type ExamId, type Flashcard, type Lesson, type Library, type Question } from "./types";

interface MetaFile {
  disclaimer: string;
  voice_id: string;
  files: { lessons: string[]; questions: string[]; flashcards: string[] };
  exams: Library["exams"];
}

export async function loadLibrary(): Promise<Library> {
  const meta = await fetchJson<MetaFile>("content/meta.json");
  const lessons = (await Promise.all(meta.files.lessons.map((path) => fetchJson<Lesson>(path)))).sort(
    (a, b) => examOrder(a.exam) - examOrder(b.exam) || a.order - b.order,
  );
  const questionGroups = await Promise.all(meta.files.questions.map((path) => fetchJson<Question[]>(path)));
  const cardGroups = await Promise.all(meta.files.flashcards.map((path) => fetchJson<Flashcard[]>(path)));
  return {
    disclaimer: meta.disclaimer,
    voice_id: meta.voice_id,
    exams: meta.exams,
    lessons,
    questions: questionGroups.flat(),
    cards: cardGroups.flat(),
  };
}

export async function loadManifest(): Promise<AudioManifest> {
  try {
    const manifest = await fetchJson<AudioManifest>("audio/manifest.json");
    if (!manifest.clips) manifest.clips = {};
    return manifest;
  } catch {
    return EMPTY_MANIFEST;
  }
}

async function fetchJson<T>(path: string): Promise<T> {
  const response = await fetch(`${import.meta.env.BASE_URL}${path}`);
  if (!response.ok) throw new Error(`Could not load ${path}`);
  return response.json() as Promise<T>;
}

function examOrder(exam: ExamId): number {
  return exam === "SIE" ? 0 : 1;
}
