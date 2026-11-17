import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  VOICE_ID,
  clipKey,
  clipPath,
  flashcardUtterance,
  lessonUtterances,
  questionUtterances,
} from "../shared/audioKey.js";

const root = new URL("..", import.meta.url).pathname;
const meta = JSON.parse(readFileSync(join(root, "content/meta.json"), "utf8"));

function load(path) {
  return JSON.parse(readFileSync(join(root, path), "utf8"));
}

const utterances = [];
for (const path of meta.files.questions) {
  for (const question of load(path)) utterances.push(...questionUtterances(question));
}
for (const path of meta.files.flashcards) {
  for (const card of load(path)) utterances.push(flashcardUtterance(card));
}
for (const path of meta.files.lessons) utterances.push(...lessonUtterances(load(path)));

const rows = utterances.map((utterance) => {
  const key = clipKey(VOICE_ID, utterance.text);
  return { ...utterance, voice: VOICE_ID, key, path: clipPath(key) };
});

process.stdout.write(`${JSON.stringify(rows, null, 2)}\n`);
