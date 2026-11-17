/**
 * Canonical audio-clip key for Drive Study.
 * Contract: docs/audio-manifest.md
 *
 * This file is the reference implementation. The site, the content
 * validator, and the TTS pipeline must all use it (or match it byte for byte).
 * Playback speed is NOT part of the key.
 */

export const VOICE_ID = "natural-en-us-v1";

/** NFC, then collapse ASCII whitespace and NBSP to a single space, then trim. */
export function normalizeSpokenText(text) {
  const nfc = String(text ?? "").normalize("NFC");
  return nfc.replace(/[\t\n\r\f\v \u00A0]+/g, " ").trim();
}

export function clipKey(voiceId, text) {
  const payload = `${voiceId}\n${normalizeSpokenText(text)}`;
  return sha1Hex(payload);
}

/**
 * Spoken choices line. True/false is a fixed phrase.
 * Multiple choice is "A. {text}. B. {text}." with a period added when missing.
 * The result is hashed after normalizeSpokenText, which is idempotent here.
 */
export function choicesReadout(question) {
  if (question.type === "tf") return "True, or false.";
  const choices = Array.isArray(question.choices) ? question.choices : [];
  return choices
    .map((choice) => {
      let text = normalizeSpokenText(choice.text);
      if (text && !/[.!?]$/.test(text)) text += ".";
      return `${choice.key}. ${text}`;
    })
    .join(" ");
}

export function questionUtterances(question) {
  return [
    {
      kind: "spoken_stem",
      ref: question.id,
      section: question.section,
      exam: question.exam,
      text: question.spoken_stem || question.stem,
    },
    {
      kind: "choices",
      ref: question.id,
      section: question.section,
      exam: question.exam,
      text: choicesReadout(question),
    },
    {
      kind: "spoken_explanation",
      ref: question.id,
      section: question.section,
      exam: question.exam,
      text: question.spoken_explanation || question.explanation,
    },
  ];
}

export function flashcardUtterance(card) {
  return {
    kind: "flashcard",
    ref: card.id,
    section: card.section,
    exam: card.exam,
    text: card.spoken,
  };
}

export function lessonUtterances(lesson) {
  const out = [];
  const segments = Array.isArray(lesson.segments) ? lesson.segments : [];
  segments.forEach((segment, index) => {
    if (segment.type !== "narration") return;
    out.push({
      kind: "narration",
      ref: `${lesson.id}#${index}`,
      section: lesson.section,
      exam: lesson.exam,
      text: segment.text,
    });
  });
  return out;
}

/** Suggested relative path for a clip. The pipeline may use this exact layout. */
export function clipPath(key) {
  return `audio/clips/${key.slice(0, 2)}/${key}.mp3`;
}

function sha1Hex(message) {
  const msg = new TextEncoder().encode(message);
  const bitLen = msg.length * 8;
  const paddedLen = Math.ceil((msg.length + 9) / 64) * 64;
  const buf = new Uint8Array(paddedLen);
  buf.set(msg);
  buf[msg.length] = 0x80;
  const view = new DataView(buf.buffer);
  const hi = Math.floor(bitLen / 0x100000000);
  view.setUint32(paddedLen - 8, hi, false);
  view.setUint32(paddedLen - 4, bitLen >>> 0, false);

  let h0 = 0x67452301;
  let h1 = 0xefcdab89;
  let h2 = 0x98badcfe;
  let h3 = 0x10325476;
  let h4 = 0xc3d2e1f0;
  const w = new Uint32Array(80);

  for (let offset = 0; offset < paddedLen; offset += 64) {
    for (let t = 0; t < 16; t++) w[t] = view.getUint32(offset + t * 4, false);
    for (let t = 16; t < 80; t++) {
      const x = w[t - 3] ^ w[t - 8] ^ w[t - 14] ^ w[t - 16];
      w[t] = ((x << 1) | (x >>> 31)) >>> 0;
    }
    let a = h0;
    let b = h1;
    let c = h2;
    let d = h3;
    let e = h4;
    for (let t = 0; t < 80; t++) {
      let f;
      let k;
      if (t < 20) {
        f = (b & c) | (~b & d);
        k = 0x5a827999;
      } else if (t < 40) {
        f = b ^ c ^ d;
        k = 0x6ed9eba1;
      } else if (t < 60) {
        f = (b & c) | (b & d) | (c & d);
        k = 0x8f1bbcdc;
      } else {
        f = b ^ c ^ d;
        k = 0xca62c1d6;
      }
      const temp = (rotl(a, 5) + f + e + k + w[t]) >>> 0;
      e = d;
      d = c;
      c = rotl(b, 30);
      b = a;
      a = temp;
    }
    h0 = (h0 + a) >>> 0;
    h1 = (h1 + b) >>> 0;
    h2 = (h2 + c) >>> 0;
    h3 = (h3 + d) >>> 0;
    h4 = (h4 + e) >>> 0;
  }

  return [h0, h1, h2, h3, h4].map((value) => value.toString(16).padStart(8, "0")).join("");
}

function rotl(value, shift) {
  return ((value << shift) | (value >>> (32 - shift))) >>> 0;
}
