import type { PlayerView } from "./lib/engine";

export function SessionScreen(props: {
  view: PlayerView | null;
  rate: number;
  onRate: (rate: number) => void;
  onAnswer: (key: string) => void;
  onCommand: (type: "repeat" | "skip" | "explain" | "dontknow" | "pause" | "stop" | "know") => void;
  onGuess: () => void;
  onResume: () => void;
}) {
  const view = props.view;
  if (!view) return <p className="lede">Starting the session.</p>;
  const speeds = [0.8, 1, 1.25, 1.5, 2];
  return (
    <section>
      <div className="player-top">
        <button className="icon-btn" type="button" onClick={() => props.onCommand("stop")}>
          End
        </button>
        <div>
          <div className="kicker">{view.title}</div>
          <div>
            {view.index} / {view.total}
            {view.remainingSec != null ? ` · ${formatClock(view.remainingSec)}` : ""}
          </div>
        </div>
        <button className="icon-btn" type="button" onClick={() => (view.phase === "paused" ? props.onResume() : props.onCommand("pause"))}>
          {view.phase === "paused" ? "Play" : "Pause"}
        </button>
      </div>
      <div className="bar" aria-hidden="true">
        <span style={{ width: `${view.total ? (view.index / view.total) * 100 : 0}%` }} />
      </div>
      <p className="kicker">
        {view.kicker}
        {view.sample ? " · Sample" : ""} · {view.source}
      </p>
      <h1 className="prompt">{view.prompt}</h1>
      <p className="status" aria-live="polite">
        {view.phase === "paused" ? "Paused. Playback is held." : view.status}
        {view.heard ? ` Heard “${view.heard}”.` : ""}
        {view.listening ? " Listening." : ""}
      </p>
      {view.choices.length > 0 && (
        <div className="choices">
          {view.choices.map((choice) => {
            const revealed = view.answerKey != null;
            const right = revealed && choice.key === view.answerKey;
            const wrong = revealed && view.selected === choice.key && !right;
            return (
              <button
                key={choice.key}
                className={`choice-btn${right ? " right" : ""}${wrong ? " wrong" : ""}`}
                type="button"
                data-testid={`choice-${choice.key}`}
                onClick={() => {
                  if (!revealed) props.onAnswer(choice.key);
                }}
              >
                <b>{choice.key}</b>
                <span>{choice.text}</span>
              </button>
            );
          })}
        </div>
      )}
      {view.phase === "reveal" && view.explanation && (
        <div className="card" data-testid="explanation">
          <p className="kicker">Answer {view.answerKey}</p>
          <p>{view.explanation}</p>
        </div>
      )}
      <div className="speeds" aria-label="Playback speed">
        {speeds.map((speed) => (
          <button key={speed} type="button" aria-pressed={props.rate === speed} onClick={() => props.onRate(speed)}>
            {speed}x
          </button>
        ))}
      </div>
      <div className="commands">
        <button type="button" onClick={() => props.onCommand("repeat")}>Repeat</button>
        <button type="button" onClick={() => props.onCommand("skip")}>Skip</button>
        {view.cardMode ? (
          <>
            <button type="button" onClick={() => props.onCommand("know")}>Know it</button>
            <button type="button" onClick={() => props.onCommand("dontknow")}>Don't know</button>
          </>
        ) : (
          <>
            <button type="button" onClick={() => props.onCommand("explain")}>Explain</button>
            <button type="button" onClick={() => props.onCommand("dontknow")}>Don't know</button>
            <button type="button" aria-pressed={view.guessing} onClick={props.onGuess}>
              {view.guessing ? "Guessing: on" : "I guessed"}
            </button>
          </>
        )}
      </div>
      <p className="muted">Score {view.correct} right, {view.wrong} missed. Next on the car remote skips. Previous repeats.</p>
    </section>
  );
}

function formatClock(total: number): string {
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}
