import { describe, expect, it } from "vitest";
import { answerGrammar, parseVoiceCommand } from "../src/lib/grammar";

const mc = { choiceKeys: ["A", "B", "C", "D"] };
const tf = { choiceKeys: ["T", "F"] };

describe("parseVoiceCommand", () => {
  it("reads letters, phonetics, and lead-in phrases", () => {
    expect(parseVoiceCommand("a", mc)).toEqual({ type: "answer", key: "A" });
    expect(parseVoiceCommand("Ay", mc)).toEqual({ type: "answer", key: "A" });
    expect(parseVoiceCommand("the answer is b", mc)).toEqual({ type: "answer", key: "B" });
    expect(parseVoiceCommand("bravo", mc)).toEqual({ type: "answer", key: "B" });
    expect(parseVoiceCommand("charlie.", mc)).toEqual({ type: "answer", key: "C" });
    expect(parseVoiceCommand("delta", mc)).toEqual({ type: "answer", key: "D" });
    expect(parseVoiceCommand("option see", mc)).toEqual({ type: "answer", key: "C" });
    expect(parseVoiceCommand("be", mc)).toEqual({ type: "answer", key: "B" });
    expect(parseVoiceCommand("dee", mc)).toEqual({ type: "answer", key: "D" });
    expect(parseVoiceCommand("alpha bravo", mc)).toEqual({ type: "answer", key: "B" });
  });

  it("reads true and false only when those keys exist", () => {
    expect(parseVoiceCommand("true", tf)).toEqual({ type: "answer", key: "T" });
    expect(parseVoiceCommand("false", tf)).toEqual({ type: "answer", key: "F" });
    expect(parseVoiceCommand("true", mc)).toEqual({ type: "unrecognized" });
  });

  it("prefers commands over answers", () => {
    expect(parseVoiceCommand("repeat please", mc)).toEqual({ type: "repeat" });
    expect(parseVoiceCommand("skip", mc)).toEqual({ type: "skip" });
    expect(parseVoiceCommand("next", mc)).toEqual({ type: "skip" });
    expect(parseVoiceCommand("explain", mc)).toEqual({ type: "explain" });
    expect(parseVoiceCommand("explain bravo", mc)).toEqual({ type: "explain" });
    expect(parseVoiceCommand("pause", mc)).toEqual({ type: "pause" });
    expect(parseVoiceCommand("stop", mc)).toEqual({ type: "stop" });
    expect(parseVoiceCommand("I don't know", mc)).toEqual({ type: "dontknow" });
    expect(parseVoiceCommand("i dont know", mc)).toEqual({ type: "dontknow" });
    expect(parseVoiceCommand("no idea", mc)).toEqual({ type: "dontknow" });
  });

  it("rejects noise and bare homophones inside a sentence", () => {
    expect(parseVoiceCommand("", mc)).toEqual({ type: "unrecognized" });
    expect(parseVoiceCommand("banana", mc)).toEqual({ type: "unrecognized" });
    expect(parseVoiceCommand("I see", mc)).toEqual({ type: "unrecognized" });
  });

  it("maps know only for flashcards", () => {
    expect(parseVoiceCommand("got it", { ...mc, allowKnow: true })).toEqual({ type: "know" });
    expect(parseVoiceCommand("got it", mc)).toEqual({ type: "unrecognized" });
    expect(parseVoiceCommand("I don't know", { ...mc, allowKnow: true })).toEqual({ type: "dontknow" });
  });

  it("builds a JSGF grammar that includes the car commands", () => {
    const grammar = answerGrammar();
    expect(grammar.startsWith("#JSGF V1.0;")).toBe(true);
    expect(grammar).toContain("alpha");
    expect(grammar).toContain("i don't know");
    expect(grammar).not.toContain(" true");
  });
});
