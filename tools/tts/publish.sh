#!/usr/bin/env bash
# Push generated audio to the orphan branch `audio` and ask Pages to redeploy.
# Clips and episodes stay off main. Pull --rebase before the push.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
WT="$ROOT/.audio-worktree"

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "publish.sh: not a git checkout; audio files were left in ./audio and ./podcast." >&2
  exit 0
fi

git fetch origin main
if git ls-remote --exit-code --heads origin audio >/dev/null 2>&1; then
  git fetch origin audio
  if [[ ! -e "$WT/.git" ]]; then
    git worktree add -B audio "$WT" origin/audio
  fi
  git -C "$WT" pull --rebase origin audio
else
  if [[ ! -e "$WT/.git" ]]; then
    rm -rf "$WT"
    git worktree add --detach "$WT" HEAD
  fi
  git -C "$WT" checkout --orphan audio
  git -C "$WT" rm -rf . >/dev/null 2>&1 || true
fi

SRC="$ROOT/build/audio-publish"
if [[ ! -f "$SRC/audio/manifest.json" ]]; then
  echo "publish.sh: no build at $SRC. Run make audio first." >&2
  exit 1
fi
mkdir -p "$WT/audio" "$WT/meta"
rsync -a --delete "$SRC/audio/" "$WT/audio/"
if [[ -d "$SRC/meta" ]]; then
  rsync -a "$SRC/meta/" "$WT/meta/"
fi
# Keep a copy of the workflow on this branch so a push here can redeploy.
if [[ -f "$ROOT/.github/workflows/pages.yml" ]]; then
  mkdir -p "$WT/.github/workflows"
  cp "$ROOT/.github/workflows/pages.yml" "$WT/.github/workflows/pages.yml"
fi

git -C "$WT" add -A
if git -C "$WT" diff --cached --quiet; then
  echo "audio branch already matches this build."
else
  git -C "$WT" commit -m "$(cat <<'EOF'
Update generated study audio and podcast feed.

EOF
)"
fi

git -C "$WT" pull --rebase origin audio || true
git -C "$WT" push -u origin audio

if command -v gh >/dev/null 2>&1; then
  gh workflow run pages.yml --ref main >/dev/null 2>&1 || true
fi

echo "Pushed the audio branch."
