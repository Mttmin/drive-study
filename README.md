# Drive Study

Audio-first study app for the FINRA SIE and Series 7. Built as an installable PWA for an iPhone in the car. Hosted on GitHub Pages.

Live site: <https://mttmin.github.io/drive-study/>

This is original sample material so every mode runs. It is not a FINRA exam, not affiliated with FINRA, and not a substitute for a full question bank. Another author can replace the JSON under `content/` without changing the app.

## Use it

Plain-language steps for an iPhone are in [docs/USER_GUIDE.md](docs/USER_GUIDE.md).

## Develop

```bash
npm install
npm run dev
```

Open `http://127.0.0.1:5173/drive-study/`.

```bash
npm run validate   # content schema and references
npm test           # spaced repetition, scoring, blueprint, answer grammar
npm run build
npm run preview
npm run test:e2e   # Playwright, mobile WebKit
```

Stack: Vite, TypeScript, Preact. No UI framework beyond that. Progress, mistakes, and settings are in IndexedDB. A service worker precaches the shell and every content JSON file.

## Content

```
content/meta.json
content/{sie,s7}/lessons/<lesson_id>.json
content/{sie,s7}/questions/<section>.json
content/{sie,s7}/flashcards/<section>.json
```

`content/meta.json` lists the files to load (GitHub Pages does not serve directory indexes) and the exam blueprints. Section weights match FINRA's public outlines:

- SIE: 75 scored questions, 105 minutes, 70% to pass. Sections 16% / 44% / 31% / 9% (12 / 33 / 23 / 7).
- Series 7: 125 scored questions, 225 minutes, 72% to pass. Functions 7% / 9% / 73% / 11% (9 / 11 / 91 / 14).

Sample questions are marked `"sample": true`. The app shows a SAMPLE badge. Real items can omit the flag.

## Audio and GitHub Pages

Natural-voice MP3s are not committed to `main`. They can be several hundred megabytes. Pages has a 1 GB site limit, and the clips have to be same-origin so iOS can store them in the Cache API.

The orphan branch `audio` holds:

```
audio/manifest.json
audio/clips/<aa>/<sha1>.mp3
audio/podcast/*.mp3
```

The deploy workflow checks out `main`, builds the site, then `git archive`s `audio/` from `origin/audio` into `dist/` when that branch has a manifest. Until then, the stub in this branch is published and the app uses the phone voice.

The hash contract is [docs/audio-manifest.md](docs/audio-manifest.md). Reference code: [`shared/audioKey.js`](shared/audioKey.js). Generate the utterance list the pipeline should speak:

```bash
node scripts/list-utterances.mjs
```

Playback speed (0.8x–2x) is applied in the player for downloaded audio, streamed audio, and `speechSynthesis`. It is not part of the hash.

## Progress file

Settings → Export writes a JSON file (version 1) with SRS cards, attempts, day streaks, and settings. Import replaces progress on this device.
