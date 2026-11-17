import { describe, expect, it } from "vitest";
import { allocateQuestions, drawWeighted } from "../src/lib/blueprint";
import { readiness, scoreExam, weakTopics } from "../src/lib/scoring";

const sie = [
  { id: "SIE-1", weight: 16 },
  { id: "SIE-2", weight: 44 },
  { id: "SIE-3", weight: 31 },
  { id: "SIE-4", weight: 9 },
];

const s7 = [
  { id: "S7-1", weight: 7 },
  { id: "S7-2", weight: 9 },
  { id: "S7-3", weight: 73 },
  { id: "S7-4", weight: 11 },
];

describe("allocateQuestions", () => {
  it("matches the published FINRA question counts", () => {
    expect(allocateQuestions(sie, 75)).toEqual({
      "SIE-1": 12,
      "SIE-2": 33,
      "SIE-3": 23,
      "SIE-4": 7,
    });
    expect(allocateQuestions(s7, 125)).toEqual({
      "S7-1": 9,
      "S7-2": 11,
      "S7-3": 91,
      "S7-4": 14,
    });
  });

  it("sums to the requested total", () => {
    const quotas = allocateQuestions(sie, 10);
    expect(Object.values(quotas).reduce((sum, n) => sum + n, 0)).toBe(10);
  });
});

describe("drawWeighted", () => {
  it("fills from other sections when a quota cannot be met", () => {
    const items = [
      { id: "a1", section: "SIE-1" },
      { id: "b1", section: "SIE-2" },
      { id: "b2", section: "SIE-2" },
      { id: "b3", section: "SIE-2" },
    ];
    const drawn = drawWeighted(items, sie, 4, () => 0);
    expect(drawn).toHaveLength(4);
    expect(new Set(drawn.map((item) => item.id)).size).toBe(4);
  });
});

describe("scoreExam", () => {
  it("uses the unrounded ratio against the pass mark", () => {
    const fail = [
      ...Array.from({ length: 52 }, () => ({ correct: true, section: "SIE-1" })),
      ...Array.from({ length: 23 }, () => ({ correct: false, section: "SIE-2" })),
    ];
    const pass = [
      ...Array.from({ length: 53 }, () => ({ correct: true, section: "SIE-1" })),
      ...Array.from({ length: 22 }, () => ({ correct: false, section: "SIE-2" })),
    ];
    expect(scoreExam(fail, 70).passed).toBe(false);
    expect(scoreExam(fail, 70).percent).toBe(69.3);
    expect(scoreExam(pass, 70).passed).toBe(true);
    expect(scoreExam(pass, 70).percent).toBe(70.7);
    expect(scoreExam(pass, 70).bySection["SIE-1"].correct).toBe(53);
  });
});

describe("readiness", () => {
  it("stays empty until a section has enough answers", () => {
    const result = readiness({
      sections: [{ id: "SIE-1", weight: 16 }],
      attempts: [{ section: "SIE-1", correct: true }],
      seen: 1,
      pool: 10,
    });
    expect(result.score).toBeNull();
  });

  it("weights accuracy and scales by coverage", () => {
    const result = readiness({
      sections: [
        { id: "A", weight: 50 },
        { id: "B", weight: 50 },
      ],
      attempts: [
        { section: "A", topic: "markets", correct: true },
        { section: "A", topic: "markets", correct: true },
        { section: "A", topic: "markets", correct: true },
        { section: "B", topic: "options", correct: false },
        { section: "B", topic: "options", correct: false },
        { section: "B", topic: "options", correct: false },
      ],
      seen: 6,
      pool: 10,
    });
    expect(result.score).toBe(41);
    expect(result.coverage).toBe(0.6);
    expect(result.weak.map((row) => row.id)).toEqual(["B"]);
    expect(weakTopics([
      { section: "B", topic: "options", correct: false },
      { section: "B", topic: "options", correct: false },
    ])[0].topic).toBe("options");
  });
});
