# Drive Study

Audio-first study app for the FINRA SIE and Series 7. Built as an installable PWA for an iPhone in the car. Hosted on GitHub Pages.

Live site: <https://mttmin.github.io/drive-study/>

This is original sample material so every mode runs. It is not a FINRA exam, not affiliated with FINRA, and not a substitute for a full question bank. Another author can replace the JSON under `content/` without changing the app.

## Use it

Plain-language steps for an iPhone are in [docs/USER_GUIDE.md](docs/USER_GUIDE.md).

## Develop

The interface is [Leptos](https://leptos.dev/) (client-side) compiled to WebAssembly with [Trunk](https://trunkrs.dev/). Study rules live in the `core` crate and are tested with `cargo test`. The service worker is plain JavaScript. Speech recognition, the media session, and the wake lock are reached from Rust through wasm-bindgen.

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk --locked --version 0.21.14
cargo test -p drive-study-core
cargo run -p drive-study-core --bin validate-content
cd app && env -u NO_COLOR trunk serve
```

Open `http://127.0.0.1:8080/drive-study/`.

```bash
cd app && env -u NO_COLOR trunk build --release
cargo run -p drive-study-core --bin generate-sw -- dist
npm ci
npx playwright test
```

`node scripts/list-utterances.mjs` is what `make audio` reads. Leave that script and `shared/audioKey.js` in place. The Rust key function is checked against clips from the live `audio` branch.

## Content

```
content/meta.json
content/{sie,s7}/lessons/<lesson_id>.json
content/{sie,s7}/questions/<section>.json
content/{sie,s7}/flashcards/<section>.json
```

`content/meta.json` is the blueprint. The build writes `content/index.json` by scanning the folders, because Pages cannot list directories. Replacing `content/` and rebuilding picks up the next wave. `node scripts/list-utterances.mjs` scans the same folders for `make audio`.

- SIE: 75 scored questions plus 5 pretest, 105 minutes, passing score 70. Sections 16% / 44% / 31% / 9%.
- Series 7: 125 scored questions plus 5 pretest, 225 minutes, passing score 72. Functions 7% / 9% / 73% / 11%. Margin is in S7-4.

True/false items use choices A = True and B = False. Wave 1 is 104 SIE questions, 60 SIE flashcards, 3 SIE lessons, 111 Series 7 questions, 70 Series 7 flashcards, and 3 Series 7 lessons. Later waves keep those ids and append new ones.

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

Rebuild the clips and podcast episodes with `make audio` (see `tools/tts/README.md`). The Pages workflow writes the Apple Podcasts feed at `podcast/feed.xml`. Subscribe with `podcast://mttmin.github.io/drive-study/podcast/feed.xml`.

Playback speed (0.8x–2x) is applied in the player for downloaded audio, streamed audio, and `speechSynthesis`. It is not part of the hash.

## Progress file

Settings → Export writes a JSON file (version 1) with SRS cards, attempts, day streaks, and settings. Import replaces progress on this device.
