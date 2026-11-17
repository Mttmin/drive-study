import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { questionUtterances, flashcardUtterance, lessonUtterances, clipKey } from "../shared/audioKey.js";

const root = new URL("..", import.meta.url).pathname;
const errors = [];

function fail(message) {
  errors.push(message);
}

function readJson(path) {
  try {
    return JSON.parse(readFileSync(join(root, path), "utf8"));
  } catch (error) {
    fail(`${path}: ${error.message}`);
    return null;
  }
}

function walk(dir, acc = []) {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) walk(full, acc);
    else if (entry.endsWith(".json")) acc.push(full.slice(root.length).replace(/^\//, ""));
  }
  return acc;
}

const meta = readJson("content/meta.json");
if (!meta) {
  report();
}

const exams = meta.exams ?? {};
for (const examId of ["SIE", "S7"]) {
  const exam = exams[examId];
  if (!exam) {
    fail(`meta.exams.${examId} missing`);
    continue;
  }
  for (const field of ["name", "questions", "minutes", "pass_percent", "sections"]) {
    if (exam[field] == null) fail(`${examId} missing ${field}`);
  }
  const weight = (exam.sections ?? []).reduce((sum, section) => sum + section.weight, 0);
  if (weight !== 100) fail(`${examId} section weights sum to ${weight}, expected 100`);
  const counted = (exam.sections ?? []).reduce((sum, section) => sum + section.questions, 0);
  if (counted !== exam.questions) fail(`${examId} section question counts sum to ${counted}, expected ${exam.questions}`);
  const ids = new Set();
  for (const section of exam.sections ?? []) {
    if (ids.has(section.id)) fail(`duplicate section ${section.id}`);
    ids.add(section.id);
    if (!section.title) fail(`${section.id} missing title`);
  }
}

const onDisk = walk(join(root, "content")).filter((path) => path !== "content/meta.json").sort();
const listed = [
  ...(meta.files?.lessons ?? []),
  ...(meta.files?.questions ?? []),
  ...(meta.files?.flashcards ?? []),
].sort();
if (JSON.stringify(onDisk) !== JSON.stringify(listed)) {
  fail(`meta.files does not match content on disk.\n disk: ${onDisk.join(", ")}\n list: ${listed.join(", ")}`);
}

const sectionOf = (examId, sectionId) =>
  (exams[examId]?.sections ?? []).some((section) => section.id === sectionId);

const lessons = new Map();
const questions = new Map();
const cards = new Map();

for (const path of meta.files?.lessons ?? []) {
  const lesson = readJson(path);
  if (!lesson) continue;
  checkLesson(path, lesson);
  if (lessons.has(lesson.id)) fail(`duplicate lesson id ${lesson.id}`);
  lessons.set(lesson.id, lesson);
}

for (const path of meta.files?.questions ?? []) {
  const rows = readJson(path);
  if (!Array.isArray(rows)) {
    fail(`${path} must be an array`);
    continue;
  }
  const fileSection = path.split("/").pop().replace(/\.json$/, "");
  for (const question of rows) {
    checkQuestion(path, question, fileSection);
    if (questions.has(question.id)) fail(`duplicate question id ${question.id}`);
    questions.set(question.id, question);
  }
}

for (const path of meta.files?.flashcards ?? []) {
  const rows = readJson(path);
  if (!Array.isArray(rows)) {
    fail(`${path} must be an array`);
    continue;
  }
  const fileSection = path.split("/").pop().replace(/\.json$/, "");
  for (const card of rows) {
    checkCard(path, card, fileSection);
    if (cards.has(card.id)) fail(`duplicate flashcard id ${card.id}`);
    cards.set(card.id, card);
  }
}

for (const lesson of lessons.values()) {
  for (const segment of lesson.segments ?? []) {
    if (segment.type === "check" && !questions.has(segment.question_id)) {
      fail(`lesson ${lesson.id} check ${segment.question_id} does not exist`);
    }
  }
}
for (const question of questions.values()) {
  if (question.lesson_id && !lessons.has(question.lesson_id)) {
    fail(`question ${question.id} lesson_id ${question.lesson_id} does not exist`);
  }
}

const manifest = readJson("audio/manifest.json");
if (manifest) {
  if (manifest.version !== 1) fail("audio manifest version must be 1");
  if (manifest.voice_id !== "natural-en-us-v1") fail("audio manifest voice_id must be natural-en-us-v1");
  if (!manifest.clips || typeof manifest.clips !== "object" || Array.isArray(manifest.clips)) {
    fail("audio manifest clips must be an object");
  } else {
    const known = new Map();
    for (const question of questions.values()) {
      for (const utterance of questionUtterances(question)) known.set(clipKey(manifest.voice_id, utterance.text), utterance.text);
    }
    for (const card of cards.values()) {
      const utterance = flashcardUtterance(card);
      known.set(clipKey(manifest.voice_id, utterance.text), utterance.text);
    }
    for (const lesson of lessons.values()) {
      for (const utterance of lessonUtterances(lesson)) known.set(clipKey(manifest.voice_id, utterance.text), utterance.text);
    }
    for (const [key, clip] of Object.entries(manifest.clips)) {
      if (!/^[0-9a-f]{40}$/.test(key)) fail(`clip key ${key} is not sha1 hex`);
      if (!clip.path || typeof clip.path !== "string") fail(`clip ${key} missing path`);
      if (clip.voice !== manifest.voice_id) fail(`clip ${key} voice does not match manifest`);
      if (known.has(key) && clipKey(manifest.voice_id, known.get(key)) !== key) {
        fail(`clip ${key} does not match its text`);
      }
    }
  }
}

report();

function checkLesson(path, lesson) {
  const folder = path.includes("/sie/") ? "SIE" : path.includes("/s7/") ? "S7" : "";
  for (const field of ["id", "exam", "section", "section_title", "title", "order", "est_minutes", "segments", "key_terms", "mnemonics", "summary"]) {
    if (lesson[field] == null) fail(`${path} ${lesson.id ?? "?"} missing ${field}`);
  }
  if (lesson.exam !== folder) fail(`${lesson.id} exam ${lesson.exam} does not match folder`);
  if (!sectionOf(lesson.exam, lesson.section)) fail(`${lesson.id} section ${lesson.section} is not in the blueprint`);
  if (!Array.isArray(lesson.segments) || lesson.segments.length === 0) fail(`${lesson.id} needs segments`);
  for (const segment of lesson.segments ?? []) {
    if (segment.type === "narration" && !segment.text) fail(`${lesson.id} narration missing text`);
    if (segment.type === "check" && !segment.question_id) fail(`${lesson.id} check missing question_id`);
    if (segment.type !== "narration" && segment.type !== "check") fail(`${lesson.id} bad segment type`);
  }
  if (lesson.sample != null && typeof lesson.sample !== "boolean") fail(`${lesson.id} sample must be boolean`);
}

function checkQuestion(path, question, fileSection) {
  const folder = path.includes("/sie/") ? "SIE" : "S7";
  const idOk = folder === "SIE" ? /^sie-q-\d{4}$/ : /^s7-q-\d{4}$/;
  if (!idOk.test(question.id ?? "")) fail(`${path} bad question id ${question.id}`);
  if (question.exam !== folder) fail(`${question.id} exam does not match folder`);
  if (question.section !== fileSection) fail(`${question.id} section ${question.section} does not match file ${fileSection}`);
  if (!sectionOf(question.exam, question.section)) fail(`${question.id} unknown section`);
  const minimums = {
    topic: 3,
    outline_ref: 4,
    stem: 12,
    spoken_stem: 12,
    explanation: 40,
    spoken_explanation: 40,
  };
  for (const [field, minimum] of Object.entries(minimums)) {
    if (typeof question[field] !== "string" || question[field].trim().length < minimum) {
      fail(`${question.id} weak or missing ${field}`);
    }
  }
  if (![1, 2, 3].includes(question.difficulty)) fail(`${question.id} difficulty must be 1, 2, or 3`);
  if (!Array.isArray(question.tags)) fail(`${question.id} tags must be an array`);
  if (question.type === "mc") {
    const keys = (question.choices ?? []).map((choice) => choice.key).join("");
    if (keys !== "ABCD") fail(`${question.id} multiple choice must be A B C D in order`);
  } else if (question.type === "tf") {
    const keys = (question.choices ?? []).map((choice) => choice.key).join("");
    if (keys !== "TF") fail(`${question.id} true/false must be T then F`);
  } else {
    fail(`${question.id} type must be mc or tf`);
  }
  if (!(question.choices ?? []).some((choice) => choice.key === question.answer)) {
    fail(`${question.id} answer ${question.answer} is not a choice`);
  }
  for (const choice of question.choices ?? []) {
    if (!choice.text || typeof choice.text !== "string") fail(`${question.id} choice ${choice.key} missing text`);
  }
  if (question.sample != null && typeof question.sample !== "boolean") fail(`${question.id} sample must be boolean`);
}

function checkCard(path, card, fileSection) {
  const folder = path.includes("/sie/") ? "SIE" : "S7";
  const idOk = folder === "SIE" ? /^sie-f-\d{4}$/ : /^s7-f-\d{4}$/;
  if (!idOk.test(card.id ?? "")) fail(`${path} bad flashcard id ${card.id}`);
  if (card.exam !== folder) fail(`${card.id} exam does not match folder`);
  if (card.section !== fileSection) fail(`${card.id} section does not match file`);
  if (!sectionOf(card.exam, card.section)) fail(`${card.id} unknown section`);
  for (const field of ["front", "back", "spoken"]) {
    if (typeof card[field] !== "string" || card[field].trim().length < 4) fail(`${card.id} weak or missing ${field}`);
  }
}

function report() {
  if (errors.length) {
    console.error(`Content check failed (${errors.length})`);
    for (const error of errors) console.error(`- ${error}`);
    process.exit(1);
  }
  console.log("Content check passed.");
}
