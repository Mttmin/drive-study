# Study audio

Kokoro-82M speaks the bank. The manifest voice id is `natural-en-us-v1`, which is what [`shared/audioKey.js`](../../shared/audioKey.js) hashes. The voice that produces that audio is Kokoro `af_heart` (American English, the model's strongest voice). `am_michael` is the other pleasant US voice; pass `--kokoro-voice am_michael` to rebuild the bank with it. The voice id stays `natural-en-us-v1`, so the app does not change.

Clips are mono 24 kHz MP3 at 40 kbps CBR, normalized to about -16 LUFS, with edge silence trimmed. The app contract would also accept 64–96 kbps. 40 kbps is inside the requested 32–48 kbps range and leaves room for quiz episodes under the 1 GB Pages limit. If a full bank ever exceeds 900 MB, the build exits and `make audio ARGS="--bitrate 32k"` is the smaller setting.

A clip is rebuilt only when its file is missing. The file name is the SHA-1 from `clipKey()`.

## One-command rebuild

From the repo root, after content JSON changes:

```bash
make audio
```

That reads `content/`, writes `build/audio-publish/` (gitignored), and pushes the `audio/` tree to the orphan branch `audio`. It does not replace the stub `audio/manifest.json` on `main`. Set `AUDIO_PUBLISH=0` to skip the push.

First time on a new machine:

```bash
make audio-setup
sudo apt-get install -y ffmpeg espeak-ng
make audio
```

`ffmpeg` needs libmp3lame. If it is not on `PATH`, put a binary at `tools/tts/bin/ffmpeg` or set `FFMPEG`.

GPU is used when `nvidia-smi` reports at least 3 GiB free and the local time is outside 00:55–07:00 America/New_York. Inside that window, or when the GPU is tight, the same command runs on CPU. It only reads `nvidia-smi`.

```bash
make audio ARGS="--device cpu"
AUDIO_PUBLISH=0 make audio ARGS="--force"
```

## What gets spoken

The utterance list is `node scripts/list-utterances.mjs` when that script exists, otherwise the same rules as `shared/audioKey.js`:

- Question `spoken_stem` (or `stem` if the spoken field is empty)
- Choices readout: `A. {text}. B. {text}.`, or the exact string `True, or false.`
- Question `spoken_explanation` (this is the answer line; there is no separate answer clip)
- Flashcard `spoken`
- Each lesson narration segment

Whitespace is collapsed the way `normalizeSpokenText` does. Nothing else in the sentence is rewritten, or the app's hash would miss the file.

Podcast episodes are built beside the clips:

- `audio/podcast/lesson-<lesson_id>.mp3` — narration, in order, with a short pause and a spoken intro
- `audio/podcast/quiz-<section_id>.mp3` — each question, then six seconds, then the explanation. Sections longer than 20 questions are split; part 1 keeps the name above, and later parts are `quiz-<section>-part-02.mp3`

`scripts/build-podcast-feed.mjs` turns `audio/podcast/catalog.json` into `podcast/feed.xml` during the Pages deploy. Apple Podcasts subscribes at `podcast://mttmin.github.io/drive-study/podcast/feed.xml`.

## Layout

| Path | Branch |
| --- | --- |
| `tools/tts/`, `scripts/build-podcast-feed.mjs` | `main` |
| `audio/manifest.json` stub | `main` |
| `audio/clips/**/*.mp3`, `audio/podcast/*.mp3`, full `audio/manifest.json` | orphan branch `audio` |

`tools/tts/.venv/`, `tools/tts/.cache/`, `build/`, and `.audio-worktree/` stay untracked.
