import { useEffect, useRef, useState } from "preact/hooks";
import { clipKey, flashcardUtterance, lessonUtterances, questionUtterances, VOICE_ID } from "../shared/audioKey.js";
import { SessionEngine, type PlayerView, type SessionSummary } from "./lib/engine";
import { loadLibrary, loadManifest } from "./lib/load";
import {
  exportProgress,
  importProgress,
  loadState,
  saveAttempt,
  saveCard,
  saveSettings,
  touchDay,
  type AttemptRecord,
  type ProgressFile,
} from "./lib/db";
import { isDue, streakFromDates, dateKey, type SrsCard } from "./lib/srs";
import { readiness, weakTopics } from "./lib/scoring";
import { buildDrive, buildLesson, buildMistakes, buildMock, buildQuiz, buildRapid, type SessionPlan } from "./lib/session";
import { cachedUrls, clipUrl, deleteClips, downloadClips } from "./lib/speaker";
import { DEFAULT_SETTINGS, type AudioManifest, type ExamId, type Lesson, type Library, type PlanId, type Settings } from "./lib/types";
import { SessionScreen } from "./player";

type Route =
  | { name: "home" }
  | { name: "lessons" }
  | { name: "lesson"; id: string }
  | { name: "quiz" }
  | { name: "rapid" }
  | { name: "mock" }
  | { name: "mistakes" }
  | { name: "stats" }
  | { name: "settings" }
  | { name: "play" }
  | { name: "summary" };

export function App() {
  const [library, setLibrary] = useState<Library | null>(null);
  const [manifest, setManifest] = useState<AudioManifest | null>(null);
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  const [cards, setCards] = useState<SrsCard[]>([]);
  const [attempts, setAttempts] = useState<AttemptRecord[]>([]);
  const [days, setDays] = useState<string[]>([]);
  const [route, setRoute] = useState<Route>({ name: "home" });
  const [error, setError] = useState("");
  const [plan, setPlan] = useState<SessionPlan | null>(null);
  const [summary, setSummary] = useState<SessionSummary | null>(null);
  const settingsRef = useRef(settings);
  settingsRef.current = settings;

  useEffect(() => {
    void (async () => {
      try {
        const [loaded, audio, state] = await Promise.all([loadLibrary(), loadManifest(), loadState(DEFAULT_SETTINGS)]);
        setLibrary(loaded);
        setManifest(audio);
        setSettings(state.settings);
        setCards(state.cards);
        setAttempts(state.attempts);
        setDays(state.days.map((day) => day.date));
      } catch (err) {
        setError(err instanceof Error ? err.message : "Could not load the study files.");
      }
    })();
  }, []);

  useEffect(() => {
    const sync = () => setRoute(readRoute());
    window.addEventListener("hashchange", sync);
    sync();
    return () => window.removeEventListener("hashchange", sync);
  }, []);

  function updateSettings(next: Settings) {
    setSettings(next);
    void saveSettings(next);
  }

  function go(hash: string) {
    location.hash = hash;
  }

  function start(next: SessionPlan) {
    if (next.items.length === 0) {
      setError("Nothing is queued for that session yet.");
      return;
    }
    setError("");
    setPlan(next);
    setSummary(null);
    location.hash = "#/play";
  }

  if (error && !library) {
    return <main className="shell"><h1>Drive Study</h1><p>{error}</p></main>;
  }
  if (!library || !manifest) {
    return <main className="shell"><h1>Drive Study</h1><p className="lede">Loading the question files.</p></main>;
  }

  const today = dateKey(new Date());
  const due = cards.filter((card) => isDue(card, new Date()));
  const streak = streakFromDates(days, today);

  return (
    <main className="shell">
      {route.name !== "play" && (
        <header className="top">
          <button className="brand" type="button" onClick={() => go("#/")}>
            <b>DRIVE</b>
            <span>Study</span>
          </button>
          <button className="icon-btn" type="button" onClick={() => go("#/settings")} aria-label="Settings">
            Set
          </button>
        </header>
      )}
      {route.name === "home" && (
        <Home
          library={library}
          settings={settings}
          due={due.length}
          streak={streak}
          onPlan={(planId) => updateSettings({ ...settings, plan: planId })}
          onDrive={() =>
            start(
              buildDrive({
                exam: settings.plan,
                questions: library.questions,
                cards: library.cards,
                lessons: library.lessons,
                dueIds: new Set(due.map((card) => card.id)),
                weakSectionIds: weakSectionIds(library, attempts, settings.plan),
                completedLessons: settings.completedLessons,
                listenOnly: settings.answerMode === "listen",
              }),
            )
          }
          onGo={go}
        />
      )}
      {route.name === "lessons" && <LessonList library={library} plan={settings.plan} onOpen={(id) => go(`#/lesson/${id}`)} />}
      {route.name === "lesson" && (
        <LessonDetail
          lesson={library.lessons.find((lesson) => lesson.id === route.id) ?? null}
          onStart={(lesson) => start(buildLesson(lesson, library.questions, settings.answerMode === "listen"))}
        />
      )}
      {route.name === "quiz" && (
        <QuizSetup
          library={library}
          plan={settings.plan}
          onStart={(exam, sectionId, count) =>
            start(
              buildQuiz({
                exam,
                sectionId,
                count,
                questions: library.questions,
                sections: exam === "BOTH" ? [] : library.exams[exam].sections,
                listenOnly: settings.answerMode === "listen",
              }),
            )
          }
        />
      )}
      {route.name === "rapid" && (
        <RapidSetup
          library={library}
          plan={settings.plan}
          onStart={(exam, sectionId, count) =>
            start(buildRapid({ exam, sectionId, count, cards: library.cards, listenOnly: settings.answerMode === "listen" }))
          }
        />
      )}
      {route.name === "mock" && (
        <MockSetup
          library={library}
          plan={settings.plan}
          onStart={(exam, timed) =>
            start(
              buildMock({
                exam,
                timed,
                questions: library.questions,
                blueprint: library.exams[exam],
                listenOnly: settings.answerMode === "listen",
              }),
            )
          }
        />
      )}
      {route.name === "mistakes" && (
        <Mistakes
          due={due}
          cards={cards}
          onStart={() =>
            start(
              buildMistakes({
                exam: settings.plan,
                questions: library.questions,
                cards: library.cards,
                dueIds: new Set(due.map((card) => card.id)),
                listenOnly: settings.answerMode === "listen",
              }),
            )
          }
        />
      )}
      {route.name === "stats" && <Stats library={library} attempts={attempts} cards={cards} days={days} />}
      {route.name === "settings" && (
        <SettingsScreen
          library={library}
          manifest={manifest}
          settings={settings}
          onChange={updateSettings}
          onImported={async () => {
            const state = await loadState(DEFAULT_SETTINGS);
            setSettings(state.settings);
            setCards(state.cards);
            setAttempts(state.attempts);
            setDays(state.days.map((day) => day.date));
          }}
        />
      )}
      {route.name === "play" && !plan && (
        <section>
          <h1>Session ended</h1>
          <button className="big" type="button" onClick={() => go("#/")}>Home</button>
        </section>
      )}
      {route.name === "play" && plan && (
        <Play
          plan={plan}
          manifest={manifest}
          settings={settings}
          cards={cards}
          onRate={(rate) => updateSettings({ ...settingsRef.current, rate })}
          onPersist={(card, attempt) => {
            setCards((current) => {
              const next = current.filter((item) => item.id !== card.id);
              next.push(card);
              return next;
            });
            setAttempts((current) => [...current, attempt]);
            setDays((current) => (current.includes(today) ? current : [...current, today]));
            void saveCard(card);
            void saveAttempt(attempt);
            void touchDay(today);
          }}
          onDone={(done) => {
            setSummary(done);
            if (done.lessonId) {
              const completed = settingsRef.current.completedLessons.includes(done.lessonId)
                ? settingsRef.current.completedLessons
                : [...settingsRef.current.completedLessons, done.lessonId];
              updateSettings({ ...settingsRef.current, completedLessons: completed });
            }
            location.hash = "#/summary";
          }}
        />
      )}
      {route.name === "summary" && summary && <Summary summary={summary} onHome={() => go("#/")} />}
      {error && <p className="bad">{error}</p>}
      {route.name !== "play" && <p className="footer">{library.disclaimer}</p>}
    </main>
  );
}

function Play(props: {
  plan: SessionPlan;
  manifest: AudioManifest;
  settings: Settings;
  cards: SrsCard[];
  onRate: (rate: number) => void;
  onPersist: (card: SrsCard, attempt: AttemptRecord) => void;
  onDone: (summary: SessionSummary) => void;
}) {
  const [view, setView] = useState<PlayerView | null>(null);
  const engineRef = useRef<SessionEngine | null>(null);
  const settingsRef = useRef(props.settings);
  settingsRef.current = props.settings;
  useEffect(() => {
    const engine = new SessionEngine(
      props.plan,
      props.manifest,
      () => settingsRef.current,
      props.cards,
      setView,
      props.onPersist,
      props.onDone,
    );
    engineRef.current = engine;
    engine.start();
    return () => engine.dispose();
  }, [props.plan]);
  return (
    <SessionScreen
      view={view}
      rate={props.settings.rate}
      onRate={(rate) => {
        props.onRate(rate);
        engineRef.current?.setRate(rate);
      }}
      onAnswer={(key) => engineRef.current?.answer(key)}
      onCommand={(type) => engineRef.current?.command(type)}
      onGuess={() => engineRef.current?.toggleGuess()}
      onResume={() => engineRef.current?.resume()}
    />
  );
}

function Home(props: {
  library: Library;
  settings: Settings;
  due: number;
  streak: number;
  onPlan: (plan: PlanId) => void;
  onDrive: () => void;
  onGo: (hash: string) => void;
}) {
  return (
    <section>
      <h1 data-testid="home-title">Drive Study</h1>
      <p className="lede">
        SIE and Series 7, out loud. {props.due} due. {props.streak} day streak.
      </p>
      <div className="seg" aria-label="Exam plan">
        {(["SIE", "S7", "BOTH"] as PlanId[]).map((plan) => (
          <button key={plan} type="button" aria-pressed={props.settings.plan === plan} onClick={() => props.onPlan(plan)}>
            {plan === "S7" ? "Series 7" : plan === "BOTH" ? "Both" : "SIE"}
          </button>
        ))}
      </div>
      <button className="big" type="button" data-testid="drive-start" onClick={props.onDrive}>
        Drive, 20 minutes
      </button>
      <div className="grid">
        <Mode name="Lessons" detail={`${props.library.lessons.length} sample lessons`} hash="#/lessons" onGo={props.onGo} />
        <Mode name="Quiz" detail="10, 20, or 30" hash="#/quiz" onGo={props.onGo} />
        <Mode name="Rapid fire" detail="Terms, short" hash="#/rapid" onGo={props.onGo} />
        <Mode name="Mock exam" detail="Weighted, timed or car" hash="#/mock" onGo={props.onGo} />
        <Mode name="Mistakes" detail={`${props.due} due`} hash="#/mistakes" onGo={props.onGo} />
        <Mode name="Stats" detail="Readiness" hash="#/stats" onGo={props.onGo} />
      </div>
      <div className="banner">
        <strong>Sample pack.</strong> These questions are originals for trying the app. A full bank can replace the JSON later. Saying answers needs a network connection on an iPhone. The buttons always work.
      </div>
    </section>
  );
}

function Mode(props: { name: string; detail: string; hash: string; onGo: (hash: string) => void }) {
  return (
    <button className="mode" type="button" data-testid={`mode-${props.name.toLowerCase().split(" ")[0]}`} onClick={() => props.onGo(props.hash)}>
      <b>{props.name}</b>
      <small>{props.detail}</small>
    </button>
  );
}

function LessonList(props: { library: Library; plan: PlanId; onOpen: (id: string) => void }) {
  const lessons = props.library.lessons.filter((lesson) => props.plan === "BOTH" || lesson.exam === props.plan);
  return (
    <section>
      <h1>Lessons</h1>
      <p className="lede">SIE foundations come first when you are studying both exams.</p>
      <div className="stack">
        {lessons.map((lesson) => (
          <button key={lesson.id} className="text-btn list-btn" type="button" onClick={() => props.onOpen(lesson.id)}>
            <b>{lesson.title}</b>
            <small>{lesson.section_title} · about {lesson.est_minutes} min</small>
          </button>
        ))}
      </div>
    </section>
  );
}

function LessonDetail(props: { lesson: Lesson | null; onStart: (lesson: Lesson) => void }) {
  if (!props.lesson) return <p>That lesson is not in the library.</p>;
  const lesson = props.lesson;
  return (
    <section>
      <p className="kicker">{lesson.section_title}</p>
      <h1>{lesson.title}</h1>
      <p>{lesson.summary}</p>
      <div className="stack">
        {lesson.key_terms.map((term) => (
          <div key={term.term} className="card">
            <b>{term.term}</b>
            <p>{term.definition}</p>
          </div>
        ))}
      </div>
      {lesson.mnemonics.length > 0 && <p className="note">{lesson.mnemonics.join(" ")}</p>}
      <button className="big" type="button" onClick={() => props.onStart(lesson)}>Play lesson</button>
    </section>
  );
}

function QuizSetup(props: {
  library: Library;
  plan: PlanId;
  onStart: (exam: PlanId, sectionId: string | null, count: number) => void;
}) {
  const [exam, setExam] = useState<PlanId>(props.plan);
  const [sectionId, setSectionId] = useState<string>("");
  const [count, setCount] = useState(10);
  const sections = exam === "BOTH" ? [] : props.library.exams[exam].sections;
  return (
    <section>
      <h1>Quiz</h1>
      <PlanPick value={exam} onChange={setExam} allowBoth />
      {exam !== "BOTH" && (
        <label className="field">
          Section
          <select value={sectionId} onChange={(event) => setSectionId(event.currentTarget.value)}>
            <option value="">All sections, weighted</option>
            {sections.map((section) => (
              <option key={section.id} value={section.id}>{section.title}</option>
            ))}
          </select>
        </label>
      )}
      <CountPick value={count} options={[10, 20, 30]} onChange={setCount} />
      <button className="big" type="button" data-testid="start-quiz" onClick={() => props.onStart(exam, sectionId || null, count)}>
        Start quiz
      </button>
    </section>
  );
}

function RapidSetup(props: {
  library: Library;
  plan: PlanId;
  onStart: (exam: PlanId, sectionId: string | null, count: number) => void;
}) {
  const [exam, setExam] = useState<PlanId>(props.plan);
  const [count, setCount] = useState(10);
  return (
    <section>
      <h1>Rapid fire</h1>
      <p className="lede">The term, a pause, then the definition. Tap Know it or Don't know.</p>
      <PlanPick value={exam} onChange={setExam} allowBoth />
      <CountPick value={count} options={[10, 20]} onChange={setCount} />
      <button className="big" type="button" onClick={() => props.onStart(exam, null, count)}>Start rapid fire</button>
    </section>
  );
}

function MockSetup(props: {
  library: Library;
  plan: PlanId;
  onStart: (exam: ExamId, timed: boolean) => void;
}) {
  const initial: ExamId = props.plan === "S7" ? "S7" : "SIE";
  const [exam, setExam] = useState<ExamId>(initial);
  const blueprint = props.library.exams[exam];
  const have = props.library.questions.filter((question) => question.exam === exam).length;
  return (
    <section>
      <h1>Mock exam</h1>
      <p className="lede">
        {blueprint.name}: {blueprint.questions} scored questions, {blueprint.minutes} minutes, {blueprint.pass_percent}% to pass.
        The library has {have}. A short pool still uses the blueprint weights.
      </p>
      <div className="seg">
        {(["SIE", "S7"] as ExamId[]).map((id) => (
          <button key={id} type="button" aria-pressed={exam === id} onClick={() => setExam(id)}>
            {id === "S7" ? "Series 7" : "SIE"}
          </button>
        ))}
      </div>
      <div className="stack">
        <button className="big" type="button" onClick={() => props.onStart(exam, true)}>Timed</button>
        <button className="big secondary" type="button" onClick={() => props.onStart(exam, false)}>Car mode, no clock</button>
      </div>
    </section>
  );
}

function Mistakes(props: { due: SrsCard[]; cards: SrsCard[]; onStart: () => void }) {
  const graduated = props.cards.filter((card) => card.graduated).length;
  const nextDue = props.cards
    .filter((card) => !card.graduated)
    .map((card) => card.due)
    .sort((a, b) => a - b)[0];
  return (
    <section>
      <h1>Mistakes</h1>
      <p className="lede">
        Wrong answers and guesses land here. An item graduates after you get it right on three different days. {graduated} graduated.
      </p>
      {props.due.length === 0 ? (
        <p>{nextDue ? `Next review ${new Date(nextDue).toLocaleDateString()}.` : "Nothing in the deck yet. Miss one on purpose and it will show up."}</p>
      ) : (
        <button className="big" type="button" onClick={props.onStart}>Start mistakes drive ({props.due.length})</button>
      )}
    </section>
  );
}

function Stats(props: { library: Library; attempts: AttemptRecord[]; cards: SrsCard[]; days: string[] }) {
  const today = dateKey(new Date());
  return (
    <section>
      <h1>Stats</h1>
      <p className="lede">{streakFromDates(props.days, today)} day streak. {props.attempts.length} answers saved on this phone.</p>
      {(["SIE", "S7"] as ExamId[]).map((exam) => {
        const sections = props.library.exams[exam].sections;
        const examAttempts = props.attempts.filter((attempt) => attempt.exam === exam && attempt.kind === "question");
        const seen = new Set(examAttempts.map((attempt) => attempt.itemId)).size;
        const pool = props.library.questions.filter((question) => question.exam === exam).length;
        const ready = readiness({
          sections,
          attempts: examAttempts.map((attempt) => ({ section: attempt.section, topic: attempt.topic, correct: attempt.correct })),
          seen,
          pool,
        });
        const topics = weakTopics(examAttempts.map((attempt) => ({ section: attempt.section, topic: attempt.topic, correct: attempt.correct })));
        return (
          <article key={exam} className="card">
            <h2>{exam === "S7" ? "Series 7" : "SIE"}</h2>
            <p className="prompt">{ready.score == null ? "—" : `${ready.score}`}</p>
            <p className="muted">
              Readiness against a {props.library.exams[exam].pass_percent}% pass mark.
              {ready.score == null ? " Answer at least 3 questions in a section." : ` Library coverage ${Math.round(ready.coverage * 100)}%.`}
            </p>
            {sections.map((section) => {
              const rows = examAttempts.filter((attempt) => attempt.section === section.id);
              const accuracy = rows.length ? rows.filter((row) => row.correct).length / rows.length : 0;
              return (
                <div key={section.id} className="meter">
                  <div className="spread"><span>{section.title}</span><span>{rows.length ? `${Math.round(accuracy * 100)}%` : "—"}</span></div>
                  <div className="bar"><span style={{ width: `${accuracy * 100}%` }} /></div>
                </div>
              );
            })}
            {topics.length > 0 && <p>Weak topics: {topics.map((topic) => topic.topic).join(", ")}.</p>}
          </article>
        );
      })}
    </section>
  );
}

function SettingsScreen(props: {
  library: Library;
  manifest: AudioManifest;
  settings: Settings;
  onChange: (settings: Settings) => void;
  onImported: () => Promise<void>;
}) {
  const [voices, setVoices] = useState<SpeechSynthesisVoice[]>([]);
  const [storage, setStorage] = useState("");
  const [cached, setCached] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState("");
  useEffect(() => {
    const loadVoices = () => setVoices(window.speechSynthesis?.getVoices?.() ?? []);
    loadVoices();
    window.speechSynthesis?.addEventListener?.("voiceschanged", loadVoices);
    void cachedUrls().then(setCached);
    void navigator.storage?.estimate?.().then((estimate) => {
      const used = estimate.usage ?? 0;
      const quota = estimate.quota ?? 0;
      setStorage(`${formatBytes(used)} used of about ${formatBytes(quota)}.`);
    });
    return () => window.speechSynthesis?.removeEventListener?.("voiceschanged", loadVoices);
  }, []);

  const sections = [...props.library.exams.SIE.sections, ...props.library.exams.S7.sections];
  return (
    <section>
      <h1>Settings</h1>
      <h2>Answer mode</h2>
      <div className="seg">
        <button type="button" aria-pressed={props.settings.answerMode === "spoken"} onClick={() => props.onChange({ ...props.settings, answerMode: "spoken" })}>Spoken answers</button>
        <button type="button" aria-pressed={props.settings.answerMode === "listen"} onClick={() => props.onChange({ ...props.settings, answerMode: "listen" })}>Listen only</button>
      </div>
      <p className="muted">Listen only reads the question, waits {props.settings.pauseSeconds} seconds, then reads the answer. Spoken mode listens for A, B, C, D, alpha, bravo, charlie, delta, true, false, repeat, skip, explain, I don't know, pause, and stop.</p>
      <h2>Voice</h2>
      <div className="stack">
        <button className="text-btn" type="button" aria-pressed={props.settings.voiceSource === "downloaded"} onClick={() => props.onChange({ ...props.settings, voiceSource: "downloaded" })}>
          <b>Downloaded natural audio</b>
          <small>Saved on the phone. If a clip is missing, the app streams it, then uses the phone voice.</small>
        </button>
        <button className="text-btn" type="button" aria-pressed={props.settings.voiceSource === "streamed"} onClick={() => props.onChange({ ...props.settings, voiceSource: "streamed" })}>
          <b>Streamed natural audio</b>
          <small>Plays from the site while you are online. It is not kept unless you download it.</small>
        </button>
        <button className="text-btn" type="button" aria-pressed={props.settings.voiceSource === "phone"} onClick={() => props.onChange({ ...props.settings, voiceSource: "phone" })}>
          <b>Phone voice</b>
          <small>The iPhone voice. Works offline. It often stops when the screen locks.</small>
        </button>
      </div>
      <label className="field">
        Speed {props.settings.rate.toFixed(1)}x, for every voice
        <input
          type="range"
          min="0.8"
          max="2"
          step="0.1"
          value={props.settings.rate}
          onInput={(event) => props.onChange({ ...props.settings, rate: Number(event.currentTarget.value) })}
        />
      </label>
      <label className="field">
        Pause before the answer, {props.settings.pauseSeconds} seconds
        <input
          type="range"
          min="3"
          max="12"
          step="1"
          value={props.settings.pauseSeconds}
          onInput={(event) => props.onChange({ ...props.settings, pauseSeconds: Number(event.currentTarget.value) })}
        />
      </label>
      <label className="field">
        Phone voice
        <select
          value={props.settings.voiceURI ?? ""}
          onChange={(event) => props.onChange({ ...props.settings, voiceURI: event.currentTarget.value || null })}
        >
          <option value="">System default</option>
          {voices.map((voice) => (
            <option key={voice.voiceURI} value={voice.voiceURI}>{voice.name} ({voice.lang})</option>
          ))}
        </select>
      </label>
      <h2>Downloads</h2>
      <p className="muted">{storage || "Storage estimate unavailable."} Published clips: {Object.keys(props.manifest.clips).length}.</p>
      {sections.map((section) => {
        const pack = clipsForSection(props.library, props.manifest, section.id);
        const ready = pack.clips.filter((clip) => cached.has(clip.url)).length;
        return (
          <div key={section.id} className="card">
            <div className="spread">
              <b>{section.title}</b>
              <span>{pack.clips.length} clips</span>
            </div>
            <p className="muted">{ready} saved · about {formatBytes(pack.clips.reduce((sum, clip) => sum + clip.bytes, 0))} · {pack.utterances} spoken lines</p>
            <div className="row">
              <button
                type="button"
                disabled={pack.clips.length === 0 || busy === section.id}
                onClick={() => {
                  setBusy(section.id);
                  void downloadClips(pack.clips, () => undefined)
                    .then(async () => setCached(await cachedUrls()))
                    .catch((err: unknown) => alert(err instanceof Error ? err.message : "Download failed."))
                    .finally(() => setBusy(""));
                }}
              >
                Download
              </button>
              <button
                type="button"
                onClick={() => {
                  void deleteClips(pack.clips.map((clip) => clip.url)).then(async () => setCached(await cachedUrls()));
                }}
              >
                Delete
              </button>
            </div>
          </div>
        );
      })}
      <h2>Progress</h2>
      <div className="row">
        <button
          type="button"
          onClick={() => {
            void exportProgress(props.settings).then((file) => downloadJson(file));
          }}
        >
          Export
        </button>
        <label className="text-btn">
          Import
          <input
            type="file"
            accept="application/json"
            onChange={(event) => {
              const file = event.currentTarget.files?.[0];
              if (!file) return;
              void file.text().then(async (text) => {
                try {
                  const parsed = JSON.parse(text) as ProgressFile;
                  if (!window.confirm("Replace the progress on this phone with this file?")) return;
                  await importProgress(parsed);
                  await props.onImported();
                } catch (err) {
                  window.alert(err instanceof Error ? err.message : "Could not import that file.");
                }
              });
            }}
          />
        </label>
      </div>
      <h2>On an iPhone</h2>
      <p>
        Install from Safari: Share, then Add to Home Screen. Speech recognition needs a network connection and the microphone.
        The phone voice usually stops when the screen locks. Downloaded or streamed audio keeps going more reliably, and Apple Podcasts is the sure way to listen with the phone locked.
        Wake lock only applies while this page is on screen.
      </p>
      <p>
        Podcast feed: <a href={`${import.meta.env.BASE_URL}podcast/feed.xml`}>podcast/feed.xml</a>.
        Episodes show up when the audio branch publishes lesson and quiz mp3s.
      </p>
    </section>
  );
}

function Summary(props: { summary: SessionSummary; onHome: () => void }) {
  const summary = props.summary;
  return (
    <section>
      <h1>{summary.title}</h1>
      <p className="prompt" data-testid="summary-score">{summary.percent}%</p>
      <p>
        {summary.correct} right, {summary.wrong} missed, {summary.skipped} skipped.
        {summary.passed == null ? "" : summary.passed ? " That clears the practice pass mark." : " Under the practice pass mark."}
        {summary.timedOut ? " Time ran out." : ""}
      </p>
      <button className="big" type="button" onClick={props.onHome}>Home</button>
    </section>
  );
}

function PlanPick(props: { value: PlanId; onChange: (plan: PlanId) => void; allowBoth: boolean }) {
  const options: PlanId[] = props.allowBoth ? ["SIE", "S7", "BOTH"] : ["SIE", "S7"];
  return (
    <div className="seg">
      {options.map((plan) => (
        <button key={plan} type="button" aria-pressed={props.value === plan} onClick={() => props.onChange(plan)}>
          {plan === "S7" ? "Series 7" : plan === "BOTH" ? "Both" : "SIE"}
        </button>
      ))}
    </div>
  );
}

function CountPick(props: { value: number; options: number[]; onChange: (count: number) => void }) {
  return (
    <div className="seg">
      {props.options.map((count) => (
        <button key={count} type="button" aria-pressed={props.value === count} onClick={() => props.onChange(count)}>
          {count}
        </button>
      ))}
    </div>
  );
}

function clipsForSection(library: Library, manifest: AudioManifest, sectionId: string) {
  const texts: string[] = [];
  for (const question of library.questions) {
    if (question.section === sectionId) texts.push(...questionUtterances(question).map((item) => item.text));
  }
  for (const card of library.cards) {
    if (card.section === sectionId) texts.push(flashcardUtterance(card).text);
  }
  for (const lesson of library.lessons) {
    if (lesson.section === sectionId) texts.push(...lessonUtterances(lesson).map((item) => item.text));
  }
  const clips = [];
  for (const text of texts) {
    const entry = manifest.clips[clipKey(VOICE_ID, text)];
    if (!entry) continue;
    clips.push({ url: clipUrl(entry.path), bytes: entry.bytes ?? 0 });
  }
  return { utterances: texts.length, clips };
}

function weakSectionIds(library: Library, attempts: AttemptRecord[], plan: PlanId): string[] {
  const exams: ExamId[] = plan === "BOTH" ? ["SIE", "S7"] : [plan];
  const weak: { id: string; accuracy: number }[] = [];
  for (const exam of exams) {
    for (const section of library.exams[exam].sections) {
      const rows = attempts.filter((attempt) => attempt.section === section.id && attempt.kind === "question");
      if (rows.length === 0) continue;
      const accuracy = rows.filter((row) => row.correct).length / rows.length;
      if (accuracy < 0.7) weak.push({ id: section.id, accuracy });
    }
  }
  weak.sort((a, b) => a.accuracy - b.accuracy);
  return weak.map((row) => row.id);
}

function readRoute(): Route {
  const hash = location.hash.replace(/^#/, "") || "/";
  if (hash.startsWith("/lesson/")) return { name: "lesson", id: decodeURIComponent(hash.slice("/lesson/".length)) };
  if (hash === "/lessons") return { name: "lessons" };
  if (hash === "/quiz") return { name: "quiz" };
  if (hash === "/rapid") return { name: "rapid" };
  if (hash === "/mock") return { name: "mock" };
  if (hash === "/mistakes") return { name: "mistakes" };
  if (hash === "/stats") return { name: "stats" };
  if (hash === "/settings") return { name: "settings" };
  if (hash === "/play") return { name: "play" };
  if (hash === "/summary") return { name: "summary" };
  return { name: "home" };
}

function downloadJson(file: ProgressFile) {
  const blob = new Blob([JSON.stringify(file, null, 2)], { type: "application/json" });
  const link = document.createElement("a");
  link.href = URL.createObjectURL(blob);
  link.download = `drive-study-progress-${file.exportedAt.slice(0, 10)}.json`;
  link.click();
  URL.revokeObjectURL(link.href);
}

function formatBytes(bytes: number): string {
  if (!bytes) return "0 MB";
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
