#!/usr/bin/env bash
# Generate clips and the podcast, then publish the audio branch when this is a git checkout.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

PY="${TTS_PYTHON:-$ROOT/tools/tts/.venv/bin/python}"
if [[ ! -x "$PY" ]]; then
  echo "No TTS virtualenv at tools/tts/.venv." >&2
  echo "Run: make audio-setup" >&2
  exit 1
fi
# misaki installs a small spaCy model with uv on first use, and uv only
# installs into VIRTUAL_ENV. Invoking the venv python directly does not set it.
if [[ -z "${VIRTUAL_ENV:-}" ]]; then
  export VIRTUAL_ENV="$(cd "$(dirname "$PY")/.." && pwd)"
fi

if [[ -z "${FFMPEG:-}" && -x "$ROOT/tools/tts/bin/ffmpeg" ]]; then
  export FFMPEG="$ROOT/tools/tts/bin/ffmpeg"
fi
if [[ -d "$ROOT/tools/tts/espeak-prefix/usr/bin" ]]; then
  export PATH="$ROOT/tools/tts/espeak-prefix/usr/bin:$PATH"
fi

mkdir -p "$ROOT/tools/tts/.cache"
WT="$ROOT/.audio-worktree"
if [[ -f "$WT/meta/episode-dates.json" ]]; then
  cp "$WT/meta/episode-dates.json" "$ROOT/tools/tts/.cache/episode-dates.json"
fi

"$PY" "$ROOT/tools/tts/generate.py" --root "$ROOT" --out "$ROOT/build/audio-publish" "$@"

if [[ "${AUDIO_PUBLISH:-1}" == "1" ]] && git -C "$ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  bash "$ROOT/tools/tts/publish.sh"
fi
