import { readdirSync, readFileSync } from "node:fs";
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

function discover(kind) {
  if (meta.files?.[kind]?.length) return meta.files[kind];
  const found = [];
  for (const exam of ["sie", "s7"]) {
    const dir = join(root, "content", exam, kind);
    let names = [];
    try {
      names = readdirSync(dir).filter((name) => name.endsWith(".json")).sort();
    } catch {
      continue;
    }
    for (const name of names) found.push(join("content", exam, kind, name));
  }
  return found;
}

const utterances = [];
for (const path of discover("questions")) {
  for (const question of load(path)) utterances.push(...questionUtterances(question));
}
for (const path of discover("flashcards")) {
  for (const card of load(path)) utterances.push(flashcardUtterance(card));
}
for (const path of discover("lessons")) utterances.push(...lessonUtterances(load(path)));

const rows = utterances.map((utterance) => {
  const key = clipKey(VOICE_ID, utterance.text);
  return { ...utterance, voice: VOICE_ID, key, path: clipPath(key) };
});

process.stdout.write(`${JSON.stringify(rows, null, 2)}\n`);
