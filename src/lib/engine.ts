import { choicesReadout } from "../../shared/audioKey.js";
import { answerGrammar, parseVoiceCommand, type VoiceCommand } from "./grammar";
import { reviewCard, type SrsCard } from "./srs";
import type { AttemptRecord } from "./db";
import { Speaker } from "./speaker";
import type { SessionPlan } from "./session";
import type { AudioManifest, Flashcard, Question, Settings } from "./types";

export interface PlayerView {
  title: string;
  index: number;
  total: number;
  phase: "prompt" | "reveal" | "paused" | "done";
  kicker: string;
  prompt: string;
  detail: string;
  choices: { key: string; text: string }[];
  answerKey: string | null;
  selected: string | null;
  explanation: string;
  listening: boolean;
  heard: string;
  source: string;
  remainingSec: number | null;
  status: string;
  correct: number;
  wrong: number;
  guessing: boolean;
  sample: boolean;
  cardMode: boolean;
}

export interface SessionSummary {
  title: string;
  mode: string;
  correct: number;
  wrong: number;
  skipped: number;
  total: number;
  passed: boolean | null;
  percent: number;
  timedOut: boolean;
  lessonId?: string;
}

type Action = VoiceCommand | { type: "guess-toggle" };

export class SessionEngine {
  private speaker: Speaker;
  private stopped = false;
  private paused = false;
  private index = 0;
  private correct = 0;
  private wrong = 0;
  private skipped = 0;
  private guessing = false;
  private pending: Action | null = null;
  private listening = false;
  private heard = "";
  private status = "";
  private source = "Phone voice";
  private selected: string | null = null;
  private phase: PlayerView["phase"] = "prompt";
  private remaining: number | null = null;
  private timer = 0;
  private stopListen: (() => void) | null = null;
  private wakeLock: WakeLockSentinel | null = null;
  private responses: { section: string; correct: boolean }[] = [];
  private cards: Map<string, SrsCard>;
  private onVis = () => {
    if (document.visibilityState === "visible" && !this.stopped) void this.acquireWake();
  };

  constructor(
    private readonly plan: SessionPlan,
    manifest: AudioManifest,
    private readonly settings: () => Settings,
    cards: SrsCard[],
    private readonly onView: (view: PlayerView) => void,
    private readonly onPersist: (card: SrsCard, attempt: AttemptRecord) => void,
    private readonly onDone: (summary: SessionSummary) => void,
  ) {
    this.cards = new Map(cards.map((card) => [card.id, card]));
    this.speaker = new Speaker(
      () => manifest,
      () => settings().voiceSource,
      () => settings().voiceURI,
    );
    this.speaker.onSource = (label) => {
      this.source = label;
      this.render();
    };
    if (plan.timed && plan.limitSeconds) this.remaining = plan.limitSeconds;
  }

  start(): void {
    this.speaker.arm();
    this.speaker.setRate(this.settings().rate);
    this.installMediaSession();
    void this.acquireWake();
    document.addEventListener("visibilitychange", this.onVis);
    if (this.remaining != null) this.timer = window.setInterval(() => this.tick(), 1000);
    void this.loop();
  }

  setRate(rate: number): void {
    this.speaker.setRate(rate);
  }

  answer(key: string): void {
    this.queue({ type: "answer", key });
  }

  command(type: "repeat" | "skip" | "explain" | "dontknow" | "pause" | "stop" | "know"): void {
    this.queue({ type });
  }

  toggleGuess(): void {
    this.guessing = !this.guessing;
    this.render();
  }

  resume(): void {
    this.paused = false;
    this.phase = "prompt";
    this.speaker.resume();
    this.status = this.listening ? "Listening." : "";
    this.render();
    void this.acquireWake();
  }

  dispose(): void {
    this.stopped = true;
    window.clearInterval(this.timer);
    this.stopRec();
    this.speaker.stop();
    this.speaker.stopKeeper();
    document.removeEventListener("visibilitychange", this.onVis);
    void this.wakeLock?.release().catch(() => undefined);
    if (navigator.mediaSession) {
      for (const action of ["play", "pause", "nexttrack", "previoustrack"] as const) {
        try {
          navigator.mediaSession.setActionHandler(action, null);
        } catch {
          /* ignore */
        }
      }
    }
  }

  private queue(action: Action): void {
    if (this.stopped) return;
    if (action.type === "pause") {
      this.paused = true;
      this.phase = "paused";
      this.speaker.pause();
      this.stopRec();
      this.status = "Paused.";
      this.render();
      return;
    }
    if (action.type === "stop") {
      this.pending = action;
      this.stopped = true;
      this.speaker.stop();
      return;
    }
    if (this.paused && action.type !== "repeat") return;
    this.pending = action;
    this.speaker.stop();
    this.stopRec();
  }

  private take(): Action | null {
    const action = this.pending;
    this.pending = null;
    return action;
  }

  private tick(): void {
    if (this.paused || this.remaining == null || this.stopped) return;
    this.remaining -= 1;
    this.render();
    if (this.remaining <= 0) {
      this.status = "Time is up.";
      this.stopped = true;
      this.speaker.stop();
    }
  }

  private async loop(): Promise<void> {
    while (!this.stopped && this.index < this.plan.items.length) {
      const again = await this.runItem();
      if (this.stopped) break;
      if (!again) this.index += 1;
    }
    this.finish(this.remaining === 0);
  }

  private async runItem(): Promise<boolean> {
    const item = this.plan.items[this.index];
    this.guessing = false;
    this.selected = null;
    this.heard = "";
    this.phase = "prompt";
    if (item.kind === "narration") return this.runNarration(item.title, item.text);
    if (item.kind === "flashcard") return this.runCard(item.card);
    return this.runQuestion(item.question);
  }

  private async runNarration(title: string, text: string): Promise<boolean> {
    this.prompt = text;
    this.kicker = title;
    this.detail = "Lesson";
    this.choices = [];
    this.explanation = "";
    this.answerKey = null;
    this.sample = false;
    this.cardMode = false;
    this.updateMedia(title, "Lesson");
    this.render();
    await this.speakInterruptible(text);
    const action = this.take();
    if (!action) return false;
    if (action.type === "repeat") return true;
    if (action.type === "stop") {
      this.stopped = true;
      return false;
    }
    return false;
  }

  private async runCard(card: Flashcard): Promise<boolean> {
    this.kicker = `${card.exam} · ${card.section}`;
    this.prompt = card.front;
    this.detail = "Rapid fire";
    this.choices = [];
    this.explanation = card.back;
    this.answerKey = null;
    this.sample = Boolean(card.sample);
    this.cardMode = true;
    this.updateMedia(card.front, "Flashcard");
    this.render();
    await this.speakInterruptible(card.front);
    let action: Action | "timeout" | null = this.take();
    if (action?.type === "repeat") return true;
    if (action?.type === "stop") {
      this.stopped = true;
      return false;
    }
    if (action?.type === "skip") {
      this.skipped += 1;
      return false;
    }
    if (!action) {
      this.listening = true;
      this.render();
      this.startRec(["T", "F"], true);
      action = await this.waitFor(this.plan.listenOnly ? this.settings().pauseSeconds * 1000 : undefined);
      this.stopRec();
    }
    if (action === "timeout") {
      await this.speakInterruptible(card.spoken);
      return false;
    }
    if (!action || action.type === "skip") {
      this.skipped += 1;
      return false;
    }
    if (action.type === "repeat") return true;
    if (action.type === "stop") {
      this.stopped = true;
      return false;
    }
    const knew = action.type === "know";
    if (action.type === "know" || action.type === "dontknow") {
      this.grade(card.id, "flashcard", card.exam, card.section, undefined, knew ? "correct" : "wrong", false);
      if (knew) this.correct += 1;
      else this.wrong += 1;
    }
    this.phase = "reveal";
    this.render();
    await this.speakInterruptible(card.spoken);
    return false;
  }

  private async runQuestion(question: Question): Promise<boolean> {
    const choiceKeys = question.choices.map((choice) => choice.key);
    this.kicker = `${question.exam} · ${question.section}`;
    this.prompt = question.stem;
    this.detail = question.topic.replace(/-/g, " ");
    this.choices = question.choices;
    this.explanation = "";
    this.answerKey = null;
    this.sample = Boolean(question.sample);
    this.cardMode = false;
    this.updateMedia(question.stem, question.exam === "S7" ? "Series 7" : "SIE");
    this.render();

    await this.speakInterruptible(question.spoken_stem);
    if (await this.consumeEarly(question)) return this.earlyRepeat;
    await this.speakInterruptible(choicesReadout(question));
    if (await this.consumeEarly(question)) return this.earlyRepeat;

    this.listening = true;
    this.status = this.plan.listenOnly
      ? "Answers after the pause. You can tap sooner."
      : "Listening. Say A, B, C, or D, or tap.";
    this.render();
    this.startRec(choiceKeys, false);
    const waited = await this.waitFor(this.plan.listenOnly ? this.settings().pauseSeconds * 1000 : undefined);
    this.stopRec();
    if (waited === "timeout") {
      await this.reveal(question, null);
      return false;
    }
    const action = waited ?? this.take();
    if (action?.type === "repeat") return true;
    if (action?.type === "stop") {
      this.stopped = true;
      return false;
    }
    if (action?.type === "skip") {
      this.skipQuestion(question);
      return false;
    }
    if (!action || action.type === "explain" || action.type === "dontknow") {
      this.grade(question.id, "question", question.exam, question.section, question.topic, "wrong", false);
      this.wrong += 1;
      this.responses.push({ section: question.section, correct: false });
      await this.reveal(question, null);
      return false;
    }
    if (action.type === "answer") {
      await this.scoreAnswer(question, action.key);
      return false;
    }
    return false;
  }

  private earlyRepeat = false;

  private async consumeEarly(question: Question): Promise<boolean> {
    const action = this.take();
    if (!action) return false;
    this.earlyRepeat = false;
    if (action.type === "repeat") {
      this.earlyRepeat = true;
      return true;
    }
    if (action.type === "stop") {
      this.stopped = true;
      return true;
    }
    if (action.type === "skip") {
      this.skipQuestion(question);
      return true;
    }
    if (action.type === "explain" || action.type === "dontknow") {
      this.grade(question.id, "question", question.exam, question.section, question.topic, "wrong", false);
      this.wrong += 1;
      this.responses.push({ section: question.section, correct: false });
      await this.reveal(question, null);
      return true;
    }
    if (action.type === "answer") {
      await this.scoreAnswer(question, action.key);
      return true;
    }
    return false;
  }

  private async scoreAnswer(question: Question, key: string): Promise<void> {
    const right = key === question.answer;
    const outcome = right ? (this.guessing ? "guessed" : "correct") : "wrong";
    this.selected = key;
    this.grade(question.id, "question", question.exam, question.section, question.topic, outcome, this.guessing && right);
    if (right) this.correct += 1;
    else this.wrong += 1;
    this.responses.push({ section: question.section, correct: right });
    await this.reveal(question, key);
  }

  private skipQuestion(question: Question): void {
    this.skipped += 1;
    if (this.plan.mode === "mock") {
      this.wrong += 1;
      this.responses.push({ section: question.section, correct: false });
    }
  }

  private async reveal(question: Question, selected: string | null): Promise<void> {
    this.phase = "reveal";
    this.selected = selected;
    this.answerKey = question.answer;
    this.explanation = question.explanation;
    this.listening = false;
    this.status = selected === question.answer ? "Correct." : selected ? "Not this one." : "Here is the answer.";
    if (this.guessing && selected === question.answer) this.status = "Marked as a guess. It will come back.";
    this.render();
    await this.speakInterruptible(question.spoken_explanation);
    const action = this.take();
    if (action?.type === "stop") this.stopped = true;
  }

  private async speakInterruptible(text: string): Promise<void> {
    let finished = false;
    const task = this.speaker.speak(text).finally(() => {
      finished = true;
    });
    while (!finished && !this.pending && !this.stopped && !this.paused) await delay(40);
    while (this.paused && !this.stopped) await delay(40);
    if (!finished && (this.pending || this.stopped)) this.speaker.stop();
    await task;
  }

  private waitFor(ms?: number): Promise<Action | "timeout" | null> {
    let elapsed = 0;
    let last = Date.now();
    return new Promise((resolve) => {
      const step = () => {
        if (this.stopped) {
          resolve(this.take() ?? { type: "stop" });
          return;
        }
        if (this.pending) {
          resolve(this.take());
          return;
        }
        const now = Date.now();
        if (!this.paused) elapsed += now - last;
        last = now;
        if (ms != null && elapsed >= ms) {
          resolve("timeout");
          return;
        }
        window.setTimeout(step, 80);
      };
      step();
    });
  }

  private startRec(choiceKeys: string[], allowKnow: boolean): void {
    this.stopRec();
    const Ctor = window.SpeechRecognition || window.webkitSpeechRecognition;
    if (!Ctor) {
      this.status = "This browser has no speech recognition. Use the buttons.";
      this.listening = false;
      this.render();
      return;
    }
    const rec = new Ctor();
    rec.lang = "en-US";
    rec.interimResults = true;
    rec.continuous = false;
    rec.maxAlternatives = 5;
    const Grammar = window.webkitSpeechGrammarList || window.SpeechGrammarList;
    if (Grammar) {
      try {
        const list = new Grammar();
        list.addFromString(answerGrammar(choiceKeys), 1);
        rec.grammars = list;
      } catch {
        /* the parser still handles the transcript */
      }
    }
    rec.onresult = (event) => {
      for (let i = event.resultIndex; i < event.results.length; i += 1) {
        const result = event.results[i];
        for (let alt = 0; alt < result.length; alt += 1) {
          const transcript = result[alt].transcript;
          const command = parseVoiceCommand(transcript, { choiceKeys, allowKnow });
          if (command.type === "unrecognized") continue;
          this.heard = transcript;
          this.queue(command);
          return;
        }
      }
    };
    rec.onerror = (event) => {
      const code = event.error ?? "";
      if (code === "network") this.status = "Speech recognition needs a network connection on this iPhone. Use the buttons.";
      else if (code === "not-allowed") this.status = "The microphone is blocked. Use the buttons.";
      else if (code !== "aborted" && code !== "no-speech") this.status = "Didn't catch that. Use the buttons.";
      this.render();
    };
    let restarts = 0;
    rec.onend = () => {
      if (!this.listening || this.paused || this.stopped || this.pending) return;
      restarts += 1;
      if (restarts > 6) {
        this.status = "Still here. Tap an answer if it is noisy.";
        this.render();
        return;
      }
      window.setTimeout(() => {
        try {
          rec.start();
        } catch {
          this.listening = false;
        }
      }, 300);
    };
    try {
      rec.start();
      this.listening = true;
    } catch {
      this.status = "Could not start the microphone. Use the buttons.";
      this.listening = false;
    }
    this.stopListen = () => {
      this.listening = false;
      try {
        rec.onend = null;
        rec.abort();
      } catch {
        /* already stopped */
      }
    };
  }

  private stopRec(): void {
    this.stopListen?.();
    this.stopListen = null;
    this.listening = false;
  }

  private grade(
    id: string,
    kind: "question" | "flashcard",
    exam: "SIE" | "S7",
    section: string,
    topic: string | undefined,
    outcome: "correct" | "wrong" | "guessed",
    guessed: boolean,
  ): void {
    const next = reviewCard(this.cards.get(id) ?? null, { id, kind, exam, section }, outcome, new Date());
    this.cards.set(id, next);
    this.onPersist(next, {
      itemId: id,
      kind,
      exam,
      section,
      topic,
      correct: outcome !== "wrong",
      guessed,
      mode: this.plan.mode,
      at: Date.now(),
    });
  }

  private finish(timedOut: boolean): void {
    if (this.phase === "done") return;
    if (timedOut && this.plan.mode === "mock") {
      for (let cursor = this.index; cursor < this.plan.items.length; cursor += 1) {
        const item = this.plan.items[cursor];
        if (item.kind !== "question") continue;
        if (this.responses.length > cursor) continue;
        this.responses.push({ section: item.question.section, correct: false });
        this.wrong += 1;
      }
    }
    this.phase = "done";
    this.dispose();
    const graded = this.correct + this.wrong;
    const ratio = graded === 0 ? 0 : this.correct / graded;
    const passed =
      this.plan.mode === "mock" && this.plan.passPercent != null && this.responses.length > 0
        ? ratio + 1e-12 >= this.plan.passPercent / 100
        : null;
    this.onDone({
      title: this.plan.title,
      mode: this.plan.mode,
      correct: this.correct,
      wrong: this.wrong,
      skipped: this.skipped,
      total: this.responses.length || graded,
      passed,
      percent: Math.round(ratio * 1000) / 10,
      timedOut,
      lessonId: !timedOut && this.index >= this.plan.items.length ? this.plan.lessonId : undefined,
    });
  }

  private async acquireWake(): Promise<void> {
    if (!("wakeLock" in navigator)) return;
    try {
      this.wakeLock = await navigator.wakeLock.request("screen");
    } catch {
      /* iOS releases this when the page is hidden */
    }
  }

  private installMediaSession(): void {
    if (!navigator.mediaSession) return;
    try {
      navigator.mediaSession.setActionHandler("play", () => this.resume());
      navigator.mediaSession.setActionHandler("pause", () => this.command("pause"));
      navigator.mediaSession.setActionHandler("nexttrack", () => this.command("skip"));
      navigator.mediaSession.setActionHandler("previoustrack", () => this.command("repeat"));
    } catch {
      /* some handlers are missing on older iOS */
    }
  }

  private updateMedia(title: string, album: string): void {
    if (!navigator.mediaSession) return;
    navigator.mediaSession.metadata = new MediaMetadata({
      title,
      artist: "Drive Study",
      album,
    });
    navigator.mediaSession.playbackState = "playing";
  }

  private prompt = "";
  private kicker = "";
  private detail = "";
  private choices: { key: string; text: string }[] = [];
  private explanation = "";
  private answerKey: string | null = null;

  private render(): void {
    const item = this.plan.items[this.index];
    this.onView({
      title: this.plan.title,
      index: Math.min(this.index + 1, this.plan.items.length),
      total: this.plan.items.length,
      phase: this.phase,
      kicker: this.kicker,
      prompt: this.prompt,
      detail: this.detail,
      choices: this.choices,
      answerKey: this.answerKey,
      selected: this.selected,
      explanation: this.explanation,
      listening: this.listening,
      heard: this.heard,
      source: this.source,
      remainingSec: this.remaining,
      status: this.status,
      correct: this.correct,
      wrong: this.wrong,
      guessing: this.guessing,
      sample: this.sample || Boolean(item && "question" in item && item.question.sample),
      cardMode: this.cardMode,
    });
  }

  private cardMode = false;
  private sample = false;
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}
