import { describe, expect, it } from "vitest";
import { isDue, nextEf, reviewCard, streakFromDates, type SrsMeta } from "../src/lib/srs";

const meta: SrsMeta = { id: "sie-q-0001", kind: "question", exam: "SIE", section: "SIE-1" };

describe("reviewCard", () => {
  it("graduates after three correct answers on separate days", () => {
    let card = reviewCard(null, meta, "correct", new Date("2026-01-01T15:00:00"));
    expect(card.correctDates).toEqual(["2026-01-01"]);
    expect(card.graduated).toBe(false);
    expect(card.intervalDays).toBe(1);
    expect(card.reps).toBe(1);
    expect(card.ef).toBe(2.6);

    card = reviewCard(card, meta, "correct", new Date("2026-01-01T18:00:00"));
    expect(card.correctDates).toEqual(["2026-01-01"]);
    expect(card.reps).toBe(1);
    expect(card.intervalDays).toBe(1);
    expect(card.ef).toBe(2.6);

    card = reviewCard(card, meta, "correct", new Date("2026-01-02T15:00:00"));
    expect(card.correctDates).toEqual(["2026-01-01", "2026-01-02"]);
    expect(card.intervalDays).toBe(3);
    expect(card.ef).toBe(2.7);
    expect(card.graduated).toBe(false);

    card = reviewCard(card, meta, "correct", new Date("2026-01-05T15:00:00"));
    expect(card.correctDates).toHaveLength(3);
    expect(card.graduated).toBe(true);
    expect(card.intervalDays).toBe(8);
    expect(card.ef).toBe(2.8);
    expect(isDue(card, new Date("2026-01-06T15:00:00"))).toBe(false);
  });

  it("resets the streak on a wrong answer and schedules a short return", () => {
    let card = reviewCard(null, meta, "correct", new Date("2026-01-01T15:00:00"));
    card = reviewCard(card, meta, "correct", new Date("2026-01-02T15:00:00"));
    const now = new Date("2026-01-10T15:00:00");
    card = reviewCard(card, meta, "wrong", now);
    expect(card.graduated).toBe(false);
    expect(card.correctDates).toEqual([]);
    expect(card.lapses).toBe(1);
    expect(card.reps).toBe(0);
    expect(card.due - now.getTime()).toBe(10 * 60 * 1000);
    expect(isDue(card, new Date(now.getTime() + 11 * 60 * 1000))).toBe(true);
  });

  it("puts a correct guess back in the deck without graduation credit", () => {
    let card = reviewCard(null, meta, "correct", new Date("2026-01-01T15:00:00"));
    card = reviewCard(card, meta, "correct", new Date("2026-01-02T15:00:00"));
    const now = new Date("2026-01-03T15:00:00");
    card = reviewCard(card, meta, "guessed", now);
    expect(card.correctDates).toEqual([]);
    expect(card.graduated).toBe(false);
    expect(card.due - now.getTime()).toBe(24 * 60 * 60 * 1000);
    expect(nextEf(2.5, 5)).toBe(2.6);
  });
});

describe("streakFromDates", () => {
  it("counts a run ending today or yesterday", () => {
    expect(streakFromDates(["2026-04-01", "2026-04-02", "2026-04-03"], "2026-04-03")).toBe(3);
    expect(streakFromDates(["2026-04-01", "2026-04-02"], "2026-04-03")).toBe(2);
    expect(streakFromDates(["2026-04-01"], "2026-04-03")).toBe(0);
  });
});
