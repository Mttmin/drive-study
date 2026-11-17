"""Kokoro-82M speech, with a CPU fallback when the GPU window is closed.

Jarvis keeps a llama-server on this machine. This module only reads
nvidia-smi. It never signals, restarts, or pins that process.
"""

from __future__ import annotations

import os
import subprocess
from datetime import datetime
from zoneinfo import ZoneInfo

import numpy as np

# Modest allocator behavior so a long batch does not pin a huge cache.
os.environ.setdefault("PYTORCH_CUDA_ALLOC_CONF", "max_split_size_mb:128")

REPO_ID = "hexgrad/Kokoro-82M"
SAMPLE_RATE = 24000
# Leave headroom for llama-server. Kokoro itself is a few hundred MB.
MIN_FREE_MIB = 3072
ET = ZoneInfo("America/New_York")


def gpu_quiet_hours(now: datetime | None = None) -> bool:
    """True from 00:55 through 07:00 America/New_York.

    The autobed nightly vision job uses the GPU in that window.
    """
    now = now.astimezone(ET) if now else datetime.now(ET)
    minutes = now.hour * 60 + now.minute
    return 55 <= minutes < 7 * 60


def gpu_free_mib() -> int | None:
    try:
        out = subprocess.check_output(
            ["nvidia-smi", "--query-gpu=memory.free", "--format=csv,noheader,nounits"],
            text=True,
            timeout=15,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    line = out.strip().splitlines()[0].strip()
    try:
        return int(line)
    except ValueError:
        return None


def choose_device(requested: str = "auto") -> str:
    requested = (requested or "auto").lower()
    if requested == "cpu":
        return "cpu"
    if gpu_quiet_hours():
        print("GPU quiet hours (00:55-07:00 ET). Using CPU.")
        return "cpu"
    free = gpu_free_mib()
    if free is None:
        print("nvidia-smi did not report free memory. Using CPU.")
        return "cpu"
    if free < MIN_FREE_MIB:
        print(f"Only {free} MiB free on the GPU. Using CPU so llama-server keeps its memory.")
        return "cpu"
    if requested in {"auto", "cuda"}:
        print(f"Using CUDA. {free} MiB free.")
        return "cuda"
    raise SystemExit(f"Unknown device {requested!r}. Use auto, cuda, or cpu.")


class KokoroEngine:
    model_name = "kokoro-82m"
    sample_rate = SAMPLE_RATE

    def __init__(self, voice: str, device: str):
        import torch
        from kokoro import KPipeline

        torch.set_num_threads(4)
        self.voice = voice
        self.device = device
        self._torch = torch
        self.pipeline = KPipeline(
            lang_code="a",
            repo_id=REPO_ID,
            device=device,
            trf=False,
        )

    def synthesize(self, text: str, voice: str | None = None) -> np.ndarray:
        gap = np.zeros(int(0.06 * SAMPLE_RATE), dtype=np.float32)
        parts: list[np.ndarray] = []
        try:
            results = self.pipeline(text, voice=voice or self.voice, speed=1.0)
        except RuntimeError as exc:
            if self.device == "cuda" and "out of memory" in str(exc).lower():
                raise GpuStarved(str(exc)) from exc
            raise
        for result in results:
            audio = result.audio
            if audio is None:
                continue
            if hasattr(audio, "detach"):
                audio = audio.detach().float().cpu().numpy()
            audio = np.asarray(audio, dtype=np.float32).reshape(-1)
            if audio.size == 0:
                continue
            if parts:
                parts.append(gap)
            parts.append(audio)
        if not parts:
            raise RuntimeError("Kokoro returned no audio")
        merged = np.concatenate(parts)
        peak = float(np.max(np.abs(merged))) if merged.size else 0.0
        if peak > 0.99:
            merged = merged * (0.95 / peak)
        return merged


class GpuStarved(RuntimeError):
    """CUDA ran out of memory. Caller should switch this process to CPU."""


def load_engine(voice: str, requested_device: str = "auto") -> KokoroEngine:
    device = choose_device(requested_device)
    try:
        return KokoroEngine(voice, device)
    except Exception as exc:
        if device != "cuda":
            raise
        print(f"CUDA init failed ({exc}). Falling back to CPU.")
        return KokoroEngine(voice, "cpu")
