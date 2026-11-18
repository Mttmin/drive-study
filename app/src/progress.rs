use std::cell::{Cell, RefCell};

use super::{AttemptRecord, DayRecord, LessonProgress, ProgressFile, Settings};
use drive_study_core::SrsCard;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use rexie::{ObjectStore, Rexie, TransactionMode};
use serde::{Deserialize, Serialize};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

const DISK_KEY: &str = "drive-study-disk-v2";

#[derive(Clone, Debug)]
pub struct ProfileHead {
    pub id: String,
    pub name: String,
    pub created_at: i64,
}

#[derive(Clone)]
pub struct LoadedState {
    pub head: Option<ProfileHead>,
    pub roster: Vec<ProfileHead>,
    pub settings: Settings,
    pub cards: Vec<SrsCard>,
    pub attempts: Vec<AttemptRecord>,
    pub days: Vec<String>,
    pub lessons: Vec<LessonProgress>,
    pub had_legacy: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct PersistNote {
    pub asked: bool,
    pub granted: bool,
}

#[derive(Clone, Copy)]
struct Sink {
    head: RwSignal<Option<ProfileHead>>,
    settings: RwSignal<Settings>,
    cards: RwSignal<Vec<SrsCard>>,
    attempts: RwSignal<Vec<AttemptRecord>>,
    days: RwSignal<Vec<String>>,
    lessons: RwSignal<Vec<LessonProgress>>,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Disk {
    #[serde(default)]
    active: Option<String>,
    #[serde(default)]
    profiles: Vec<ProfileRecord>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileRecord {
    id: String,
    name: String,
    created_at: i64,
    updated_at: i64,
    settings: Settings,
    srs: Vec<SrsCard>,
    attempts: Vec<AttemptRecord>,
    days: Vec<DayRecord>,
    #[serde(default)]
    lessons: Vec<LessonProgress>,
}

struct LegacyBlob {
    settings: Settings,
    cards: Vec<SrsCard>,
    attempts: Vec<AttemptRecord>,
    days: Vec<DayRecord>,
}

thread_local! {
    static DISK: RefCell<Disk> = RefCell::new(Disk::default());
    static LEGACY: RefCell<Option<LegacyBlob>> = const { RefCell::new(None) };
    static DIRTY: Cell<bool> = const { Cell::new(false) };
    static FLUSHING: Cell<bool> = const { Cell::new(false) };
    static READY: Cell<bool> = const { Cell::new(false) };
    static HOLD: Cell<bool> = const { Cell::new(false) };
    static SINK: Cell<Option<Sink>> = const { Cell::new(None) };
    static LISTENING: Cell<bool> = const { Cell::new(false) };
    static PERSIST: Cell<PersistNote> = const { Cell::new(PersistNote { asked: false, granted: false }) };
}

pub fn bind_progress(
    head: RwSignal<Option<ProfileHead>>,
    settings: RwSignal<Settings>,
    cards: RwSignal<Vec<SrsCard>>,
    attempts: RwSignal<Vec<AttemptRecord>>,
    days: RwSignal<Vec<String>>,
    lessons: RwSignal<Vec<LessonProgress>>,
) {
    SINK.with(|slot| {
        slot.set(Some(Sink {
            head,
            settings,
            cards,
            attempts,
            days,
            lessons,
        }));
    });
    install_listeners();
}

pub fn mark_progress_ready() {
    READY.with(|slot| slot.set(true));
}

pub fn persist_note() -> PersistNote {
    PERSIST.with(|slot| slot.get())
}

pub async fn boot_state() -> LoadedState {
    request_persist().await;
    let local = read_local();
    let idb = read_idb().await;
    let disk = match &idb {
        Some(saved) => merge(local, saved.clone()),
        None => local,
    };
    if disk.profiles.is_empty() {
        if let Some(legacy) = read_legacy().await {
            LEGACY.with(|slot| *slot.borrow_mut() = Some(legacy));
        }
    }
    DISK.with(|slot| *slot.borrow_mut() = disk);
    write_local();
    if idb.is_some() {
        let copy = DISK.with(|slot| slot.borrow().clone());
        let _ = write_idb(&copy).await;
    }
    loaded_from_disk()
}

pub async fn create_profile(name: &str) -> Result<LoadedState, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Type a name first.".into());
    }
    if name.chars().count() > 32 {
        return Err("Use 32 characters or fewer.".into());
    }
    persist_progress();
    flush_progress().await;
    let now = js_sys::Date::now() as i64;
    let first = DISK.with(|slot| slot.borrow().profiles.is_empty());
    let mut record = ProfileRecord {
        id: new_id(),
        name: name.to_string(),
        created_at: now,
        updated_at: now,
        settings: device_settings(),
        srs: Vec::new(),
        attempts: Vec::new(),
        days: Vec::new(),
        lessons: Vec::new(),
    };
    if first {
        if let Some(legacy) = take_legacy() {
            record.settings = legacy.settings;
            record.srs = legacy.cards;
            record.attempts = legacy.attempts;
            record.days = legacy.days;
            record.lessons = lessons_from_completed(&record.settings, Vec::new());
        }
    }
    DISK.with(|slot| upsert(&mut slot.borrow_mut(), record));
    write_both().await;
    Ok(loaded_from_disk())
}

pub async fn switch_profile(id: &str) -> Result<LoadedState, String> {
    persist_progress();
    flush_progress().await;
    let exists = DISK.with(|slot| {
        slot.borrow()
            .profiles
            .iter()
            .any(|profile| profile.id == id)
    });
    if !exists {
        return Err("That profile is not on this phone.".into());
    }
    DISK.with(|slot| slot.borrow_mut().active = Some(id.to_string()));
    write_both().await;
    Ok(loaded_from_disk())
}

pub fn persist_progress() {
    if HOLD.with(|slot| slot.get()) || !READY.with(|slot| slot.get()) {
        return;
    }
    let Some(record) = snapshot() else { return };
    DISK.with(|slot| upsert(&mut slot.borrow_mut(), record));
    write_local();
    DIRTY.with(|slot| slot.set(true));
    kick_flush();
}

pub async fn flush_progress() {
    persist_progress();
    wait_for_idb().await;
}

async fn wait_for_idb() {
    for _ in 0..40 {
        let dirty = DIRTY.with(|slot| slot.get());
        let flushing = FLUSHING.with(|slot| slot.get());
        if !dirty && !flushing {
            return;
        }
        TimeoutFuture::new(50).await;
    }
}

pub async fn export_progress() -> ProgressFile {
    persist_progress();
    flush_progress().await;
    let record = DISK.with(|slot| {
        let disk = slot.borrow();
        disk.active
            .as_ref()
            .and_then(|id| {
                disk.profiles
                    .iter()
                    .find(|profile| &profile.id == id)
                    .cloned()
            })
            .or_else(|| disk.profiles.first().cloned())
    });
    let Some(record) = record else {
        return ProgressFile {
            version: 2,
            exported_at: now_iso(),
            profile_id: String::new(),
            profile_name: String::new(),
            settings: Settings::default(),
            srs: Vec::new(),
            attempts: Vec::new(),
            days: Vec::new(),
            lessons: Vec::new(),
        };
    };
    ProgressFile {
        version: 2,
        exported_at: now_iso(),
        profile_id: record.id,
        profile_name: record.name,
        settings: record.settings,
        srs: record.srs,
        attempts: record.attempts,
        days: record.days,
        lessons: record.lessons,
    }
}

pub fn sync_code(file: &ProgressFile) -> String {
    let bytes = serde_json::to_vec(file).unwrap_or_default();
    format!("DS2.{}", base64url(&bytes))
}

pub fn parse_transfer(text: &str) -> Result<ProgressFile, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("Paste a sync code or choose a JSON file.".into());
    }
    let file = if trimmed.starts_with('{') {
        serde_json::from_str::<ProgressFile>(trimmed)
            .map_err(|_| "Could not read that file.".to_string())?
    } else {
        let compact: String = trimmed.chars().filter(|ch| !ch.is_whitespace()).collect();
        let payload = compact
            .strip_prefix("DS2.")
            .or_else(|| compact.strip_prefix("DS1."))
            .unwrap_or(compact.as_str());
        let bytes = base64url_decode(payload)?;
        serde_json::from_slice::<ProgressFile>(&bytes)
            .map_err(|_| "Could not read that sync code.".to_string())?
    };
    if file.version != 1 && file.version != 2 {
        return Err("That file is not a Drive Study progress export.".into());
    }
    Ok(file)
}

pub fn import_replaces(file: &ProgressFile) -> Option<String> {
    let id = if !file.profile_id.is_empty() {
        file.profile_id.clone()
    } else {
        return DISK.with(|slot| {
            let disk = slot.borrow();
            disk.active.as_ref().and_then(|id| {
                disk.profiles
                    .iter()
                    .find(|profile| &profile.id == id)
                    .map(|profile| profile.name.clone())
            })
        });
    };
    DISK.with(|slot| {
        slot.borrow()
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .map(|profile| profile.name.clone())
    })
}

pub async fn import_progress(file: &ProgressFile) -> Result<LoadedState, String> {
    if file.version != 1 && file.version != 2 {
        return Err("That file is not a Drive Study progress export.".into());
    }
    persist_progress();
    flush_progress().await;
    let now = js_sys::Date::now() as i64;
    let id = if file.version >= 2 && !file.profile_id.is_empty() {
        file.profile_id.clone()
    } else if let Some(active) = DISK.with(|slot| slot.borrow().active.clone()) {
        active
    } else {
        new_id()
    };
    let created = DISK.with(|slot| {
        slot.borrow()
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .map(|profile| profile.created_at)
            .unwrap_or(now)
    });
    let name = if !file.profile_name.trim().is_empty() {
        file.profile_name.trim().to_string()
    } else {
        DISK.with(|slot| {
            slot.borrow()
                .profiles
                .iter()
                .find(|profile| profile.id == id)
                .map(|profile| profile.name.clone())
        })
        .or_else(|| {
            SINK.with(|slot| slot.get())
                .and_then(|sink| sink.head.get_untracked())
                .map(|head| head.name)
        })
        .unwrap_or_else(|| "Imported".into())
    };
    let lessons = lessons_from_completed(&file.settings, file.lessons.clone());
    let record = ProfileRecord {
        id,
        name,
        created_at: created,
        updated_at: now,
        settings: file.settings.clone(),
        srs: file.srs.clone(),
        attempts: file.attempts.clone(),
        days: file.days.clone(),
        lessons,
    };
    DISK.with(|slot| upsert(&mut slot.borrow_mut(), record));
    write_both().await;
    Ok(loaded_from_disk())
}

pub(super) fn note_lesson(lesson_id: &str, index: usize, total: usize, done: bool) {
    let Some(sink) = SINK.with(|slot| slot.get()) else {
        return;
    };
    if sink.head.get_untracked().is_none() {
        return;
    }
    let now = js_sys::Date::now() as i64;
    let mut changed = false;
    sink.lessons.update(|list| {
        if let Some(row) = list.iter_mut().find(|row| row.lesson_id == lesson_id) {
            if row.status == "finished" && !done {
                return;
            }
            let status = if done { "finished" } else { "started" };
            let next_index = index as u32;
            let next_total = total as u32;
            if row.status != status || row.index != next_index || row.total != next_total {
                row.status = status.into();
                row.index = next_index;
                row.total = next_total;
                row.updated_at = now;
                changed = true;
            }
        } else {
            list.push(LessonProgress {
                lesson_id: lesson_id.into(),
                status: if done {
                    "finished".into()
                } else {
                    "started".into()
                },
                index: index as u32,
                total: total as u32,
                updated_at: now,
            });
            changed = true;
        }
    });
    if done {
        sink.settings.update(|settings| {
            if !settings.completed_lessons.iter().any(|id| id == lesson_id) {
                settings.completed_lessons.push(lesson_id.into());
                changed = true;
            }
        });
    }
    if changed {
        persist_progress();
    }
}

fn device_settings() -> Settings {
    let Some(sink) = SINK.with(|slot| slot.get()) else {
        return Settings::default();
    };
    if sink.head.get_untracked().is_none() {
        return Settings::default();
    }
    let mut settings = sink.settings.get_untracked();
    settings.completed_lessons.clear();
    settings
}

fn snapshot() -> Option<ProfileRecord> {
    let sink = SINK.with(|slot| slot.get())?;
    let head = sink.head.get_untracked()?;
    let (created, old_days) = DISK.with(|slot| {
        let disk = slot.borrow();
        match disk.profiles.iter().find(|profile| profile.id == head.id) {
            Some(profile) => (profile.created_at, profile.days.clone()),
            None => (head.created_at, Vec::new()),
        }
    });
    let day_keys = sink.days.get_untracked();
    Some(ProfileRecord {
        id: head.id,
        name: head.name,
        created_at: created,
        updated_at: js_sys::Date::now() as i64,
        settings: sink.settings.get_untracked(),
        srs: sink.cards.get_untracked(),
        attempts: sink.attempts.get_untracked(),
        lessons: sink.lessons.get_untracked(),
        days: day_keys
            .into_iter()
            .map(|date| {
                let count = old_days
                    .iter()
                    .find(|day| day.date == date)
                    .map(|day| day.count)
                    .unwrap_or(1);
                DayRecord { date, count }
            })
            .collect(),
    })
}

fn loaded_from_disk() -> LoadedState {
    let disk = DISK.with(|slot| slot.borrow().clone());
    let mut roster: Vec<ProfileHead> = disk
        .profiles
        .iter()
        .map(|profile| ProfileHead {
            id: profile.id.clone(),
            name: profile.name.clone(),
            created_at: profile.created_at,
        })
        .collect();
    roster.sort_by_key(|profile| profile.created_at);
    let active = disk
        .active
        .as_ref()
        .and_then(|id| disk.profiles.iter().find(|profile| &profile.id == id))
        .or_else(|| disk.profiles.first());
    let had_legacy = LEGACY.with(|slot| slot.borrow().is_some());
    match active {
        Some(record) => LoadedState {
            head: Some(ProfileHead {
                id: record.id.clone(),
                name: record.name.clone(),
                created_at: record.created_at,
            }),
            roster,
            settings: record.settings.clone(),
            cards: record.srs.clone(),
            attempts: record.attempts.clone(),
            days: record.days.iter().map(|day| day.date.clone()).collect(),
            lessons: record.lessons.clone(),
            had_legacy,
        },
        None => LoadedState {
            head: None,
            roster,
            settings: Settings::default(),
            cards: Vec::new(),
            attempts: Vec::new(),
            days: Vec::new(),
            lessons: Vec::new(),
            had_legacy,
        },
    }
}

fn upsert(disk: &mut Disk, record: ProfileRecord) {
    disk.active = Some(record.id.clone());
    if let Some(slot) = disk
        .profiles
        .iter_mut()
        .find(|profile| profile.id == record.id)
    {
        *slot = record;
    } else {
        disk.profiles.push(record);
    }
}

fn merge(local: Disk, idb: Disk) -> Disk {
    let local_stamp = local
        .profiles
        .iter()
        .map(|profile| profile.updated_at)
        .max()
        .unwrap_or(0);
    let idb_stamp = idb
        .profiles
        .iter()
        .map(|profile| profile.updated_at)
        .max()
        .unwrap_or(0);
    let local_active = local.active;
    let idb_active = idb.active;
    let mut profiles = local.profiles;
    for record in idb.profiles {
        if let Some(slot) = profiles.iter_mut().find(|profile| profile.id == record.id) {
            if record.updated_at > slot.updated_at {
                *slot = record;
            }
        } else {
            profiles.push(record);
        }
    }
    let active = if idb_stamp > local_stamp {
        idb_active.or(local_active)
    } else {
        local_active.or(idb_active)
    };
    Disk { active, profiles }
}

fn lessons_from_completed(
    settings: &Settings,
    mut lessons: Vec<LessonProgress>,
) -> Vec<LessonProgress> {
    let now = js_sys::Date::now() as i64;
    for id in &settings.completed_lessons {
        if lessons.iter().any(|row| &row.lesson_id == id) {
            continue;
        }
        lessons.push(LessonProgress {
            lesson_id: id.clone(),
            status: "finished".into(),
            index: 0,
            total: 0,
            updated_at: now,
        });
    }
    lessons
}

fn take_legacy() -> Option<LegacyBlob> {
    LEGACY.with(|slot| slot.borrow_mut().take())
}

fn new_id() -> String {
    let now = js_sys::Date::now() as u64;
    let rand = (js_sys::Math::random() * 1_000_000_000.0) as u64;
    format!("p{now:x}{rand:x}")
}

fn now_iso() -> String {
    js_sys::Date::new_0().to_iso_string().into()
}

fn kick_flush() {
    if FLUSHING.with(|slot| slot.get()) {
        return;
    }
    FLUSHING.with(|slot| slot.set(true));
    leptos::task::spawn_local(async move {
        let mut failures = 0;
        loop {
            if !DIRTY.with(|slot| slot.get()) {
                break;
            }
            DIRTY.with(|slot| slot.set(false));
            let disk = DISK.with(|slot| slot.borrow().clone());
            if write_idb(&disk).await.is_err() {
                failures += 1;
                DIRTY.with(|slot| slot.set(true));
                if failures >= 3 {
                    break;
                }
                TimeoutFuture::new(300).await;
            } else {
                failures = 0;
            }
        }
        FLUSHING.with(|slot| slot.set(false));
        if DIRTY.with(|slot| slot.get()) && failures < 3 {
            kick_flush();
        }
    });
}

async fn write_both() {
    HOLD.with(|slot| slot.set(true));
    write_local();
    DIRTY.with(|slot| slot.set(true));
    kick_flush();
    wait_for_idb().await;
    HOLD.with(|slot| slot.set(false));
}

fn write_local() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return;
    };
    let disk = DISK.with(|slot| slot.borrow().clone());
    let Ok(text) = serde_json::to_string(&disk) else {
        return;
    };
    let _ = storage.set_item(DISK_KEY, &text);
}

fn read_local() -> Disk {
    let Some(window) = web_sys::window() else {
        return Disk::default();
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return Disk::default();
    };
    let Ok(Some(text)) = storage.get_item(DISK_KEY) else {
        return Disk::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

async fn open_db() -> Result<Rexie, String> {
    Rexie::builder("drive-study")
        .version(2)
        .add_object_store(ObjectStore::new("settings"))
        .add_object_store(ObjectStore::new("srs").key_path("id"))
        .add_object_store(ObjectStore::new("attempts").auto_increment(true))
        .add_object_store(ObjectStore::new("days"))
        .add_object_store(ObjectStore::new("profiles").key_path("id"))
        .add_object_store(ObjectStore::new("meta"))
        .build()
        .await
        .map_err(|err| format!("{err:?}"))
}

async fn read_idb() -> Option<Disk> {
    let db = open_db().await.ok()?;
    let tx = db
        .transaction(&["profiles", "meta"], TransactionMode::ReadOnly)
        .ok()?;
    let profiles = read_all::<ProfileRecord>(&tx, "profiles").await;
    let active = match tx.store("meta") {
        Ok(store) => store
            .get(JsValue::from_str("active"))
            .await
            .ok()
            .flatten()
            .and_then(|value| value.as_string()),
        Err(_) => None,
    };
    if tx.done().await.is_err() && profiles.is_empty() {
        return None;
    }
    Some(Disk { active, profiles })
}

async fn read_legacy() -> Option<LegacyBlob> {
    let db = open_db().await.ok()?;
    let tx = db
        .transaction(
            &["settings", "srs", "attempts", "days"],
            TransactionMode::ReadOnly,
        )
        .ok()?;
    let settings_raw = match tx.store("settings") {
        Ok(store) => store.get(JsValue::from_str("app")).await.ok().flatten(),
        Err(_) => None,
    };
    let has_settings = settings_raw.is_some();
    let settings = settings_raw
        .and_then(|value| serde_wasm_bindgen::from_value(value).ok())
        .unwrap_or_default();
    let cards = read_all::<SrsCard>(&tx, "srs").await;
    let attempts = read_all::<AttemptRecord>(&tx, "attempts").await;
    let days = read_all::<DayRecord>(&tx, "days").await;
    let _ = tx.done().await;
    if !has_settings && cards.is_empty() && attempts.is_empty() && days.is_empty() {
        return None;
    }
    Some(LegacyBlob {
        settings,
        cards,
        attempts,
        days,
    })
}

async fn read_all<T: for<'de> Deserialize<'de>>(tx: &rexie::Transaction, store: &str) -> Vec<T> {
    let Ok(store) = tx.store(store) else {
        return Vec::new();
    };
    let Ok(values) = store.get_all(None, None).await else {
        return Vec::new();
    };
    values
        .into_iter()
        .filter_map(|value| serde_wasm_bindgen::from_value(value).ok())
        .collect()
}

async fn write_idb(disk: &Disk) -> Result<(), String> {
    let db = open_db().await?;
    let tx = db
        .transaction(&["profiles", "meta"], TransactionMode::ReadWrite)
        .map_err(|err| format!("{err:?}"))?;
    let profiles = tx.store("profiles").map_err(|err| format!("{err:?}"))?;
    profiles.clear().await.map_err(|err| format!("{err:?}"))?;
    for record in &disk.profiles {
        let value = serde_wasm_bindgen::to_value(record).map_err(|err| err.to_string())?;
        profiles
            .put(&value, None)
            .await
            .map_err(|err| format!("{err:?}"))?;
    }
    let meta = tx.store("meta").map_err(|err| format!("{err:?}"))?;
    if let Some(active) = &disk.active {
        meta.put(
            &JsValue::from_str(active),
            Some(&JsValue::from_str("active")),
        )
        .await
        .map_err(|err| format!("{err:?}"))?;
    }
    tx.done()
        .await
        .map(|_| ())
        .map_err(|err| format!("{err:?}"))
}

fn install_listeners() {
    if LISTENING.with(|slot| slot.get()) {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    LISTENING.with(|slot| slot.set(true));
    let flush = Closure::wrap(Box::new(move || {
        persist_progress();
    }) as Box<dyn FnMut()>);
    let _ = window.add_event_listener_with_callback("pagehide", flush.as_ref().unchecked_ref());
    flush.forget();
    let hidden = Closure::wrap(Box::new(move || {
        let concealed = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| js_sys::Reflect::get(&document, &JsValue::from_str("hidden")).ok())
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        if concealed {
            persist_progress();
        }
    }) as Box<dyn FnMut()>);
    let _ = window
        .add_event_listener_with_callback("visibilitychange", hidden.as_ref().unchecked_ref());
    hidden.forget();
}

async fn request_persist() {
    let Some(storage) = web_sys::window().and_then(|window| {
        js_sys::Reflect::get(&window.navigator(), &JsValue::from_str("storage")).ok()
    }) else {
        PERSIST.with(|slot| {
            slot.set(PersistNote {
                asked: false,
                granted: false,
            })
        });
        return;
    };
    let Ok(persist) = js_sys::Reflect::get(&storage, &JsValue::from_str("persist"))
        .and_then(|value| value.dyn_into::<js_sys::Function>())
    else {
        PERSIST.with(|slot| {
            slot.set(PersistNote {
                asked: false,
                granted: false,
            })
        });
        return;
    };
    let Ok(promise) = persist
        .call0(&storage)
        .and_then(|value| value.dyn_into::<js_sys::Promise>())
    else {
        PERSIST.with(|slot| {
            slot.set(PersistNote {
                asked: false,
                granted: false,
            })
        });
        return;
    };
    let pending = JsFuture::from(promise);
    let timeout = TimeoutFuture::new(1_500);
    futures::pin_mut!(pending, timeout);
    let granted = match futures::future::select(pending, timeout).await {
        futures::future::Either::Left((Ok(value), _)) => value.as_bool().unwrap_or(false),
        _ => false,
    };
    PERSIST.with(|slot| {
        slot.set(PersistNote {
            asked: true,
            granted,
        })
    });
}

fn base64url(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let mut value = (chunk[0] as u32) << 16;
        if chunk.len() > 1 {
            value |= (chunk[1] as u32) << 8;
        }
        if chunk.len() > 2 {
            value |= chunk[2] as u32;
        }
        out.push(TABLE[((value >> 18) & 63) as usize] as char);
        out.push(TABLE[((value >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((value >> 6) & 63) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(TABLE[(value & 63) as usize] as char);
        }
    }
    out.replace('+', "-").replace('/', "_")
}

fn base64url_decode(text: &str) -> Result<Vec<u8>, String> {
    let mut padded = text.trim().replace('-', "+").replace('_', "/");
    if padded
        .chars()
        .any(|ch| !(ch.is_ascii_alphanumeric() || ch == '+' || ch == '/' || ch == '='))
    {
        return Err("Could not read that sync code.".into());
    }
    while padded.len() % 4 != 0 {
        padded.push('=');
    }
    let bytes = padded.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        if chunk.len() < 4 {
            return Err("Could not read that sync code.".into());
        }
        let mut values = [0u8; 4];
        let mut pad = 0;
        for (index, byte) in chunk.iter().copied().enumerate() {
            values[index] = match byte {
                b'=' => {
                    pad += 1;
                    0
                }
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                _ => return Err("Could not read that sync code.".into()),
            };
        }
        if pad > 2 {
            return Err("Could not read that sync code.".into());
        }
        out.push((values[0] << 2) | (values[1] >> 4));
        if pad < 2 {
            out.push((values[1] << 4) | (values[2] >> 2));
        }
        if pad < 1 {
            out.push((values[2] << 6) | values[3]);
        }
    }
    Ok(out)
}
