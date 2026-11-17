mod engine;

use std::collections::{HashMap, HashSet};

use drive_study_core::{
    build_drive, build_lesson, build_mistakes, build_mock, build_quiz, build_rapid, readiness, streak_from_dates, weak_topics, AudioManifest, ExamId, Flashcard, Lesson, Meta, PlanId, Question, ReadinessAttempt, SectionBlueprint, VOICE_ID,
};
use engine::{arm_audio, download_urls, delete_urls, export_progress, import_progress, is_due_card, load_state, register_service_worker, run_session, save_settings, today_key, Act, Control, DownloadProgress, PlayerModel, ProgressFile, Settings, Summary};
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

#[derive(Clone)]
struct Shelf {
    meta: Meta,
    index: ContentIndex,
    lessons: Vec<Lesson>,
    questions: Vec<Question>,
    cards: Vec<Flashcard>,
    loaded: HashSet<String>,
}

#[derive(Clone, serde::Deserialize)]
struct ContentIndex {
    lessons: Vec<String>,
    questions: Vec<String>,
    flashcards: Vec<String>,
    #[serde(default)]
    counts: HashMap<String, SectionCounts>,
}

#[derive(Clone, Default, serde::Deserialize)]
struct SectionCounts {
    questions: usize,
    flashcards: usize,
    lessons: usize,
}

#[component]
fn App() -> impl IntoView {
    let shelf = RwSignal::new(None::<Shelf>);
    let manifest = RwSignal::new(AudioManifest {
        version: 1,
        algorithm: String::new(),
        voice_id: VOICE_ID.into(),
        clips: Default::default(),
    });
    let settings = RwSignal::new(Settings::default());
    let cards = RwSignal::new(Vec::<drive_study_core::SrsCard>::new());
    let attempts = RwSignal::new(Vec::<engine::AttemptRecord>::new());
    let days = RwSignal::new(Vec::<String>::new());
    let route = RwSignal::new(normalize_hash());
    let plan = RwSignal::new(None::<drive_study_core::SessionPlan>);
    let summary = RwSignal::new(None::<Summary>);
    let error = RwSignal::new(String::new());
    let loading = RwSignal::new(String::new());
    LOADING.with(|slot| slot.set(Some(loading)));

    register_service_worker();
    if let Some(window) = web_sys::window() {
        let route_for_event = route;
        let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move || route_for_event.set(normalize_hash())) as Box<dyn FnMut()>);
        let _ = window.add_event_listener_with_callback("hashchange", closure.as_ref().unchecked_ref());
        closure.forget();
    }
    leptos::task::spawn_local(async move {
        let (saved_settings, saved_cards, saved_attempts, saved_days) = load_state().await;
        settings.set(saved_settings);
        cards.set(saved_cards);
        attempts.set(saved_attempts);
        days.set(saved_days);
        match load_library().await {
            Ok((loaded, audio)) => {
                shelf.set(Some(loaded));
                manifest.set(audio);
            }
            Err(message) => error.set(message),
        }
    });

    let listen_only = Signal::derive(move || settings.get().answer_mode == "listen");

    view! {
        <main class="shell">
            <Show when=move || route.get() != "/play">
                <header class="top">
                    <button class="brand" type="button" on:click=move |_| go("/")>
                        <b>"DRIVE"</b>
                        <span>"Study"</span>
                    </button>
                    <button class="icon-btn" type="button" data-testid="open-settings" on:pointerup=move |_| tap(true, || go("/settings")) on:click=move |_| tap(false, || go("/settings"))>"Set"</button>
                </header>
            </Show>
            <Show when=move || shelf.get().is_none() && error.get().is_empty()>
                <h1>"Drive Study"</h1>
                <p class="lede">"Loading the question files."</p>
            </Show>
            <Show when=move || !error.get().is_empty() && shelf.get().is_none()>
                <h1>"Drive Study"</h1>
                <p>{move || error.get()}</p>
            </Show>
            <Show when=move || shelf.get().is_some() && route.get() == "/">
                <Home shelf=shelf settings=settings cards=cards attempts=attempts days=days plan=plan />
            </Show>
            <Show when=move || route.get() == "/lessons">
                <Lessons shelf=shelf settings=settings />
            </Show>
            <Show when=move || route.get().starts_with("/lesson/")>
                <LessonPage shelf=shelf settings=settings plan=plan route=route />
            </Show>
            <Show when=move || route.get() == "/quiz">
                <Quiz shelf=shelf settings=settings plan=plan />
            </Show>
            <Show when=move || route.get() == "/rapid">
                <Rapid shelf=shelf settings=settings plan=plan />
            </Show>
            <Show when=move || route.get() == "/mock">
                <Mock shelf=shelf settings=settings plan=plan />
            </Show>
            <Show when=move || route.get() == "/mistakes">
                <Mistakes shelf=shelf settings=settings cards=cards plan=plan />
            </Show>
            <Show when=move || route.get() == "/stats">
                <Stats shelf=shelf attempts=attempts days=days />
            </Show>
            <Show when=move || route.get() == "/settings">
                <SettingsScreen shelf=shelf manifest=manifest settings=settings cards=cards attempts=attempts days=days />
            </Show>
            <Show when=move || route.get() == "/play">
                <Play plan=plan settings=settings cards=cards attempts=attempts days=days manifest=manifest summary=summary listen_only=listen_only />
            </Show>
            <Show when=move || route.get() == "/summary">
                <SummaryView summary=summary />
            </Show>
            {move || (!loading.get().is_empty()).then(|| view! { <p class="muted">{loading.get()}</p> })}
            {move || (!error.get().is_empty() && shelf.get().is_some()).then(|| view! { <p class="bad">{error.get()}</p> })}
            <Show when=move || route.get() != "/play" && shelf.get().is_some()>
                <p class="footer">{move || shelf.get().map(|item| item.meta.disclaimer).unwrap_or_default()}</p>
            </Show>
        </main>
    }
}

#[component]
fn Home(shelf: RwSignal<Option<Shelf>>, settings: RwSignal<Settings>, cards: RwSignal<Vec<drive_study_core::SrsCard>>, attempts: RwSignal<Vec<engine::AttemptRecord>>, days: RwSignal<Vec<String>>, plan: RwSignal<Option<drive_study_core::SessionPlan>>) -> impl IntoView {
    let due = Signal::derive(move || cards.get().iter().filter(|card| is_due_card(card)).count());
    let streak = Signal::derive(move || streak_from_dates(&days.get(), &today_key()));
    view! {
        <section>
            <h1 data-testid="home-title">"Drive Study"</h1>
            <p class="lede">{move || format!("SIE and Series 7, out loud. {} due. {} day streak.", due.get(), streak.get())}</p>
            <div class="seg">
                <button type="button" aria-pressed=move || settings.get().plan == "SIE" on:click=move |_| set_plan(settings, "SIE")>"SIE"</button>
                <button type="button" aria-pressed=move || settings.get().plan == "S7" on:click=move |_| set_plan(settings, "S7")>"Series 7"</button>
                <button type="button" aria-pressed=move || settings.get().plan == "BOTH" on:click=move |_| set_plan(settings, "BOTH")>"Both"</button>
            </div>
            <button class="big" type="button" data-testid="drive-start" on:click=move |_| {
                let Some(loaded) = shelf.get_untracked() else { return };
                let prefs = settings.get_untracked();
                let sections = section_ids(&loaded, parse_plan(&prefs.plan));
                let due_ids: HashSet<String> = cards.get_untracked().into_iter().filter(is_due_card).map(|card| card.id).collect();
                let weak = weak_section_ids(&attempts.get_untracked());
                begin_session(shelf, sections, &["questions", "flashcards", "lessons"], "Loading the bank…", move |loaded| {
                    let prefs = settings.get_untracked();
                    plan.set(Some(build_drive(parse_plan(&prefs.plan), &loaded.questions, &loaded.cards, &loaded.lessons, &due_ids, &weak, &prefs.completed_lessons, prefs.answer_mode == "listen", &mut js_random)));
                    go("/play");
                });
            }>"Drive, 20 minutes"</button>
            <div class="grid">
                <Mode name="Lessons" detail=Signal::derive(move || shelf.get().map(|item| format!("{} lessons", count_field(&item, |row| row.lessons))).unwrap_or_default()) hash="/lessons" testid="mode-lessons" />
                <Mode name="Quiz" detail=Signal::derive(|| "10, 20, or 30".into()) hash="/quiz" testid="mode-quiz" />
                <Mode name="Rapid fire" detail=Signal::derive(|| "Terms, short".into()) hash="/rapid" testid="mode-rapid" />
                <Mode name="Mock exam" detail=Signal::derive(|| "Weighted, timed or car".into()) hash="/mock" testid="mode-mock" />
                <Mode name="Mistakes" detail=Signal::derive(move || format!("{} due", due.get())) hash="/mistakes" testid="mode-mistakes" />
                <Mode name="Stats" detail=Signal::derive(|| "Readiness".into()) hash="/stats" testid="mode-stats" />
            </div>
            <div class="banner">"Practice questions, not official FINRA items. Saying answers needs a network connection on an iPhone. The buttons always work."</div>
        </section>
    }
}

fn weak_section_ids(attempts: &[engine::AttemptRecord]) -> Vec<String> {
    let mut rows: Vec<(String, usize, usize)> = Vec::new();
    for attempt in attempts.iter().filter(|attempt| attempt.kind == drive_study_core::ItemKind::Question) {
        if let Some(row) = rows.iter_mut().find(|row| row.0 == attempt.section) {
            row.2 += 1;
            if attempt.correct {
                row.1 += 1;
            }
        } else {
            rows.push((attempt.section.clone(), usize::from(attempt.correct), 1));
        }
    }
    rows.retain(|row| row.2 > 0 && (row.1 as f64 / row.2 as f64) < 0.7);
    rows.sort_by(|a, b| (a.1 as f64 / a.2 as f64).partial_cmp(&(b.1 as f64 / b.2 as f64)).unwrap_or(std::cmp::Ordering::Equal));
    rows.into_iter().map(|row| row.0).collect()
}

#[component]
fn Mode(name: &'static str, detail: Signal<String>, hash: &'static str, testid: &'static str) -> impl IntoView {
    view! {
        <button class="mode" type="button" data-testid=testid on:click=move |_| go(hash)>
            <b>{name}</b>
            <small>{move || detail.get()}</small>
        </button>
    }
}

#[component]
fn Lessons(shelf: RwSignal<Option<Shelf>>, settings: RwSignal<Settings>) -> impl IntoView {
    Effect::new(move |_| {
        let Some(loaded) = shelf.get() else { return };
        let sections = section_ids(&loaded, PlanId::Both);
        if sections.iter().all(|id| loaded.loaded.contains(&format!("{id}:lessons"))) {
            return;
        }
        load_then(shelf, sections, &["lessons"], "Loading lessons…", |_| {});
    });
    view! {
        <section>
            <h1>"Lessons"</h1>
            <p class="lede">"SIE foundations come first when you are studying both exams."</p>
            <div class="stack">
                {move || {
                    let Some(shelf) = shelf.get() else { return Vec::new() };
                    let plan = parse_plan(&settings.get().plan);
                    shelf.lessons.iter().filter(|lesson| plan == PlanId::Both || exam_plan(lesson.exam) == plan).map(|lesson| {
                        let id = lesson.id.clone();
                        let title = lesson.title.clone();
                        let detail = format!("{} · about {} min", lesson.section_title, lesson.est_minutes);
                        view! { <button class="text-btn list-btn" type="button" on:click=move |_| go(&format!("/lesson/{id}"))><b>{title}</b><small>{detail}</small></button> }
                    }).collect::<Vec<_>>()
                }}
            </div>
        </section>
    }
}

#[component]
fn LessonPage(shelf: RwSignal<Option<Shelf>>, settings: RwSignal<Settings>, plan: RwSignal<Option<drive_study_core::SessionPlan>>, route: RwSignal<String>) -> impl IntoView {
    Effect::new(move |_| {
        let Some(loaded) = shelf.get() else { return };
        let sections = section_ids(&loaded, PlanId::Both);
        if sections.iter().all(|id| loaded.loaded.contains(&format!("{id}:lessons"))) {
            return;
        }
        load_then(shelf, sections, &["lessons"], "Loading lessons…", |_| {});
    });
    view! {
        {move || {
            let id = route.get().trim_start_matches("/lesson/").to_string();
            let Some(loaded) = shelf.get() else { return view! { <p>"Loading."</p> }.into_any() };
            let Some(lesson) = loaded.lessons.iter().find(|lesson| lesson.id == id).cloned() else {
                let ready = section_ids(&loaded, PlanId::Both).iter().all(|section| loaded.loaded.contains(&format!("{section}:lessons")));
                return if ready {
                    view! { <p>"That lesson is not in the library."</p> }.into_any()
                } else {
                    view! { <p>"Loading the lesson."</p> }.into_any()
                };
            };
            let title = lesson.title.clone();
            let summary = lesson.summary.clone();
            let kicker = lesson.section_title.clone();
            let terms = lesson.key_terms.clone();
            let start = lesson.clone();
            view! {
                <section>
                    <p class="kicker">{kicker}</p>
                    <h1>{title}</h1>
                    <p>{summary}</p>
                    <div class="stack">
                        {terms.into_iter().map(|term| view! { <div class="card"><b>{term.term}</b><p>{term.definition}</p></div> }).collect::<Vec<_>>()}
                    </div>
                    <button class="big" type="button" on:click=move |_| {
                        let section = start.section.clone();
                        let lesson = start.clone();
                        begin_session(shelf, vec![section], &["questions"], "Loading the checks…", move |loaded| {
                            plan.set(Some(build_lesson(&lesson, &loaded.questions, settings.get_untracked().answer_mode == "listen")));
                            go("/play");
                        });
                    }>"Play lesson"</button>
                </section>
            }.into_any()
        }}
    }
}

#[component]
fn Quiz(shelf: RwSignal<Option<Shelf>>, settings: RwSignal<Settings>, plan: RwSignal<Option<drive_study_core::SessionPlan>>) -> impl IntoView {
    let exam = RwSignal::new(settings.get_untracked().plan);
    let section = RwSignal::new(String::new());
    let count = RwSignal::new(10usize);
    view! {
        <section>
            <h1>"Quiz"</h1>
            <div class="seg">
                <button type="button" aria-pressed=move || exam.get() == "SIE" on:click=move |_| exam.set("SIE".into())>"SIE"</button>
                <button type="button" aria-pressed=move || exam.get() == "S7" on:click=move |_| exam.set("S7".into())>"Series 7"</button>
                <button type="button" aria-pressed=move || exam.get() == "BOTH" on:click=move |_| exam.set("BOTH".into())>"Both"</button>
            </div>
            <Show when=move || exam.get() != "BOTH">
                <label class="field">"Section"
                    <select on:change=move |event| section.set(event_target_value(&event))>
                        <option value="">"All sections, weighted"</option>
                        {move || section_options(&shelf.get(), &exam.get())}
                    </select>
                </label>
            </Show>
            <CountPick count=count options=vec![10, 20, 30] />
            <button class="big" type="button" data-testid="start-quiz" on:click=move |_| {
                let Some(loaded) = shelf.get_untracked() else { return };
                let chosen = parse_plan(&exam.get_untracked());
                let section_id = section.get_untracked();
                let needed = if section_id.is_empty() { section_ids(&loaded, chosen) } else { vec![section_id.clone()] };
                let total = count.get_untracked();
                begin_session(shelf, needed, &["questions"], "Loading questions…", move |loaded| {
                    let sections = sections_for(loaded, chosen);
                    let section_ref = if section_id.is_empty() { None } else { Some(section_id.as_str()) };
                    plan.set(Some(build_quiz(chosen, section_ref, total, &loaded.questions, &sections, settings.get_untracked().answer_mode == "listen", &mut js_random)));
                    go("/play");
                });
            }>"Start quiz"</button>
        </section>
    }
}

#[component]
fn Rapid(shelf: RwSignal<Option<Shelf>>, settings: RwSignal<Settings>, plan: RwSignal<Option<drive_study_core::SessionPlan>>) -> impl IntoView {
    let exam = RwSignal::new(settings.get_untracked().plan);
    let count = RwSignal::new(10usize);
    view! {
        <section>
            <h1>"Rapid fire"</h1>
            <p class="lede">{move || shelf.get().map(|item| format!("{} terms. The term, a pause, then the definition.", count_field(&item, |row| row.flashcards))).unwrap_or_else(|| "The term, a pause, then the definition.".into())}</p>
            <div class="seg">
                <button type="button" on:click=move |_| exam.set("SIE".into())>"SIE"</button>
                <button type="button" on:click=move |_| exam.set("S7".into())>"Series 7"</button>
                <button type="button" on:click=move |_| exam.set("BOTH".into())>"Both"</button>
            </div>
            <CountPick count=count options=vec![10, 20] />
            <button class="big" type="button" on:click=move |_| {
                let Some(loaded) = shelf.get_untracked() else { return };
                let chosen = parse_plan(&exam.get_untracked());
                let needed = section_ids(&loaded, chosen);
                let total = count.get_untracked();
                begin_session(shelf, needed, &["flashcards"], "Loading terms…", move |loaded| {
                    plan.set(Some(build_rapid(chosen, None, total, &loaded.cards, settings.get_untracked().answer_mode == "listen", &mut js_random)));
                    go("/play");
                });
            }>"Start rapid fire"</button>
        </section>
    }
}

#[component]
fn Mock(shelf: RwSignal<Option<Shelf>>, settings: RwSignal<Settings>, plan: RwSignal<Option<drive_study_core::SessionPlan>>) -> impl IntoView {
    let exam = RwSignal::new(if settings.get_untracked().plan == "S7" { "S7".to_string() } else { "SIE".to_string() });
    view! {
        <section>
            <h1>"Mock exam"</h1>
            <p class="lede">{move || mock_blurb(&shelf.get(), &exam.get())}</p>
            <div class="seg">
                <button type="button" aria-pressed=move || exam.get() == "SIE" on:click=move |_| exam.set("SIE".into())>"SIE"</button>
                <button type="button" aria-pressed=move || exam.get() == "S7" on:click=move |_| exam.set("S7".into())>"Series 7"</button>
            </div>
            <div class="stack">
                <button class="big" type="button" on:click=move |_| start_mock(shelf, settings, plan, &exam.get(), true)>"Timed"</button>
                <button class="big secondary" type="button" on:click=move |_| start_mock(shelf, settings, plan, &exam.get(), false)>"Car mode, no clock"</button>
            </div>
        </section>
    }
}

fn start_mock(shelf: RwSignal<Option<Shelf>>, settings: RwSignal<Settings>, plan: RwSignal<Option<drive_study_core::SessionPlan>>, exam: &str, timed: bool) {
    let Some(loaded) = shelf.get_untracked() else { return };
    let exam_id = if exam == "S7" { ExamId::S7 } else { ExamId::Sie };
    let needed = section_ids(&loaded, if exam_id == ExamId::S7 { PlanId::S7 } else { PlanId::Sie });
    begin_session(shelf, needed, &["questions"], "Loading the mock…", move |loaded| {
        let Some(blueprint) = loaded.meta.exams.get(exam_id.as_str()).cloned() else { return };
        plan.set(Some(build_mock(exam_id, timed, &loaded.questions, &blueprint, settings.get_untracked().answer_mode == "listen", &mut js_random)));
        go("/play");
    });
}

#[component]
fn Mistakes(shelf: RwSignal<Option<Shelf>>, settings: RwSignal<Settings>, cards: RwSignal<Vec<drive_study_core::SrsCard>>, plan: RwSignal<Option<drive_study_core::SessionPlan>>) -> impl IntoView {
    view! {
        <section>
            <h1>"Mistakes"</h1>
            <p class="lede">"Wrong answers and guesses land here. An item graduates after you get it right on three different days."</p>
            {move || {
                let due: Vec<_> = cards.get().into_iter().filter(is_due_card).collect();
                let waiting: Vec<_> = cards.get().into_iter().filter(|card| !card.graduated && !is_due_card(card)).collect();
                let graduated = cards.get().iter().filter(|card| card.graduated).count();
                let waiting_note = if waiting.is_empty() {
                    String::new()
                } else {
                    let soon = waiting.iter().map(|card| card.due).min().unwrap_or(0);
                    let minutes = ((soon - js_sys::Date::now() as i64).max(0) / 60_000).max(1);
                    format!("{} more will be due later. The next one is in about {minutes} min.", waiting.len())
                };
                if due.is_empty() {
                    view! { <p data-testid="mistakes-empty">{format!("{graduated} graduated. Nothing is due right now. {waiting_note}")}</p> }.into_any()
                } else {
                    let count = due.len();
                    let note = waiting_note.clone();
                    view! { <div>
                        <button class="big" type="button" data-testid="mistakes-start" on:click=move |_| {
                            let due: Vec<_> = cards.get_untracked().into_iter().filter(is_due_card).collect();
                            let mut needed: Vec<String> = due.iter().map(|card| card.section.clone()).collect();
                            needed.sort();
                            needed.dedup();
                            let ids: HashSet<String> = due.into_iter().map(|card| card.id).collect();
                            begin_session(shelf, needed, &["questions", "flashcards"], "Loading mistakes…", move |loaded| {
                                plan.set(Some(build_mistakes(parse_plan(&settings.get_untracked().plan), &loaded.questions, &loaded.cards, &ids, settings.get_untracked().answer_mode == "listen")));
                                go("/play");
                            });
                        }>{format!("Start mistakes drive ({count})")}</button>
                        <p>{note}</p>
                    </div> }.into_any()
                }
            }}
        </section>
    }
}

#[component]
fn Stats(shelf: RwSignal<Option<Shelf>>, attempts: RwSignal<Vec<engine::AttemptRecord>>, days: RwSignal<Vec<String>>) -> impl IntoView {
    view! {
        <section>
            <h1>"Stats"</h1>
            <p class="lede" data-testid="stats-count">{move || format!("{} day streak. {} answers saved on this phone.", streak_from_dates(&days.get(), &today_key()), attempts.get().len())}</p>
            {move || stats_cards(&shelf.get(), &attempts.get())}
        </section>
    }
}

fn stats_cards(shelf: &Option<Shelf>, attempts: &[engine::AttemptRecord]) -> Vec<impl IntoView> {
    let Some(shelf) = shelf else { return Vec::new() };
    ["SIE", "S7"].into_iter().map(|exam| {
        let sections = shelf.meta.exams.get(exam).map(|item| item.sections.clone()).unwrap_or_default();
        let weights: Vec<(String, f64)> = sections.iter().map(|section| (section.id.clone(), section.weight)).collect();
        let exam_id = if exam == "S7" { ExamId::S7 } else { ExamId::Sie };
        let rows: Vec<ReadinessAttempt> = attempts.iter().filter(|attempt| attempt.exam == exam_id && attempt.kind == drive_study_core::ItemKind::Question).map(|attempt| ReadinessAttempt {
            section: attempt.section.clone(),
            topic: attempt.topic.clone(),
            correct: attempt.correct,
        }).collect();
        let seen = rows.len();
        let pool = count_exam(shelf, exam, |row| row.questions);
        let ready = readiness(&weights, &rows, seen, pool, 3);
        let topics = weak_topics(&rows, 2);
        let title = if exam == "S7" { "Series 7" } else { "SIE" };
        let score = ready.score.map(|score| score.to_string()).unwrap_or_else(|| "—".into());
        let note = if ready.score.is_none() { "Answer at least 3 questions in a section.".into() } else { format!("Library coverage {}%.", (ready.coverage * 100.0).round()) };
        let bars = sections.into_iter().map(|section| {
            let matching: Vec<_> = rows.iter().filter(|row| row.section == section.id).collect();
            let accuracy = if matching.is_empty() { 0.0 } else { matching.iter().filter(|row| row.correct).count() as f64 / matching.len() as f64 };
            let label = if matching.is_empty() { "—".into() } else { format!("{}%", (accuracy * 100.0).round()) };
            let width = format!("{}%", accuracy * 100.0);
            view! {
                <div class="meter">
                    <div class="spread"><span>{section.title}</span><span>{label}</span></div>
                    <div class="bar"><span style:width=width></span></div>
                </div>
            }
        }).collect::<Vec<_>>();
        let weak = if topics.is_empty() { String::new() } else { format!("Weak topics: {}.", topics.iter().map(|topic| topic.topic.as_str()).collect::<Vec<_>>().join(", ")) };
        view! {
            <article class="card">
                <h2>{title}</h2>
                <p class="prompt">{score}</p>
                <p class="muted">{note}</p>
                {bars}
                <p>{weak}</p>
            </article>
        }
    }).collect()
}

#[component]
fn SettingsScreen(
    shelf: RwSignal<Option<Shelf>>,
    manifest: RwSignal<AudioManifest>,
    settings: RwSignal<Settings>,
    cards: RwSignal<Vec<drive_study_core::SrsCard>>,
    attempts: RwSignal<Vec<engine::AttemptRecord>>,
    days: RwSignal<Vec<String>>,
) -> impl IntoView {
    let voices = RwSignal::new(Vec::<(String, String)>::new());
    let storage = RwSignal::new(String::new());
    let job = RwSignal::new(DownloadProgress::default());
    leptos::task::spawn_local(async move {
        voices.set(load_voices());
        storage.set(storage_line().await);
    });
    view! {
        <section>
            <h1>"Settings"</h1>
            <h2>"Answer mode"</h2>
            <div class="seg">
                <button type="button" data-testid="answer-spoken" aria-pressed=move || if settings.get().answer_mode == "spoken" { "true" } else { "false" } on:pointerup=move |_| tap(true, || update_settings(settings, |item| item.answer_mode = "spoken".into())) on:click=move |_| tap(false, || update_settings(settings, |item| item.answer_mode = "spoken".into()))>"Spoken answers"</button>
                <button type="button" data-testid="answer-listen" aria-pressed=move || if settings.get().answer_mode == "listen" { "true" } else { "false" } on:pointerup=move |_| tap(true, || update_settings(settings, |item| item.answer_mode = "listen".into())) on:click=move |_| tap(false, || update_settings(settings, |item| item.answer_mode = "listen".into()))>"Listen only"</button>
            </div>
            <p class="muted">"Listen only reads the question, waits, then reads the answer. Spoken mode listens for A, B, C, D, alpha, bravo, charlie, delta, true, false, repeat, skip, next, wait, explain, I don't know, pause, and stop."</p>
            <h2>"Explain correct answers"</h2>
            <div class="seg">
                <button type="button" data-testid="explain-always" aria-pressed=move || if settings.get().explain_correct == "always" { "true" } else { "false" } on:pointerup=move |_| tap(true, || update_settings(settings, |item| item.explain_correct = "always".into())) on:click=move |_| tap(false, || update_settings(settings, |item| item.explain_correct = "always".into()))>"Always"</button>
                <button type="button" data-testid="explain-short" aria-pressed=move || if settings.get().explain_correct == "short" { "true" } else { "false" } on:pointerup=move |_| tap(true, || update_settings(settings, |item| item.explain_correct = "short".into())) on:click=move |_| tap(false, || update_settings(settings, |item| item.explain_correct = "short".into()))>"Short"</button>
                <button type="button" data-testid="explain-never" aria-pressed=move || if settings.get().explain_correct == "never" { "true" } else { "false" } on:pointerup=move |_| tap(true, || update_settings(settings, |item| item.explain_correct = "never".into())) on:click=move |_| tap(false, || update_settings(settings, |item| item.explain_correct = "never".into()))>"Never"</button>
            </div>
            <p class="muted">"Short is one line, and it is the default. Wrong answers still get the full explanation. Say next or skip, tap Next, or press the car's next-track button to cut an explanation off."</p>
            <h2>"Voice"</h2>
            <div class="stack">
                <button class="text-btn" type="button" data-testid="voice-downloaded" aria-pressed=move || if settings.get().voice_source == "downloaded" { "true" } else { "false" } on:pointerup=move |_| tap(true, || update_settings(settings, |item| item.voice_source = "downloaded".into())) on:click=move |_| tap(false, || update_settings(settings, |item| item.voice_source = "downloaded".into()))>
                    {move || (settings.get().voice_source == "downloaded").then(|| view! { <span class="pill">"Selected"</span> })}
                    <b>"Downloaded natural audio"</b>
                    <small>"Saved on the phone. If a clip is missing, the app streams it, then uses the phone voice."</small>
                </button>
                <button class="text-btn" type="button" data-testid="voice-streamed" aria-pressed=move || if settings.get().voice_source == "streamed" { "true" } else { "false" } on:pointerup=move |_| tap(true, || update_settings(settings, |item| item.voice_source = "streamed".into())) on:click=move |_| tap(false, || update_settings(settings, |item| item.voice_source = "streamed".into()))>
                    {move || (settings.get().voice_source == "streamed").then(|| view! { <span class="pill">"Selected"</span> })}
                    <b>"Streamed natural audio"</b>
                    <small>"Plays from the site while you are online and is not kept unless you download it."</small>
                </button>
                <button class="text-btn" type="button" data-testid="voice-phone" aria-pressed=move || if settings.get().voice_source == "phone" { "true" } else { "false" } on:pointerup=move |_| tap(true, || update_settings(settings, |item| item.voice_source = "phone".into())) on:click=move |_| tap(false, || update_settings(settings, |item| item.voice_source = "phone".into()))>
                    {move || (settings.get().voice_source == "phone").then(|| view! { <span class="pill">"Selected"</span> })}
                    <b>"Phone voice"</b>
                    <small>"The iPhone voice. Works offline. It often stops when the screen locks."</small>
                </button>
            </div>
            <label class="field" data-testid="rate-label">{move || format!("Speed {:.1}x, for every voice", settings.get().rate)}
                <input type="range" data-testid="rate" min="0.8" max="2" step="0.1" prop:value=move || settings.get().rate on:input=move |event| {
                    let value = event_target_value(&event).parse().unwrap_or(1.0);
                    update_settings(settings, |item| item.rate = value);
                } />
            </label>
            <label class="field" data-testid="answer-seconds-label">{move || format!("Seconds to answer: {}. Spoken mode adds 2.", settings.get().answer_seconds)}
                <input type="range" data-testid="answer-seconds" min="6" max="20" step="1" prop:value=move || settings.get().answer_seconds on:input=move |event| {
                    let value = event_target_value(&event).parse().unwrap_or(10);
                    update_settings(settings, |item| item.answer_seconds = value);
                } />
            </label>
            <label class="field">"Phone voice"
                <select data-testid="voice-select" on:change=move |event| {
                    let value = event_target_value(&event);
                    update_settings(settings, |item| item.voice_uri = if value.is_empty() { None } else { Some(value) });
                }>
                    <option value="">"System default"</option>
                    {move || voices.get().into_iter().map(|(uri, name)| view! { <option value=uri>{name}</option> }).collect::<Vec<_>>()}
                </select>
            </label>
            <h2>"Downloads"</h2>
            <p class="muted">{move || format!("{} Published clips: {}.", storage.get(), manifest.get().clips.len())}</p>
            {move || {
                let current = job.get();
                (current.total > 0).then(|| {
                    let width = format!("{}%", if current.total == 0 { 0.0 } else { current.done as f64 / current.total as f64 * 100.0 });
                    let line = format!("{} / {} clips · {:.1} MB", current.done, current.total, current.bytes as f64 / 1_048_576.0);
                    view! {
                        <div class="card" data-testid="download-progress">
                            <div class="spread"><b>{current.label.clone()}</b><span>{line}</span></div>
                            <div class="bar"><span style:width=width></span></div>
                            <p class=if current.failed == 0 { "muted" } else { "bad" }>{current.message.clone()}</p>
                        </div>
                    }
                })
            }}
            <button class="big secondary" type="button" data-testid="download-all" on:click=move |_| start_download(manifest, job, storage, None)>"Download all sections"</button>
            {move || download_rows(shelf, manifest, job, storage)}
            <h2>"Progress"</h2>
            <div class="row">
                <button type="button" on:click=move |_| {
                    let settings = settings.get();
                    leptos::task::spawn_local(async move {
                        let file = export_progress(&settings).await;
                        download_json(&file);
                    });
                }>"Export"</button>
                <label class="text-btn">"Import"
                    <input type="file" accept="application/json" on:change=move |event| {
                        let input = event.target().and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok());
                        let Some(input) = input else { return };
                        let Some(file) = input.files().and_then(|files| files.get(0)) else { return };
                        leptos::task::spawn_local(async move {
                            let text = match JsFuture::from(file.text()).await.ok().and_then(|value| value.as_string()) {
                                Some(text) => text,
                                None => return,
                            };
                            let Ok(parsed) = serde_json::from_str::<ProgressFile>(&text) else {
                                let _ = web_sys::window().unwrap().alert_with_message("Could not read that file.");
                                return;
                            };
                            if !web_sys::window().unwrap().confirm_with_message("Replace the progress on this phone with this file?").unwrap_or(false) {
                                return;
                            }
                            if let Err(message) = import_progress(&parsed).await {
                                let _ = web_sys::window().unwrap().alert_with_message(&message);
                                return;
                            }
                            let (saved_settings, saved_cards, saved_attempts, saved_days) = load_state().await;
                            settings.set(saved_settings);
                            cards.set(saved_cards);
                            attempts.set(saved_attempts);
                            days.set(saved_days);
                        });
                    } />
                </label>
            </div>
            <h2>"On an iPhone"</h2>
            <p>"Install from Safari: Share, then Add to Home Screen. Speech recognition needs a network connection and the microphone. The phone voice usually stops when the screen locks. Downloaded or streamed audio keeps going more often. Wake lock only applies while this page is on screen."</p>
            <p class="muted">"To pick up an update, force-quit the app and open it again while you are online. If it still looks old, remove the home-screen icon, reload the site in Safari, and add it again."</p>
            <p><a class="big" href="podcast://mttmin.github.io/drive-study/podcast/feed.xml">"Subscribe in Apple Podcasts"</a></p>
            <p class="muted">"That one tap downloads lessons and quizzes for the locked screen. The feed is also at "<a href="podcast/feed.xml">"podcast/feed.xml"</a>"."</p>
        </section>
    }
}

fn download_rows(shelf: RwSignal<Option<Shelf>>, manifest: RwSignal<AudioManifest>, job: RwSignal<DownloadProgress>, storage: RwSignal<String>) -> Vec<impl IntoView> {
    let mut titles = HashMap::new();
    if let Some(loaded) = shelf.get() {
        for exam in loaded.meta.exams.values() {
            for section in &exam.sections {
                titles.insert(section.id.clone(), section.title.clone());
            }
        }
    }
    published_sections(&manifest.get()).into_iter().map(|(id, clips)| {
        let title = titles.get(&id).cloned().unwrap_or_else(|| id.clone());
        let count = clips.len();
        let bytes: u64 = clips.iter().map(|(_, size)| size).sum();
        let label = format!("{count} clips · {:.0} MB", bytes as f64 / 1_048_576.0);
        let urls: Vec<String> = clips.iter().map(|(url, _)| url.clone()).collect();
        let section_id = id.clone();
        view! {
            <div class="card">
                <div class="spread"><b>{title}</b><span>{label}</span></div>
                <div class="row">
                    <button type="button" data-testid=format!("download-{section_id}") disabled=count == 0 on:click=move |_| start_download(manifest, job, storage, Some(section_id.clone()))>"Download"</button>
                    <button type="button" on:click=move |_| {
                        let urls = urls.clone();
                        leptos::task::spawn_local(async move { delete_urls(&urls).await; });
                    }>"Delete"</button>
                </div>
            </div>
        }
    }).collect()
}

fn published_sections(manifest: &AudioManifest) -> Vec<(String, Vec<(String, u64)>)> {
    let mut ids: Vec<String> = manifest.clips.values().filter_map(|clip| clip.section.clone()).collect();
    ids.sort();
    ids.dedup();
    ids.into_iter().map(|id| {
        let clips = section_clips(manifest, &id);
        (id, clips)
    }).collect()
}

fn section_clips(manifest: &AudioManifest, section: &str) -> Vec<(String, u64)> {
    let mut clips: Vec<_> = manifest.clips.values().filter(|clip| clip.section.as_deref() == Some(section)).map(|clip| (absolute(&clip.path), clip.bytes.unwrap_or(0))).collect();
    clips.sort();
    clips
}

fn start_download(manifest: RwSignal<AudioManifest>, job: RwSignal<DownloadProgress>, storage: RwSignal<String>, section: Option<String>) {
    let current = job.get_untracked();
    if current.total > 0 && !current.finished {
        return;
    }
    let audio = manifest.get_untracked();
    let (label, clips) = if let Some(section) = section {
        (section.clone(), section_clips(&audio, &section))
    } else {
        let mut clips: Vec<_> = audio.clips.values().map(|clip| (absolute(&clip.path), clip.bytes.unwrap_or(0))).collect();
        clips.sort();
        ("All sections".into(), clips)
    };
    let bytes: u64 = clips.iter().map(|(_, size)| size).sum();
    if !confirm_download(&label, clips.len(), bytes) {
        return;
    }
    leptos::task::spawn_local(async move {
        let _ = download_urls(&clips, &label, job).await;
        storage.set(storage_line().await);
    });
}

fn confirm_download(label: &str, count: usize, bytes: u64) -> bool {
    let mb = bytes as f64 / 1_048_576.0;
    if count < 80 && mb < 15.0 {
        return true;
    }
    web_sys::window()
        .and_then(|window| window.confirm_with_message(&format!("{label}: {count} clips, about {mb:.0} MB. Download on Wi-Fi? Clips already saved are skipped, so you can resume.")).ok())
        .unwrap_or(false)
}

fn absolute(path: &str) -> String {
    web_sys::window()
        .and_then(|window| window.location().href().ok())
        .and_then(|href| web_sys::Url::new_with_base(path, &href).ok())
        .map(|url| url.href())
        .unwrap_or_else(|| path.to_string())
}

#[component]
fn Play(
    plan: RwSignal<Option<drive_study_core::SessionPlan>>,
    settings: RwSignal<Settings>,
    cards: RwSignal<Vec<drive_study_core::SrsCard>>,
    attempts: RwSignal<Vec<engine::AttemptRecord>>,
    days: RwSignal<Vec<String>>,
    manifest: RwSignal<AudioManifest>,
    summary: RwSignal<Option<Summary>>,
    listen_only: Signal<bool>,
) -> impl IntoView {
    let model = RwSignal::new(PlayerModel::default());
    let control = Control::new();
    let halt = control.clone();
    on_cleanup(move || halt.halt());
    if let Some(session) = plan.get_untracked() {
        let control = control.clone();
        leptos::task::spawn_local(async move {
            run_session(session, model, control, settings, cards, attempts, days, manifest.get_untracked(), summary).await;
        });
    }
    let _ = listen_only;
    let repeat = control.clone();
    let skip = control.clone();
    let explain = control.clone();
    let dont_up = control.clone();
    let dont_click = control.clone();
    let guess = control.clone();
    let know = control.clone();
    let pause = control.clone();
    let stop = control.clone();
    let choices = control.clone();
    let window_control = choices.clone();
    let dismiss = skip.clone();
    view! {
        <section>
            <div class="player-top">
                <button class="icon-btn" type="button" on:click=move |_| stop.interrupt(Act::Stop)>"End"</button>
                <div>
                    <div class="kicker">{move || model.get().title}</div>
                    <div>{move || format!("{} / {}", model.get().index, model.get().total)}</div>
                </div>
                <button class="icon-btn" type="button" on:click=move |_| {
                    if model.get().paused { pause.resume(); } else { pause.interrupt(Act::Pause); }
                    model.update(|item| item.paused = !item.paused);
                }>{move || if model.get().paused { "Play" } else { "Pause" }}</button>
            </div>
            <p class="kicker" data-testid="source">{move || format!("{} {} · {}", model.get().kicker, if model.get().sample { "· Sample" } else { "" }, model.get().source)}</p>
            <h1 class="prompt">{move || model.get().prompt}</h1>
            <p class="status" data-testid="status" aria-live="polite">{move || format!("{} {} right, {} missed.", model.get().status, model.get().correct, model.get().wrong)}</p>
            {move || {
                let extend = window_control.clone();
                model.get().window_left.map(move |seconds| {
                    let extend = extend.clone();
                    let extend_click = extend.clone();
                    view! {
                        <button class="big secondary" type="button" data-testid="answer-window" on:pointerup=move |_| tap(true, || extend.interrupt(Act::MoreTime)) on:click=move |_| tap(false, || extend_click.interrupt(Act::MoreTime))>
                            {format!("{seconds}s to answer. Tap or say wait for more time.")}
                        </button>
                    }
                })
            }}
            <div class="choices">
                {move || model.get().choices.into_iter().map(|(key, text)| {
                    let control = choices.clone();
                    let answer = key.clone();
                    let test_id = format!("choice-{key}");
                    let label = key.clone();
                    view! {
                        <button class="choice-btn" type="button" data-testid=test_id on:click=move |_| control.interrupt(Act::Answer(answer.clone()))>
                            <b>{label}</b>
                            <span>{text}</span>
                        </button>
                    }
                }).collect::<Vec<_>>()}
            </div>
            {move || {
                let dismiss = dismiss.clone();
                model.get().answer_key.as_ref().map(|_| {
                    let shown = model.get();
                    let dismiss_up = dismiss.clone();
                    let dismiss_click = dismiss.clone();
                    view! {
                        <div class="card" data-testid="explanation" data-explain=shown.explain_style.clone()>
                            <p class="kicker">{format!("Answer {}", shown.answer_key.unwrap_or_default())}</p>
                            <p>{shown.explanation}</p>
                            {shown.yearly.then(|| view! { <p class="muted" data-testid="yearly-note">"Verify the current year. This figure changes."</p> })}
                            <button class="big" type="button" data-testid="next" on:pointerup=move |_| tap(true, || dismiss_up.interrupt(Act::Skip)) on:click=move |_| tap(false, || dismiss_click.interrupt(Act::Skip))>"Next"</button>
                        </div>
                    }
                })
            }}
            <div class="commands">
                <button type="button" on:click=move |_| repeat.interrupt(Act::Repeat)>"Repeat"</button>
                <button type="button" on:click=move |_| skip.interrupt(Act::Skip)>"Skip"</button>
                <button type="button" on:click=move |_| explain.interrupt(Act::Explain)>"Explain"</button>
                <button type="button" data-testid="dont-know" on:pointerup=move |_| tap(true, || dont_up.interrupt(Act::DontKnow)) on:click=move |_| tap(false, || dont_click.interrupt(Act::DontKnow))>"Don't know"</button>
                <button type="button" on:click=move |_| know.interrupt(Act::Know)>"Know it"</button>
                <button type="button" aria-pressed=move || model.get().guessing on:click=move |_| {
                    let next = !model.get_untracked().guessing;
                    guess.set_guessing(next);
                    model.update(|item| item.guessing = next);
                }>"I guessed"</button>
            </div>
        </section>
    }
}

#[component]
fn SummaryView(summary: RwSignal<Option<Summary>>) -> impl IntoView {
    view! {
        <section>
            <h1>{move || summary.get().map(|item| item.title).unwrap_or_else(|| "Session".into())}</h1>
            <p class="prompt" data-testid="summary-score">{move || summary.get().map(|item| format!("{}%", item.percent)).unwrap_or_default()}</p>
            <p>{move || summary.get().map(|item| format!("{} right, {} missed, {} skipped.", item.correct, item.wrong, item.skipped)).unwrap_or_default()}</p>
            <button class="big" type="button" on:click=move |_| go("/")>"Home"</button>
        </section>
    }
}

#[component]
fn CountPick(count: RwSignal<usize>, options: Vec<usize>) -> impl IntoView {
    view! {
        <div class="seg">
            {options.into_iter().map(|option| view! {
                <button type="button" aria-pressed=move || count.get() == option on:click=move |_| count.set(option)>{option}</button>
            }).collect::<Vec<_>>()}
        </div>
    }
}

fn section_options(shelf: &Option<Shelf>, exam: &str) -> Vec<impl IntoView> {
    let Some(shelf) = shelf else { return Vec::new() };
    let Some(blueprint) = shelf.meta.exams.get(exam) else { return Vec::new() };
    blueprint.sections.iter().map(|section| {
        let id = section.id.clone();
        let title = section.title.clone();
        view! { <option value=id>{title}</option> }
    }).collect()
}

fn sections_for(shelf: &Shelf, plan: PlanId) -> Vec<SectionBlueprint> {
    match plan {
        PlanId::Sie => shelf.meta.exams.get("SIE").map(|exam| exam.sections.clone()).unwrap_or_default(),
        PlanId::S7 => shelf.meta.exams.get("S7").map(|exam| exam.sections.clone()).unwrap_or_default(),
        PlanId::Both => Vec::new(),
    }
}

fn mock_blurb(shelf: &Option<Shelf>, exam: &str) -> String {
    let Some(shelf) = shelf else { return String::new() };
    let Some(blueprint) = shelf.meta.exams.get(exam) else { return String::new() };
    let have = count_exam(shelf, exam, |row| row.questions);
    format!(
        "{}: {} scored questions plus {} pretest, {} minutes, {}% to pass. The library has {have}.",
        blueprint.name, blueprint.scored_questions, blueprint.unscored_questions, blueprint.time_limit_minutes, blueprint.passing_score
    )
}

fn set_plan(settings: RwSignal<Settings>, plan: &str) {
    let plan = plan.to_string();
    update_settings(settings, move |item| item.plan = plan);
}

thread_local! {
    static LAST_TAP: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
}

fn tap(from_pointer: bool, action: impl FnOnce()) {
    let now = js_sys::Date::now();
    let skip = LAST_TAP.with(|slot| {
        let recent = now - slot.get() < 700.0;
        if from_pointer {
            slot.set(now);
            false
        } else {
            recent
        }
    });
    if !skip {
        action();
    }
}

fn update_settings(settings: RwSignal<Settings>, change: impl FnOnce(&mut Settings)) {
    settings.update(change);
    let snapshot = settings.get_untracked();
    leptos::task::spawn_local(async move { let _ = save_settings(&snapshot).await; });
}

fn parse_plan(plan: &str) -> PlanId {
    match plan {
        "S7" => PlanId::S7,
        "BOTH" => PlanId::Both,
        _ => PlanId::Sie,
    }
}

fn exam_plan(exam: ExamId) -> PlanId {
    match exam {
        ExamId::Sie => PlanId::Sie,
        ExamId::S7 => PlanId::S7,
    }
}

fn js_random() -> f64 {
    js_sys::Math::random()
}

fn go(hash: &str) {
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_hash(hash);
    }
}

fn normalize_hash() -> String {
    let hash = web_sys::window().and_then(|window| window.location().hash().ok()).unwrap_or_default();
    let hash = hash.trim_start_matches('#');
    if hash.is_empty() || hash == "/" { "/".into() } else { hash.to_string() }
}

fn load_voices() -> Vec<(String, String)> {
    let Some(synth) = web_sys::window().and_then(|window| window.speech_synthesis().ok()) else { return Vec::new() };
    let voices = synth.get_voices();
    let mut rows = Vec::new();
    for index in 0..voices.length() {
        if let Ok(voice) = voices.get(index).dyn_into::<web_sys::SpeechSynthesisVoice>() {
            rows.push((voice.voice_uri(), format!("{} ({})", voice.name(), voice.lang())));
        }
    }
    rows
}

async fn storage_line() -> String {
    let Some(window) = web_sys::window() else { return String::new() };
    let Ok(promise) = window.navigator().storage().estimate() else { return String::new() };
    let Ok(estimate) = JsFuture::from(promise).await else { return String::new() };
    let used = js_sys::Reflect::get(&estimate, &"usage".into()).ok().and_then(|value| value.as_f64()).unwrap_or(0.0);
    let quota = js_sys::Reflect::get(&estimate, &"quota".into()).ok().and_then(|value| value.as_f64()).unwrap_or(0.0);
    format!("{:.1} MB used of about {:.0} MB.", used / 1_048_576.0, quota / 1_048_576.0)
}

fn download_json(file: &ProgressFile) {
    let Ok(text) = serde_json::to_string_pretty(file) else { return };
    let blob = web_sys::Blob::new_with_str_sequence(&js_sys::Array::of1(&text.into())).ok();
    let Some(blob) = blob else { return };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else { return };
    if let Some(window) = web_sys::window() {
        let document = window.document().unwrap();
        if let Ok(link) = document.create_element("a") {
            let _ = link.set_attribute("href", &url);
            let _ = link.set_attribute("download", "drive-study-progress.json");
            let _ = link.dyn_ref::<web_sys::HtmlElement>().map(|element| element.click());
        }
    }
}

async fn load_library() -> Result<(Shelf, AudioManifest), String> {
    let meta: Meta = serde_json::from_str(&fetch_text("content/meta.json").await?).map_err(|err| err.to_string())?;
    let index: ContentIndex = serde_json::from_str(&fetch_text("content/index.json").await?).map_err(|err| err.to_string())?;
    let manifest = match fetch_text("audio/manifest.json").await {
        Ok(text) => serde_json::from_str(&text).unwrap_or(AudioManifest { version: 1, algorithm: String::new(), voice_id: VOICE_ID.into(), clips: Default::default() }),
        Err(_) => AudioManifest { version: 1, algorithm: String::new(), voice_id: VOICE_ID.into(), clips: Default::default() },
    };
    Ok((
        Shelf { meta, index, lessons: Vec::new(), questions: Vec::new(), cards: Vec::new(), loaded: HashSet::new() },
        manifest,
    ))
}

fn count_field(shelf: &Shelf, pick: fn(&SectionCounts) -> usize) -> usize {
    shelf.index.counts.values().map(pick).sum()
}

fn count_exam(shelf: &Shelf, exam: &str, pick: fn(&SectionCounts) -> usize) -> usize {
    shelf.index.counts.iter().filter(|(id, _)| id.starts_with(exam)).map(|(_, row)| pick(row)).sum()
}

fn section_ids(shelf: &Shelf, plan: PlanId) -> Vec<String> {
    let exams: &[&str] = match plan {
        PlanId::Sie => &["SIE"],
        PlanId::S7 => &["S7"],
        PlanId::Both => &["SIE", "S7"],
    };
    exams.iter().filter_map(|exam| shelf.meta.exams.get(*exam)).flat_map(|exam| exam.sections.iter().map(|section| section.id.clone())).collect()
}

fn section_of(path: &str) -> Option<String> {
    let name = path.rsplit('/').next()?.trim_end_matches(".json").to_ascii_lowercase();
    let mut parts = name.split('-');
    let exam = parts.next()?;
    let number = parts.next()?;
    if (exam != "sie" && exam != "s7") || !number.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some(format!("{}-{number}", exam.to_ascii_uppercase()))
}

thread_local! {
    static LOADING: std::cell::Cell<Option<RwSignal<String>>> = const { std::cell::Cell::new(None) };
    static BUSY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn begin_session(shelf: RwSignal<Option<Shelf>>, sections: Vec<String>, parts: &'static [&'static str], label: &'static str, build: impl FnOnce(&Shelf) + 'static) {
    arm_audio();
    load_then(shelf, sections, parts, label, build);
}

fn load_then(shelf: RwSignal<Option<Shelf>>, sections: Vec<String>, parts: &'static [&'static str], label: &'static str, build: impl FnOnce(&Shelf) + 'static) {
    let loading = LOADING.with(|slot| slot.get()).unwrap_or_else(|| RwSignal::new(String::new()));
    loading.set(label.into());
    leptos::task::spawn_local(async move {
        match ensure(shelf, &sections, parts).await {
            Ok(()) => {
                loading.set(String::new());
                if let Some(loaded) = shelf.get_untracked() {
                    build(&loaded);
                }
            }
            Err(message) => loading.set(message),
        }
    });
}

async fn ensure(shelf: RwSignal<Option<Shelf>>, sections: &[String], parts: &[&str]) -> Result<(), String> {
    while BUSY.with(|slot| slot.get()) {
        gloo_timers::future::TimeoutFuture::new(40).await;
    }
    BUSY.with(|slot| slot.set(true));
    let result = ensure_now(shelf, sections, parts).await;
    BUSY.with(|slot| slot.set(false));
    result
}

async fn ensure_now(shelf: RwSignal<Option<Shelf>>, sections: &[String], parts: &[&str]) -> Result<(), String> {
    let Some(current) = shelf.get_untracked() else { return Err("The library is still loading.".into()) };
    let mut jobs: Vec<(String, String, Vec<String>)> = Vec::new();
    for section in sections {
        for part in parts {
            let key = format!("{section}:{part}");
            if current.loaded.contains(&key) {
                continue;
            }
            let paths = match *part {
                "questions" => &current.index.questions,
                "flashcards" => &current.index.flashcards,
                "lessons" => &current.index.lessons,
                _ => continue,
            };
            let matched: Vec<String> = paths.iter().filter(|path| section_of(path).as_deref() == Some(section.as_str())).cloned().collect();
            jobs.push((key, (*part).to_string(), matched));
        }
    }
    if jobs.is_empty() {
        return Ok(());
    }
    let mut lessons = current.lessons;
    let mut questions = current.questions;
    let mut cards = current.cards;
    let mut loaded = current.loaded;
    for (key, part, paths) in jobs {
        for path in paths {
            let text = fetch_text(&format!("content/{path}")).await?;
            match part.as_str() {
                "questions" => questions.extend(serde_json::from_str::<Vec<Question>>(&text).map_err(|err| err.to_string())?),
                "flashcards" => cards.extend(serde_json::from_str::<Vec<Flashcard>>(&text).map_err(|err| err.to_string())?),
                "lessons" => lessons.push(serde_json::from_str::<Lesson>(&text).map_err(|err| err.to_string())?),
                _ => {}
            }
        }
        loaded.insert(key);
    }
    lessons.sort_by(|left, right| (left.exam != ExamId::Sie).cmp(&(right.exam != ExamId::Sie)).then(left.order.cmp(&right.order)));
    shelf.set(Some(Shelf { meta: current.meta, index: current.index, lessons, questions, cards, loaded }));
    Ok(())
}

async fn fetch_text(path: &str) -> Result<String, String> {
    let window = web_sys::window().ok_or("no window")?;
    let response = JsFuture::from(window.fetch_with_str(path)).await.map_err(|_| format!("Could not load {path}"))?;
    let response: web_sys::Response = response.dyn_into().map_err(|_| format!("Could not load {path}"))?;
    if !response.ok() {
        return Err(format!("Could not load {path}"));
    }
    let text = JsFuture::from(response.text().map_err(|_| format!("Could not read {path}"))?).await.map_err(|_| format!("Could not read {path}"))?;
    text.as_string().ok_or_else(|| format!("Could not read {path}"))
}
