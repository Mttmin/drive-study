# Audio clips

Natural-voice MP3s do not live on `main` (that branch stays small). They live on the orphan branch `audio`, which GitHub Actions copies into the Pages artifact at deploy time.

Layout on that branch, and on the published site:

```
audio/manifest.json
audio/clips/ab/<sha1>.mp3
audio/podcast/lesson-<lesson_id>.mp3
audio/podcast/quiz-<section_id>.mp3
```

The manifest format, the hash input, and the utterance list are specified in [docs/audio-manifest.md](../docs/audio-manifest.md). The reference key function is [shared/audioKey.js](../shared/audioKey.js).

`audio/manifest.json` on `main` is an empty stub so the app runs before any clips exist. Once the `audio` branch contains `audio/manifest.json`, the deploy uses that file and those clips instead of this stub.
