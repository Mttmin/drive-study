#!/usr/bin/env python3
"""Build Kokoro clips and podcast episodes for Drive Study.

The clip key is shared/audioKey.js: sha1(voice_id + newline + NFC text with
whitespace collapsed). voice_id is natural-en-us-v1. The Kokoro voice that
speaks those words is af_heart unless --kokoro-voice is set.

Writes under --out (default build/audio-publish), never over the stub
audio/manifest.json on main. `make audio` publishes that tree to the
orphan branch `audio`.
"""

from __future__ import annotations

import argparse
import json
import sys
import time
import traceback
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from content import load_bank, load_utterances  # noqa: E402
from encode import (  # noqa: E402
    directory_size,
    encode_speech,
    find_ffmpeg,
    mp3_duration,
    write_wav,
)
from engine import GpuStarved, load_engine  # noqa: E402
from lexicon import VOICE_ID, clip_path  # noqa: E402
from podcast import EpisodeBuilder, load_dates, save_dates  # noqa: E402

KOKORO_VOICE = "af_heart"
BITRATE = "40k"
SIZE_BUDGET = 900 * 1024 * 1024


class ClipStore:
    def __init__(self, out_root: Path, engine, ffmpeg: str, bitrate: str, force: bool):
        self.out_root = out_root
        self.engine = engine
        self.ffmpeg = ffmpeg
        self.bitrate = bitrate
        self.force = force
        self.clips: dict[str, dict] = {}
        self.errors: list[dict] = []
        self.synth_seconds = 0.0
        self.synthesized = 0
        self.reused = 0
        self.question_synth_seconds = 0.0
        self.question_clips = 0

    def ensure(self, text: str, *, kind: str, ref: str = "", section: str = "", published: bool = True) -> Path:
        from lexicon import clip_key, normalize_spoken

        text = normalize_spoken(text)
        if not text:
            raise RuntimeError("empty utterance")
        key = clip_key(VOICE_ID, text)
        rel = clip_path(key)
        dest = self.out_root / rel
        if dest.is_file() and dest.stat().st_size > 200 and not self.force:
            if published and key not in self.clips:
                self.clips[key] = self._record(key, rel, dest, kind, ref, section, reused=True)
            self.reused += 1
            return dest
        if key in self.clips and dest.is_file():
            return dest
        started = time.perf_counter()
        try:
            audio = self.engine.synthesize(text)
        except GpuStarved:
            raise
        except Exception as exc:
            self.errors.append({"key": key, "kind": kind, "ref": ref, "error": str(exc)})
            traceback.print_exc()
            raise
        elapsed = time.perf_counter() - started
        self.synth_seconds += elapsed
        self.synthesized += 1
        if kind in {"spoken_stem", "choices", "spoken_explanation"}:
            self.question_synth_seconds += elapsed
            self.question_clips += 1
        wav_path = dest.with_suffix(".wav")
        try:
            write_wav(wav_path, audio, self.engine.sample_rate)
            encode_speech(self.ffmpeg, wav_path, dest, self.bitrate)
        finally:
            wav_path.unlink(missing_ok=True)
        if published:
            self.clips[key] = self._record(key, rel, dest, kind, ref, section, reused=False)
        if self.synthesized % 10 == 0:
            print(f"  synthesized {self.synthesized} ({self.synth_seconds:.1f}s) {text[:70]}")
        return dest

    def _record(self, key, rel, dest, kind, ref, section, reused: bool) -> dict:
        seconds = mp3_duration(self.ffmpeg, dest)
        record = {
            "path": rel,
            "voice": VOICE_ID,
            "bytes": dest.stat().st_size,
            "duration_ms": int(round(seconds * 1000)),
            "content_type": "audio/mpeg",
        }
        if kind:
            record["kind"] = kind
        if ref:
            record["ref"] = ref
        if section:
            record["section"] = section
        record["_reused"] = reused
        return record


def write_manifest(path: Path, clips: dict) -> None:
    body = {
        "version": 1,
        "algorithm": "sha1-voice-newline-text-nfc-collapsed-ws",
        "voice_id": VOICE_ID,
        "clips": {
            key: {k: v for k, v in record.items() if not k.startswith("_")}
            for key, record in sorted(clips.items())
        },
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(body, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def prune(clips_dir: Path, keep: set[str]) -> int:
    removed = 0
    if not clips_dir.is_dir():
        return 0
    for file in clips_dir.rglob("*.mp3"):
        if file.stem not in keep:
            file.unlink()
            removed += 1
    for directory in sorted([p for p in clips_dir.rglob("*") if p.is_dir()], reverse=True):
        if not any(directory.iterdir()):
            directory.rmdir()
    return removed


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate Drive Study audio.")
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument(
        "--out",
        type=Path,
        default=None,
        help="Output root. Default: <root>/build/audio-publish. Contains audio/.",
    )
    parser.add_argument("--kokoro-voice", default=KOKORO_VOICE)
    parser.add_argument("--device", default="auto", choices=("auto", "cuda", "cpu"))
    parser.add_argument("--bitrate", default=BITRATE)
    parser.add_argument("--force", action="store_true")
    parser.add_argument("--skip-podcast", action="store_true")
    parser.add_argument("--max-questions", type=int, default=0)
    parser.add_argument("--budget-bytes", type=int, default=SIZE_BUDGET)
    args = parser.parse_args()

    root = args.root.resolve()
    out = (args.out or (root / "build" / "audio-publish")).resolve()
    if not (root / "content").is_dir():
        print(f"No content directory at {root / 'content'}", file=sys.stderr)
        return 2

    started = time.perf_counter()
    ffmpeg = find_ffmpeg()
    print(f"ffmpeg: {ffmpeg}")
    engine = load_engine(args.kokoro_voice, args.device)
    print(f"kokoro voice {args.kokoro_voice} on {engine.device}; manifest voice_id {VOICE_ID}")

    bank = load_bank(root)
    if args.max_questions:
        bank["questions"] = bank["questions"][: args.max_questions]
    utterances = load_utterances(root, bank)
    n_questions = len({u["ref"] for u in utterances if u["kind"] in {"spoken_stem", "choices", "spoken_explanation"}})
    print(
        f"content: {len(bank['questions'])} questions, {len(bank['flashcards'])} flashcards, "
        f"{len(bank['lessons'])} lessons, {len(utterances)} utterances"
    )
    if not utterances:
        print("Nothing to speak.", file=sys.stderr)
        return 2

    store = ClipStore(out, engine, ffmpeg, args.bitrate, args.force)

    def run_utterances() -> None:
        for item in utterances:
            try:
                store.ensure(
                    item["text"],
                    kind=item["kind"],
                    ref=str(item.get("ref") or ""),
                    section=str(item.get("section") or ""),
                    published=True,
                )
            except GpuStarved:
                raise

    try:
        run_utterances()
    except GpuStarved as exc:
        print(f"GPU memory got tight ({exc}). Continuing on CPU.", file=sys.stderr)
        engine = load_engine(args.kokoro_voice, "cpu")
        store.engine = engine
        run_utterances()

    def synth_published(text: str, published: bool = True) -> Path:
        kind = "intro" if not published else "podcast"
        return store.ensure(text, kind=kind, published=published)

    podcast_dir = out / "audio" / "podcast"
    dates_path = root / "tools" / "tts" / ".cache" / "episode-dates.json"
    # Dates committed on the audio branch survive a clean checkout.
    published_dates = out / "meta" / "episode-dates.json"
    if not args.skip_podcast:
        cache = root / "tools" / "tts" / ".cache"
        cache.mkdir(parents=True, exist_ok=True)
        builder = EpisodeBuilder(ffmpeg, synth_published, cache, podcast_dir, args.bitrate)
        by_lesson: dict[str, list] = {}
        for item in utterances:
            if item["kind"] != "narration":
                continue
            lesson_id = str(item["ref"]).split("#", 1)[0]
            by_lesson.setdefault(lesson_id, []).append(item)
        for lesson in bank["lessons"]:
            lid = str(lesson.get("id") or "")
            builder.add_lesson(lesson, by_lesson.get(lid, []))
        builder.add_quizzes(bank)
        dates = load_dates(dates_path) or load_dates(published_dates)
        dates = builder.write_catalog(podcast_dir / "catalog.json", dates)
        save_dates(dates_path, dates)
        save_dates(published_dates, dates)
        from podcast import write_cover

        write_cover(podcast_dir / "cover.jpg")
        print(f"episodes: {len(builder.episodes)}")

    removed = prune(out / "audio" / "clips", set(store.clips))
    if removed:
        print(f"pruned {removed} orphan clips")

    write_manifest(out / "audio" / "manifest.json", store.clips)
    per_100 = None
    if store.question_clips:
        # Three published fields per question: stem, choices, explanation.
        per_100 = round(store.question_synth_seconds / store.question_clips * 3 * 100, 1)
    audio_bytes = directory_size(out / "audio")
    elapsed = round(time.perf_counter() - started, 2)
    stats = {
        "elapsed_sec": elapsed,
        "device": engine.device,
        "kokoro_voice": args.kokoro_voice,
        "voice_id": VOICE_ID,
        "bitrate": args.bitrate,
        "questions": n_questions,
        "utterances": len(utterances),
        "clips": len(store.clips),
        "clips_synthesized": store.synthesized,
        "clips_reused": store.reused,
        "question_synth_seconds": round(store.question_synth_seconds, 2),
        "per_100_questions_sec": per_100,
        "audio_bytes": audio_bytes,
        "errors": store.errors,
    }
    stats_path = out / "meta" / "build-stats.json"
    stats_path.write_text(json.dumps(stats, indent=2) + "\n", encoding="utf-8")
    per_text = "n/a (cache hit)" if per_100 is None else f"{per_100}s"
    print(
        f"done in {elapsed}s. synthesized {store.synthesized}, reused {store.reused}. "
        f"per 100 questions: {per_text}. output {audio_bytes / 1_048_576:.1f} MiB."
    )
    print(f"manifest: {out / 'audio' / 'manifest.json'}")
    if store.errors:
        print(f"{len(store.errors)} clip errors", file=sys.stderr)
        return 1
    if audio_bytes > args.budget_bytes:
        print(
            f"Audio is {audio_bytes} bytes, over the {args.budget_bytes} byte budget. "
            "Refusing to treat this as publishable. Rebuild with --bitrate 32k.",
            file=sys.stderr,
        )
        return 3
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
