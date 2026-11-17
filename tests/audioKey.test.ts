import { describe, expect, it } from "vitest";
import { clipKey, choicesReadout, normalizeSpokenText } from "../shared/audioKey.js";

describe("audio keys", () => {
  it("matches the published test vector", () => {
    expect(clipKey("natural-en-us-v1", "Hello, Drive Study.")).toBe(
      "1335a8d460eaaaf2cc829487fb6ea1b421f7f7cb",
    );
    expect(clipKey("natural-en-us-v1", "Hello,   Drive Study.\n")).toBe(
      "1335a8d460eaaaf2cc829487fb6ea1b421f7f7cb",
    );
  });

  it("collapses nbsp and builds the choices line", () => {
    expect(normalizeSpokenText("a\u00A0b\n\nc")).toBe("a b c");
    expect(
      choicesReadout({
        type: "mc",
        choices: [
          { key: "A", text: "Common stock" },
          { key: "B", text: "Preferred stock." },
        ],
      }),
    ).toBe("A. Common stock. B. Preferred stock.");
    expect(choicesReadout({ type: "tf" })).toBe("True, or false.");
  });
});
