"""Trim, loudness-normalize, and encode speech to mono MP3."""

from __future__ import annotations

import re
import shutil
import subprocess
import wave
from pathlib import Path

BITRATE = "40k"
SAMPLE_RATE = 24000


def find_ffmpeg() -> str:
    env = os_which()
    if env:
        return env
    here = Path(__file__).resolve().parent / "bin" / "ffmpeg"
    if here.is_file() and os_access(here):
        return str(here)
    found = shutil.which("ffmpeg")
    if found:
        return found
    raise SystemExit(
        "ffmpeg is required (libmp3lame, loudnorm, silenceremove). "
        "Install it, or set FFMPEG=/path/to/ffmpeg."
    )


def os_which() -> str | None:
    import os

    value = os.environ.get("FFMPEG")
    if value and Path(value).is_file():
        return value
    return None


def os_access(path: Path) -> bool:
    return path.is_file() and os_executable(path)


def os_executable(path: Path) -> bool:
    import os

    return os.access(path, os.X_OK)


def write_wav(path: Path, audio, sample_rate: int = SAMPLE_RATE) -> None:
    import numpy as np

    path.parent.mkdir(parents=True, exist_ok=True)
    samples = np.asarray(audio, dtype=np.float32).reshape(-1)
    samples = np.clip(samples, -1.0, 1.0)
    pcm = (samples * 32767.0).astype(np.int16)
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(sample_rate)
        handle.writeframes(pcm.tobytes())


def wav_duration(path: Path) -> float:
    with wave.open(str(path), "rb") as handle:
        rate = handle.getframerate() or 1
        return handle.getnframes() / rate


def encode_speech(ffmpeg: str, wav_path: Path, mp3_path: Path, bitrate: str = BITRATE) -> None:
    """Trim end silence, leave a short pad, normalize to -16 LUFS, write mono MP3."""
    mp3_path.parent.mkdir(parents=True, exist_ok=True)
    duration = wav_duration(wav_path)
    # loudnorm is unhappy on clips shorter than about half a second.
    pad_to = max(0.0, 0.55 - duration)
    filters = [
        "silenceremove=start_periods=1:start_duration=0.03:start_threshold=-42dB:detection=peak",
        "areverse",
        "silenceremove=start_periods=1:start_duration=0.05:start_threshold=-42dB:detection=peak",
        "areverse",
        "adelay=40:all=1",
        f"apad=pad_dur={0.08 + pad_to:.3f}",
        "loudnorm=I=-16:TP=-1.5:LRA=11",
    ]
    cmd = [
        ffmpeg,
        "-y",
        "-hide_banner",
        "-loglevel",
        "error",
        "-i",
        str(wav_path),
        "-af",
        ",".join(filters),
        "-ac",
        "1",
        "-ar",
        str(SAMPLE_RATE),
        "-c:a",
        "libmp3lame",
        "-b:a",
        bitrate,
        str(mp3_path),
    ]
    subprocess.run(cmd, check=True)


def make_silence(ffmpeg: str, dest: Path, seconds: float = 6.0, bitrate: str = BITRATE) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    if dest.is_file() and dest.stat().st_size > 200:
        return
    cmd = [
        ffmpeg,
        "-y",
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        f"anullsrc=r={SAMPLE_RATE}:cl=mono",
        "-t",
        f"{seconds:.3f}",
        "-ac",
        "1",
        "-ar",
        str(SAMPLE_RATE),
        "-c:a",
        "libmp3lame",
        "-b:a",
        bitrate,
        str(dest),
    ]
    subprocess.run(cmd, check=True)


def concat_mp3(
    ffmpeg: str,
    parts: list[Path],
    dest: Path,
    *,
    title: str,
    artist: str = "Drive Study",
    album: str = "Drive Study",
) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    list_path = dest.with_suffix(".concat.txt")
    lines = []
    for part in parts:
        # concat demuxer wants quoted paths; our names are hex and safe.
        lines.append(f"file '{part.resolve()}'")
    list_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    cmd = [
        ffmpeg,
        "-y",
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "concat",
        "-safe",
        "0",
        "-i",
        str(list_path),
        "-c",
        "copy",
        "-metadata",
        f"title={title}",
        "-metadata",
        f"artist={artist}",
        "-metadata",
        f"album={album}",
        str(dest),
    ]
    try:
        subprocess.run(cmd, check=True)
    finally:
        list_path.unlink(missing_ok=True)


_DURATION = re.compile(r"Duration:\s*(\d+):(\d+):(\d+(?:\.\d+)?)")


def mp3_duration(ffmpeg: str, path: Path) -> float:
    proc = subprocess.run(
        [ffmpeg, "-hide_banner", "-i", str(path), "-f", "null", "-"],
        capture_output=True,
        text=True,
    )
    match = _DURATION.search(proc.stderr or "")
    if not match:
        return 0.0
    hours, minutes, seconds = match.groups()
    return int(hours) * 3600 + int(minutes) * 60 + float(seconds)


def directory_size(path: Path) -> int:
    total = 0
    if not path.exists():
        return 0
    for file in path.rglob("*"):
        if file.is_file():
            total += file.stat().st_size
    return total
