import { clipKey, VOICE_ID } from "../../shared/audioKey.js";
import type { AudioManifest, ClipEntry, VoiceSource } from "./types";

const AUDIO_CACHE = "drive-study-audio-v1";

const SILENCE =
  "data:audio/wav;base64,UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQAAAAA=";

type PlayResult = "done" | "stopped";

export class Speaker {
  private audio: HTMLAudioElement | null = typeof Audio === "undefined" ? null : new Audio();
  private keeper: HTMLAudioElement | null = null;
  private token = 0;
  private hold = false;
  private resolvers = new Set<() => void>();
  rate = 1;
  sourceLabel = "Phone voice";
  onSource?: (label: string) => void;

  constructor(
    private readonly manifest: () => AudioManifest,
    private readonly mode: () => VoiceSource,
    private readonly voiceURI: () => string | null,
  ) {
    if (this.audio) this.audio.preload = "auto";
  }

  /** Call from the tap that starts a session so iOS will allow audio. */
  arm(): void {
    if (!this.keeper && typeof Audio !== "undefined") {
      this.keeper = new Audio(SILENCE);
      this.keeper.loop = true;
      this.keeper.volume = 0.05;
    }
    void this.keeper?.play().catch(() => undefined);
    if (typeof speechSynthesis !== "undefined") {
      const unlock = new SpeechSynthesisUtterance(" ");
      unlock.volume = 0;
      speechSynthesis.speak(unlock);
    }
  }

  stopKeeper(): void {
    this.keeper?.pause();
  }

  setRate(rate: number): void {
    this.rate = rate;
    if (this.audio && this.audio.src) this.audio.playbackRate = rate;
  }

  async speak(text: string): Promise<PlayResult> {
    const my = ++this.token;
    const clip = this.manifest().clips[clipKey(VOICE_ID, text)];
    const mode = this.mode();
    if (clip && mode !== "phone") {
      try {
        const resolved = await this.resolveClip(clip, mode);
        if (this.token !== my) return "stopped";
        if (resolved) {
          try {
            const result = await this.playUrl(resolved.url, my, resolved.label);
            if (resolved.revoke) URL.revokeObjectURL(resolved.url);
            if (result === "done" || this.token !== my) return this.token === my ? "done" : "stopped";
          } catch {
            if (resolved.revoke) URL.revokeObjectURL(resolved.url);
          }
        }
      } catch {
        /* fall through to the phone voice */
      }
    }
    if (this.token !== my) return "stopped";
    return this.speakPhone(text, my);
  }

  stop(): void {
    this.token += 1;
    if (this.audio) {
      this.audio.pause();
    }
    if (typeof speechSynthesis !== "undefined") speechSynthesis.cancel();
    for (const resolve of this.resolvers) resolve();
    this.resolvers.clear();
  }

  pause(): void {
    this.hold = true;
    this.audio?.pause();
    if (typeof speechSynthesis !== "undefined" && speechSynthesis.speaking) {
      try {
        speechSynthesis.pause();
      } catch {
        /* iOS may not pause speech synthesis */
      }
    }
  }

  resume(): void {
    this.hold = false;
    if (this.audio?.src && this.audio.paused && !this.audio.ended) {
      void this.audio.play().catch(() => undefined);
    }
    if (typeof speechSynthesis !== "undefined" && speechSynthesis.paused) {
      try {
        speechSynthesis.resume();
      } catch {
        /* ignore */
      }
    }
  }

  private async resolveClip(
    clip: ClipEntry,
    mode: VoiceSource,
  ): Promise<{ url: string; revoke: boolean; label: string } | null> {
    const absolute = new URL(clip.path, window.location.origin + import.meta.env.BASE_URL).toString();
    const cache = typeof caches === "undefined" ? null : await caches.open(AUDIO_CACHE);
    const hit = cache ? await cache.match(absolute) : undefined;
    if (mode === "downloaded" && hit) return blobUrl(hit, "Downloaded clip");
    if (navigator.onLine) return { url: absolute, revoke: false, label: "Streamed clip" };
    if (hit) return blobUrl(hit, "Downloaded clip");
    return null;
  }

  private playUrl(url: string, token: number, label: string): Promise<PlayResult> {
    this.setSource(label);
    const audio = this.audio;
    if (!audio) return Promise.resolve("done");
    return new Promise((resolve, reject) => {
      let settled = false;
      const finish = (result: PlayResult) => {
        if (settled) return;
        settled = true;
        this.resolvers.delete(release);
        resolve(result);
      };
      const release = () => finish("stopped");
      this.resolvers.add(release);
      audio.onended = () => finish(this.token === token ? "done" : "stopped");
      audio.onerror = () => {
        if (!settled) {
          settled = true;
          this.resolvers.delete(release);
          reject(new Error("audio"));
        }
      };
      audio.src = url;
      audio.playbackRate = this.rate;
      void audio.play().catch((error) => {
        if (!settled) {
          settled = true;
          this.resolvers.delete(release);
          reject(error);
        }
      });
    });
  }

  private async speakPhone(text: string, token: number): Promise<PlayResult> {
    this.setSource("Phone voice");
    if (typeof speechSynthesis === "undefined") {
      await delay(Math.min(1200, 400 + text.length * 12));
      return this.token === token ? "done" : "stopped";
    }
    const chunks = chunkText(text);
    for (const chunk of chunks) {
      if (this.token !== token) return "stopped";
      const result = await this.speakChunk(chunk, token);
      if (result === "stopped") return "stopped";
    }
    return "done";
  }

  private speakChunk(text: string, token: number): Promise<PlayResult> {
    return new Promise((resolve) => {
      const utterance = new SpeechSynthesisUtterance(text);
      utterance.rate = this.rate;
      const voice = speechSynthesis.getVoices().find((item) => item.voiceURI === this.voiceURI());
      if (voice) utterance.voice = voice;
      let settled = false;
      let safety = 0;
      const finish = (result: PlayResult) => {
        if (settled) return;
        settled = true;
        window.clearTimeout(safety);
        this.resolvers.delete(release);
        resolve(result);
      };
      const release = () => finish("stopped");
      this.resolvers.add(release);
      let remaining = Math.round(Math.min(120000, 2500 + text.length * 90) / Math.max(0.6, this.rate));
      let last = Date.now();
      const arm = () => {
        safety = window.setTimeout(() => {
          const now = Date.now();
          if (!this.hold) remaining -= now - last;
          last = now;
          if (this.hold || remaining > 0) arm();
          else finish(this.token === token ? "done" : "stopped");
        }, 200);
      };
      arm();
      utterance.onend = () => finish(this.token === token ? "done" : "stopped");
      utterance.onerror = () => finish(this.token === token ? "done" : "stopped");
      speechSynthesis.speak(utterance);
    });
  }

  private setSource(label: string): void {
    this.sourceLabel = label;
    this.onSource?.(label);
  }
}

async function blobUrl(response: Response, label: string): Promise<{ url: string; revoke: boolean; label: string }> {
  const blob = await response.blob();
  return { url: URL.createObjectURL(blob), revoke: true, label };
}

function chunkText(text: string): string[] {
  const parts = text.match(/[^.!?]+[.!?]+|[^.!?]+$/g) ?? [text];
  const chunks: string[] = [];
  let current = "";
  for (const part of parts) {
    if ((current + part).length > 240 && current) {
      chunks.push(current.trim());
      current = part;
    } else {
      current += part;
    }
  }
  if (current.trim()) chunks.push(current.trim());
  return chunks.length ? chunks : [text];
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

export async function downloadClips(
  clips: { url: string }[],
  onProgress: (done: number, total: number) => void,
): Promise<number> {
  if (typeof caches === "undefined") throw new Error("This browser cannot store audio for offline use.");
  if (navigator.storage?.persist) {
    try {
      await navigator.storage.persist();
    } catch {
      /* the quota may still allow the download */
    }
  }
  const cache = await caches.open(AUDIO_CACHE);
  let done = 0;
  let bytes = 0;
  for (const clip of clips) {
    const response = await fetch(clip.url);
    if (!response.ok) throw new Error(`Missing audio file (${response.status}).`);
    const copy = response.clone();
    const buffer = await copy.arrayBuffer();
    bytes += buffer.byteLength;
    await cache.put(clip.url, new Response(buffer, { headers: { "Content-Type": "audio/mpeg" } }));
    done += 1;
    onProgress(done, clips.length);
  }
  return bytes;
}

export async function deleteClips(urls: string[]): Promise<void> {
  if (typeof caches === "undefined") return;
  const cache = await caches.open(AUDIO_CACHE);
  await Promise.all(urls.map((url) => cache.delete(url)));
}

export async function cachedUrls(): Promise<Set<string>> {
  if (typeof caches === "undefined") return new Set();
  const cache = await caches.open(AUDIO_CACHE);
  const keys = await cache.keys();
  return new Set(keys.map((key) => key.url));
}

export function clipUrl(path: string): string {
  return new URL(path, window.location.origin + import.meta.env.BASE_URL).toString();
}
