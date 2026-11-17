import type { Settings } from "./types";
import type { SrsCard } from "./srs";

export interface AttemptRecord {
  id?: number;
  itemId: string;
  kind: "question" | "flashcard";
  exam: "SIE" | "S7";
  section: string;
  topic?: string;
  correct: boolean;
  guessed: boolean;
  mode: string;
  at: number;
}

export interface DayRecord {
  date: string;
  count: number;
}

export interface ProgressFile {
  version: 1;
  exportedAt: string;
  settings: Settings;
  srs: SrsCard[];
  attempts: AttemptRecord[];
  days: DayRecord[];
}

const memory = {
  on: false,
  settings: null as Settings | null,
  srs: new Map<string, SrsCard>(),
  attempts: [] as AttemptRecord[],
  days: new Map<string, number>(),
};

let dbPromise: Promise<IDBDatabase | null> | null = null;

function openDb(): Promise<IDBDatabase | null> {
  if (!dbPromise) {
    dbPromise = new Promise((resolve) => {
      if (typeof indexedDB === "undefined") {
        memory.on = true;
        resolve(null);
        return;
      }
      const request = indexedDB.open("drive-study", 1);
      request.onupgradeneeded = () => {
        const db = request.result;
        if (!db.objectStoreNames.contains("settings")) db.createObjectStore("settings");
        if (!db.objectStoreNames.contains("srs")) db.createObjectStore("srs", { keyPath: "id" });
        if (!db.objectStoreNames.contains("attempts")) {
          db.createObjectStore("attempts", { keyPath: "id", autoIncrement: true });
        }
        if (!db.objectStoreNames.contains("days")) db.createObjectStore("days");
      };
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => {
        memory.on = true;
        resolve(null);
      };
    });
  }
  return dbPromise;
}

function req<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

export async function loadState(fallback: Settings): Promise<{
  settings: Settings;
  cards: SrsCard[];
  attempts: AttemptRecord[];
  days: DayRecord[];
}> {
  const db = await openDb();
  if (!db) {
    return {
      settings: memory.settings ?? fallback,
      cards: [...memory.srs.values()],
      attempts: memory.attempts,
      days: [...memory.days.entries()].map(([date, count]) => ({ date, count })),
    };
  }
  const tx = db.transaction(["settings", "srs", "attempts", "days"], "readonly");
  const settings = (await req(tx.objectStore("settings").get("app"))) as Settings | undefined;
  const cards = (await req(tx.objectStore("srs").getAll())) as SrsCard[];
  const attempts = (await req(tx.objectStore("attempts").getAll())) as AttemptRecord[];
  const days = (await req(tx.objectStore("days").getAll())) as DayRecord[];
  return { settings: { ...fallback, ...settings }, cards, attempts, days };
}

export async function saveSettings(settings: Settings): Promise<void> {
  const db = await openDb();
  if (!db) {
    memory.settings = settings;
    return;
  }
  const tx = db.transaction("settings", "readwrite");
  tx.objectStore("settings").put(settings, "app");
  await transactionDone(tx);
}

export async function saveCard(card: SrsCard): Promise<void> {
  const db = await openDb();
  if (!db) {
    memory.srs.set(card.id, card);
    return;
  }
  const tx = db.transaction("srs", "readwrite");
  tx.objectStore("srs").put(card);
  await transactionDone(tx);
}

export async function saveAttempt(attempt: AttemptRecord): Promise<void> {
  const db = await openDb();
  if (!db) {
    memory.attempts.push(attempt);
    return;
  }
  const tx = db.transaction("attempts", "readwrite");
  tx.objectStore("attempts").add(attempt);
  await transactionDone(tx);
}

export async function touchDay(date: string): Promise<void> {
  const db = await openDb();
  if (!db) {
    memory.days.set(date, (memory.days.get(date) ?? 0) + 1);
    return;
  }
  const tx = db.transaction("days", "readwrite");
  const store = tx.objectStore("days");
  const current = (await req(store.get(date))) as DayRecord | undefined;
  store.put({ date, count: (current?.count ?? 0) + 1 });
  await transactionDone(tx);
}

export async function exportProgress(settings: Settings): Promise<ProgressFile> {
  const state = await loadState(settings);
  return {
    version: 1,
    exportedAt: new Date().toISOString(),
    settings: state.settings,
    srs: state.cards,
    attempts: state.attempts,
    days: state.days,
  };
}

export async function importProgress(file: ProgressFile): Promise<void> {
  if (file.version !== 1 || !file.settings || !Array.isArray(file.srs) || !Array.isArray(file.attempts)) {
    throw new Error("That file is not a Drive Study progress export.");
  }
  const db = await openDb();
  if (!db) {
    memory.settings = file.settings;
    memory.srs = new Map(file.srs.map((card) => [card.id, card]));
    memory.attempts = file.attempts;
    memory.days = new Map(file.days.map((day) => [day.date, day.count]));
    return;
  }
  const tx = db.transaction(["settings", "srs", "attempts", "days"], "readwrite");
  tx.objectStore("settings").put(file.settings, "app");
  const srs = tx.objectStore("srs");
  srs.clear();
  for (const card of file.srs) srs.put(card);
  const attempts = tx.objectStore("attempts");
  attempts.clear();
  for (const attempt of file.attempts) attempts.put(attempt);
  const days = tx.objectStore("days");
  days.clear();
  for (const day of file.days ?? []) days.put(day);
  await transactionDone(tx);
}

function transactionDone(tx: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
    tx.onabort = () => reject(tx.error);
  });
}
