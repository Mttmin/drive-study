"""Lesson and quiz MP3s plus catalog.json for scripts/build-podcast-feed.mjs.

Lesson episodes are the narration clips, in order, with a short pause.
Quiz episodes are stem, choices, about 6 seconds, then the spoken explanation.
A spoken intro is mixed into the episode only. It is not a manifest clip.
"""

from __future__ import annotations

import json
from datetime import datetime, timezone
from email.utils import format_datetime
from pathlib import Path

from content import chunk_questions, exam_spoken, questions_by_section
from encode import concat_mp3, make_silence, mp3_duration
from lexicon import choices_readout, normalize_spoken


def _exam_label(exam) -> str:
    token = str(exam or "").upper()
    if token == "SIE":
        return "SIE"
    if token in {"S7", "SERIES 7"}:
        return "Series 7"
    return str(exam or "Study")


class EpisodeBuilder:
    def __init__(self, ffmpeg: str, synth, cache: Path, podcast_dir: Path, bitrate: str):
        self.ffmpeg = ffmpeg
        self.synth = synth
        self.cache = cache
        self.podcast_dir = podcast_dir
        self.bitrate = bitrate
        self.gap = cache / "silence-0.7s.mp3"
        self.pause = cache / f"silence-6s-{bitrate}.mp3"
        make_silence(ffmpeg, self.gap, 0.7, bitrate)
        make_silence(ffmpeg, self.pause, 6.0, bitrate)
        self.episodes: list[dict] = []

    def _speak(self, text: str) -> Path:
        # Intros live only inside the episode. They are not manifest keys.
        return self.synth(normalize_spoken(text), published=False)

    def add_lesson(self, lesson: dict, narration: list[dict]) -> None:
        if not narration:
            return
        lid = str(lesson.get("id") or "lesson")
        title = str(lesson.get("title") or lid)
        section = str(lesson.get("section") or "")
        section_title = str(lesson.get("section_title") or section)
        intro = (
            f"This is Drive Study. {exam_spoken(str(lesson.get('exam') or ''))}. "
            f"Lesson. {title}. Section {section}. {section_title}."
        )
        parts = [self._speak(intro)]
        for index, item in enumerate(narration):
            if index:
                parts.append(self.gap)
            parts.append(self.synth(item["text"], published=True))
        parts.append(self._speak(f"That was {title}."))
        self._write(
            filename=f"lesson-{lid}.mp3",
            guid=f"drive-study-lesson-{lid}",
            title=f"{_exam_label(lesson.get('exam'))} lesson. {title}",
            description=f"{section_title}. Narrated lesson.",
            parts=parts,
            group="lesson",
        )

    def add_quizzes(self, bank: dict) -> None:
        grouped = questions_by_section(bank["questions"])
        for section in sorted(grouped):
            chunks = chunk_questions(grouped[section])
            for index, chunk in enumerate(chunks, start=1):
                self._add_quiz(section, chunk, index, len(chunks), bank["section_titles"])

    def _add_quiz(self, section, questions, part_index, part_count, titles) -> None:
        exam_raw = str(questions[0].get("exam") or "")
        section_title = titles.get(section) or section
        count = len(questions)
        part = f" Part {part_index} of {part_count}." if part_count > 1 else ""
        intro = (
            f"This is Drive Study. {exam_spoken(exam_raw)} audio quiz. "
            f"Section {section}. {section_title}.{part} "
            f"{count} questions. "
            "After each question and its choices, there are six seconds of silence, "
            "then the answer and a short explanation."
        )
        parts = [self._speak(intro)]
        for number, question in enumerate(questions, start=1):
            parts.append(self._speak(f"This is question number {number}."))
            stem = normalize_spoken(question.get("spoken_stem") or question.get("stem") or "")
            choices = normalize_spoken(choices_readout(question))
            explanation = normalize_spoken(
                question.get("spoken_explanation") or question.get("explanation") or ""
            )
            if stem:
                parts.append(self.synth(stem, published=True))
            if choices:
                parts.append(self.synth(choices, published=True))
            parts.append(self.pause)
            if explanation:
                parts.append(self.synth(explanation, published=True))
        parts.append(self._speak(f"End of quiz. {count} questions on {section_title}."))
        if part_count == 1:
            filename = f"quiz-{section}.mp3"
            guid = f"drive-study-quiz-{section}"
            title = f"{_exam_label(exam_raw)} quiz. {section_title}"
        else:
            filename = f"quiz-{section}-part-{part_index:02d}.mp3"
            guid = f"drive-study-quiz-{section}-part-{part_index:02d}"
            title = f"{_exam_label(exam_raw)} quiz. {section_title} ({part_index} of {part_count})"
        self._write(
            filename=filename,
            guid=guid,
            title=title,
            description=(
                f"{count} questions on {section_title}. "
                "Each item is the question, six seconds of silence, then the answer."
            ),
            parts=parts,
            group="quiz",
        )

    def _write(self, *, filename, guid, title, description, parts, group) -> None:
        dest = self.podcast_dir / filename
        concat_mp3(self.ffmpeg, parts, dest, title=title)
        seconds = mp3_duration(self.ffmpeg, dest)
        self.episodes.append(
            {
                "file": filename,
                "guid": guid,
                "title": title,
                "description": description,
                "seconds": round(seconds, 2),
                "bytes": dest.stat().st_size,
                "group": group,
                "content_sha1": _token(parts),
            }
        )

    def write_catalog(self, path: Path, dates: dict) -> dict:
        now = format_datetime(datetime.now(timezone.utc))
        updated = dict(dates)
        for episode in self.episodes:
            previous = updated.get(episode["guid"]) or {}
            if previous.get("content_sha1") == episode["content_sha1"] and previous.get("pubDate"):
                episode["pubDate"] = previous["pubDate"]
            else:
                episode["pubDate"] = now
            updated[episode["guid"]] = {
                "pubDate": episode["pubDate"],
                "content_sha1": episode["content_sha1"],
            }
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            json.dumps({"episodes": self.episodes}, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
        return updated


def _token(parts: list[Path]) -> str:
    import hashlib

    digest = hashlib.sha1()
    for part in parts:
        digest.update(part.name.encode())
        digest.update(str(part.stat().st_size).encode())
    return digest.hexdigest()


def write_cover(path: Path) -> None:
    from PIL import Image, ImageDraw, ImageFont

    path.parent.mkdir(parents=True, exist_ok=True)
    size = 1600
    image = Image.new("RGB", (size, size), "#12181f")
    draw = ImageDraw.Draw(image)
    draw.rounded_rectangle((96, 96, size - 96, size - 96), radius=48, fill="#1b2430")
    draw.rectangle((160, 760, size - 160, 804), fill="#e0a45a")
    bold = _font(92, True)
    regular = _font(42, False)
    small = _font(32, False)
    draw.text((160, 430), "DRIVE STUDY", font=bold, fill="#f4f1ea")
    draw.text((160, 900), "SIE   and   Series 7", font=regular, fill="#f4f1ea")
    draw.text((160, 1180), "Lessons and audio quizzes", font=small, fill="#c5d0dc")
    draw.text((160, 1240), "for the drive", font=small, fill="#c5d0dc")
    image.save(path, "JPEG", quality=90, optimize=True)


def _font(size: int, bold: bool):
    from PIL import ImageFont

    candidate = (
        "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf"
        if bold
        else "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf"
    )
    if Path(candidate).is_file():
        return ImageFont.truetype(candidate, size=size)
    fallback = (
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"
        if bold
        else "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"
    )
    return ImageFont.truetype(fallback, size=size)


def load_dates(path: Path) -> dict:
    if not path.is_file():
        return {}
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return {}
    return data if isinstance(data, dict) else {}


def save_dates(path: Path, dates: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(dates, indent=2, sort_keys=True) + "\n", encoding="utf-8")
