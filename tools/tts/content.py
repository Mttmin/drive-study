"""Load content JSON into the utterance list documented in docs/audio-manifest.md.

If scripts/list-utterances.mjs exists, that output wins. The Python path
mirrors shared/audioKey.js for the period before the script lands.
"""

from __future__ import annotations

import json
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

from lexicon import VOICE_ID, choices_readout, clip_key, clip_path, normalize_spoken

EXAMS = ("sie", "s7")


def _read_json(path: Path):
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def _as_list(data, *keys: str) -> list:
    if isinstance(data, list):
        return data
    if isinstance(data, dict):
        for key in keys:
            value = data.get(key)
            if isinstance(value, list):
                return value
        if "id" in data:
            return [data]
    return []


def load_bank(root: Path) -> dict:
    content = root / "content"
    questions: list[dict] = []
    flashcards: list[dict] = []
    lessons: list[dict] = []
    titles: dict[str, str] = {}
    meta_path = content / "meta.json"
    if meta_path.is_file():
        _collect_titles(_read_json(meta_path), titles)
    for exam in EXAMS:
        qdir = content / exam / "questions"
        if qdir.is_dir():
            for path in sorted(qdir.glob("*.json")):
                questions.extend(_as_list(_read_json(path), "questions", "items"))
        fdir = content / exam / "flashcards"
        if fdir.is_dir():
            for path in sorted(fdir.glob("*.json")):
                flashcards.extend(_as_list(_read_json(path), "flashcards", "cards", "items"))
        ldir = content / exam / "lessons"
        if ldir.is_dir():
            for path in sorted(ldir.glob("*.json")):
                lessons.extend(_as_list(_read_json(path), "lessons", "items"))
    for lesson in lessons:
        section = str(lesson.get("section") or "")
        title = lesson.get("section_title") or lesson.get("sectionTitle")
        if section and title:
            titles.setdefault(section, str(title))
    questions.sort(key=lambda q: str(q.get("id", "")))
    flashcards.sort(key=lambda c: str(c.get("id", "")))
    lessons.sort(
        key=lambda lesson: (
            str(lesson.get("exam", "")),
            int(lesson.get("order") or 0),
            str(lesson.get("id", "")),
        )
    )
    return {
        "questions": questions,
        "flashcards": flashcards,
        "lessons": lessons,
        "section_titles": titles,
    }


def _collect_titles(node, titles: dict[str, str]) -> None:
    if isinstance(node, dict):
        sid = node.get("id") or node.get("section")
        title = node.get("title") or node.get("section_title") or node.get("name")
        if isinstance(sid, str) and isinstance(title, str) and sid[:3] in {"SIE", "S7-"} or (
            isinstance(sid, str) and isinstance(title, str) and sid.startswith(("SIE", "S7"))
        ):
            titles.setdefault(sid, title)
        for value in node.values():
            _collect_titles(value, titles)
    elif isinstance(node, list):
        for value in node:
            _collect_titles(value, titles)


def utterances_from_bank(bank: dict) -> list[dict]:
    items: list[dict] = []
    for question in bank["questions"]:
        qid = str(question.get("id") or "")
        common = {
            "ref": qid,
            "section": question.get("section") or "",
            "exam": question.get("exam") or "",
        }
        stem = question.get("spoken_stem") or question.get("stem") or ""
        explanation = question.get("spoken_explanation") or question.get("explanation") or ""
        items.append({**common, "kind": "spoken_stem", "text": normalize_spoken(stem)})
        items.append({**common, "kind": "choices", "text": normalize_spoken(choices_readout(question))})
        items.append({**common, "kind": "spoken_explanation", "text": normalize_spoken(explanation)})
    for card in bank["flashcards"]:
        items.append(
            {
                "kind": "flashcard",
                "ref": str(card.get("id") or ""),
                "section": card.get("section") or "",
                "exam": card.get("exam") or "",
                "text": normalize_spoken(card.get("spoken") or ""),
            }
        )
    for lesson in bank["lessons"]:
        for index, segment in enumerate(lesson.get("segments") or []):
            if not isinstance(segment, dict) or segment.get("type") != "narration":
                continue
            items.append(
                {
                    "kind": "narration",
                    "ref": f"{lesson.get('id')}#{index}",
                    "section": lesson.get("section") or "",
                    "exam": lesson.get("exam") or "",
                    "text": normalize_spoken(segment.get("text") or ""),
                    "lesson_id": lesson.get("id"),
                    "index": index,
                }
            )
    keyed = []
    for item in items:
        text = item["text"]
        if not text:
            continue
        key = clip_key(VOICE_ID, text)
        item["key"] = key
        item["path"] = clip_path(key)
        keyed.append(item)
    return keyed


def load_utterances(root: Path, bank: dict) -> list[dict]:
    script = root / "scripts" / "list-utterances.mjs"
    if script.is_file():
        proc = subprocess.run(
            ["node", str(script)],
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )
        data = None
        if proc.returncode == 0 and proc.stdout.strip():
            raw = proc.stdout
            start, end = raw.find("["), raw.rfind("]")
            if start != -1 and end > start:
                try:
                    data = json.loads(raw[start : end + 1])
                except json.JSONDecodeError:
                    data = None
        if not isinstance(data, list):
            print(
                "list-utterances.mjs did not return a JSON array. Using the Python utterance list.",
                file=sys.stderr,
            )
            if proc.stderr.strip():
                print(proc.stderr.strip()[:500], file=sys.stderr)
            return utterances_from_bank(bank)
        cleaned = []
        for item in data:
            text = normalize_spoken(item.get("text"))
            if not text:
                continue
            key = clip_key(VOICE_ID, text)
            if item.get("key") and item["key"] != key:
                print(f"hash mismatch for {item.get('ref')}; using audioKey.js", file=sys.stderr)
            cleaned.append(
                {
                    "kind": item.get("kind"),
                    "ref": item.get("ref") or "",
                    "section": item.get("section") or "",
                    "exam": item.get("exam") or "",
                    "text": text,
                    "key": key,
                    "path": item.get("path") or clip_path(key),
                }
            )
        return cleaned
    return utterances_from_bank(bank)


def questions_by_section(questions: list[dict]) -> dict[str, list[dict]]:
    grouped: dict[str, list[dict]] = defaultdict(list)
    for question in questions:
        grouped[str(question.get("section") or "ungrouped")].append(question)
    for section in grouped:
        grouped[section].sort(key=lambda q: str(q.get("id", "")))
    return dict(grouped)


def chunk_questions(items: list, low: int = 15, high: int = 15) -> list[list]:
    """Split into episodes of at most `high` questions, as even as possible.

    Fifteen is the cap because a 20-question episode of this bank runs about
    20 minutes, and the episodes should stay near 10-15 minutes. Sections of
    `high` or fewer stay one episode.
    """
    del low
    n = len(items)
    if n == 0:
        return []
    if n <= high:
        return [list(items)]
    chunks_n = (n + high - 1) // high
    base, extra = divmod(n, chunks_n)
    out: list[list] = []
    index = 0
    for chunk_i in range(chunks_n):
        take = base + (1 if chunk_i < extra else 0)
        out.append(list(items[index : index + take]))
        index += take
    return out


def exam_spoken(exam: str) -> str:
    token = (exam or "").strip().upper()
    if token == "SIE":
        return "the S I E"
    if token in {"S7", "SERIES 7", "SERIES7"}:
        return "the Series 7"
    return exam or "the exam"
