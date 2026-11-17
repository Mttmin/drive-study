# Audio manifest contract

Version 1. Last updated 2026-11-16.

This is the contract between the Drive Study app and the TTS pipeline that fills the `audio` branch. The app looks clips up by content hash. A missing key is not an error: playback falls through to the next source (cached clip, then streamed clip, then `speechSynthesis`).

The reference implementation is [`shared/audioKey.js`](../shared/audioKey.js). The Rust copy is `clip_key` in `core/src/audio.rs`. A cargo test checks it against fixtures taken from the live `audio` branch manifest. Match either one byte for byte. Do not re-implement the hash from memory.

## Where files live

On the published site (GitHub Pages, base path `/drive-study/`):

| URL path | Role |
| --- | --- |
| `audio/manifest.json` | This manifest |
| `audio/clips/<2 hex>/<sha1>.mp3` | One spoken clip |
| `audio/podcast/lesson-<lesson_id>.mp3` | Optional concatenated lesson episode |
| `audio/podcast/quiz-<section_id>.mp3` | Optional concatenated quiz episode |

Paths inside the manifest are **relative to the site root**, with no leading slash and no `/drive-study/` prefix. The app joins them onto its base URL.

Example: path `audio/clips/13/1335a8d460eaaaf2cc829487fb6ea1b421f7f7cb.mp3` is fetched as `https://mttmin.github.io/drive-study/audio/clips/13/1335a8d460eaaaf2cc829487fb6ea1b421f7f7cb.mp3`.

Clips must be **same-origin**. The service worker stores them with the Cache API. A cross-origin response would be opaque and iOS will not store it.

Git layout:

- `main` keeps a stub `audio/manifest.json` with `"clips": {}` so the app builds before any audio exists.
- The orphan branch **`audio`** is the source of truth once it contains `audio/manifest.json`. Deploy extracts that branch's `audio/` directory over the Pages artifact. Put clips and the full manifest on `audio`, not on `main`. Main stays small; Pages allows a 1 GB site, and the audio tree can be hundreds of MB.

Branch layout (root of the `audio` branch):

```
audio/manifest.json
audio/clips/ab/<40-hex>.mp3
audio/podcast/lesson-sie-1-01.mp3
```

## Manifest shape

```json
{
  "version": 1,
  "algorithm": "sha1-voice-newline-text-nfc-collapsed-ws",
  "voice_id": "natural-en-us-v1",
  "clips": {
    "1335a8d460eaaaf2cc829487fb6ea1b421f7f7cb": {
      "path": "audio/clips/13/1335a8d460eaaaf2cc829487fb6ea1b421f7f7cb.mp3",
      "voice": "natural-en-us-v1",
      "bytes": 18432,
      "duration_ms": 2100,
      "content_type": "audio/mpeg"
    }
  }
}
```

Required on each clip: `path`, `voice`.

Optional, used by the download screen when present: `bytes`, `duration_ms`, `content_type`. You may also add `kind`, `ref`, and `section` for debugging. The player does not need them. It only needs the hash key and `path`.

`clips` is an object keyed by the 40-character lowercase hex SHA-1. An empty object is valid.

File format: MP3 (`audio/mpeg`). Prefer mono, 64–96 kbps CBR, 24 kHz or 22.05 kHz, so a few thousand clips stay well under the 1 GB Pages limit. Encode at natural speed. The app sets playback rate in the player (0.8x–2x) for downloaded audio, streamed audio, and the phone voice. Do not bake speed into the file.

## Key algorithm

```
key = lowercase_hex( SHA1( utf8( voice_id + "\n" + normalize(text) ) ) )
```

`voice_id` for this pipeline is exactly:

```
natural-en-us-v1
```

`normalize(text)`:

1. Unicode NFC.
2. Replace each run of tab, line feed, vertical tab, form feed, carriage return, ASCII space, or NBSP (`U+00A0`) with a single ASCII space.
3. Trim leading and trailing spaces.

No other characters are folded. Case, punctuation, and digits stay as authored. The newline between voice id and text is a single `\n` (U+000A). There is no trailing newline.

Speed, kind, and content id are **not** part of the key. The same voice speaking the same normalized text is one clip, even if two questions share a sentence.

### Test vector

| Field | Value |
| --- | --- |
| voice id | `natural-en-us-v1` |
| text | `Hello, Drive Study.` |
| also matches | `Hello,   Drive Study.\n` (whitespace collapses) |
| SHA-1 | `1335a8d460eaaaf2cc829487fb6ea1b421f7f7cb` |

Check: `node --input-type=module -e "import { clipKey } from './shared/audioKey.js'; console.log(clipKey('natural-en-us-v1', 'Hello, Drive Study.'))"`.

Suggested path for that clip: `audio/clips/13/1335a8d460eaaaf2cc829487fb6ea1b421f7f7cb.mp3` (`clipPath()` in `shared/audioKey.js`).

## Which texts get a clip

Generate one clip for every utterance below, using the text **exactly** as the app speaks it. List them with:

```
node scripts/list-utterances.mjs
```

That script prints a JSON array of `{ kind, ref, section, exam, text, key, path }` once content is in the repo. Hash `text` with `clipKey(voice_id, text)`.

Kinds:

| kind | Source text |
| --- | --- |
| `spoken_stem` | Question `spoken_stem` |
| `choices` | Derived readout, see below. Not a field in the question JSON. |
| `spoken_explanation` | Question `spoken_explanation` |
| `flashcard` | Flashcard `spoken` |
| `narration` | Each lesson segment with `type: "narration"`, in order. Ref is `<lesson_id>#<index>` where index is the segment's index in `segments` (check segments count, they just have no clip). |

Do **not** synthesize separate clips for the on-screen `stem`, the written `explanation`, lesson titles, summaries, or key terms. The player shows those and speaks the spoken fields. Spoken explanations should start with the answer ("The answer is C.") because the player does not speak a separate answer line.

### Choices readout

Implemented by `choicesReadout()`:

- `type: "tf"`: the exact string `True, or false.`
- `type: "mc"`: for each choice, in order, `"{key}. {text}."` joined by a single space. The choice text is normalized first. A trailing `.`, `!`, or `?` is kept; otherwise a period is added.

Example. Choices `A` = `Common stock`, `B` = `Preferred stock.` produce:

```
A. Common stock. B. Preferred stock.
```

True/false example:

```
True, or false.
```

## Podcast episodes (optional)

The Pages site serves `podcast/feed.xml`, built at deploy time by `scripts/build-podcast-feed.mjs`. An episode is included only when the mp3 exists on the `audio` branch:

| File | Episode |
| --- | --- |
| `audio/podcast/lesson-<lesson_id>.mp3` | One lesson. Concatenate that lesson's narration clips in order. A short silence between segments is welcome. |
| `audio/podcast/quiz-<section_id>.mp3` | One section quiz. For each question in section order: spoken stem, choices readout, about 6 seconds of silence, spoken explanation. |

Same MP3 recommendations as clips. The feed's enclosure URL is same-origin under `/drive-study/audio/podcast/...`.

## Fallback the app already implements

You do not need a clip for every line on day one. For each utterance the player:

1. Computes the key.
2. If the active source is **downloaded**: plays the Cache API copy, else fetches the manifest path while online, else phone voice.
3. If the active source is **streamed**: fetches the path while online, else a cached copy, else phone voice.
4. If the active source is **phone**, or the key is absent, or the file 404s or fails to decode: `speechSynthesis`.

A manifest that lists a key whose file is missing should be avoided. Omit the key instead.
