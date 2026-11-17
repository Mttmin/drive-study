export type VoiceCommand =
  | { type: "answer"; key: string }
  | { type: "repeat" }
  | { type: "skip" }
  | { type: "explain" }
  | { type: "dontknow" }
  | { type: "pause" }
  | { type: "stop" }
  | { type: "know" }
  | { type: "unrecognized" };

export interface ParseOptions {
  choiceKeys?: string[];
  allowKnow?: boolean;
}

const STRONG: Record<string, string> = {
  alpha: "A",
  alfa: "A",
  bravo: "B",
  charlie: "C",
  charley: "C",
  delta: "D",
  true: "T",
  false: "F",
};

const WEAK: Record<string, string> = {
  ay: "A",
  eh: "A",
  be: "B",
  bee: "B",
  see: "C",
  sea: "C",
  cee: "C",
  dee: "D",
};

const LETTER: Record<string, string> = {
  a: "A",
  b: "B",
  c: "C",
  d: "D",
  t: "T",
  f: "F",
};

const COMMANDS: { type: Exclude<VoiceCommand["type"], "answer" | "unrecognized" | "know">; phrases: string[][] }[] = [
  { type: "stop", phrases: [["stop"], ["end", "session"], ["quit"]] },
  { type: "pause", phrases: [["pause"], ["hold", "on"]] },
  { type: "repeat", phrases: [["repeat"], ["repeat", "that"], ["say", "that", "again"], ["say", "again"], ["previous"]] },
  { type: "skip", phrases: [["skip"], ["next"], ["next", "question"], ["pass"]] },
  { type: "explain", phrases: [["explain"], ["explanation"], ["why"]] },
  {
    type: "dontknow",
    phrases: [
      ["i", "don't", "know"],
      ["i", "dont", "know"],
      ["don't", "know"],
      ["dont", "know"],
      ["do", "not", "know"],
      ["no", "idea"],
      ["i", "have", "no", "idea"],
      ["not", "sure"],
      ["idk"],
    ],
  },
];

const KNOW_PHRASES = [
  ["got", "it"],
  ["i", "know"],
  ["i", "know", "it"],
  ["knew", "it"],
  ["know"],
];

export function answerGrammar(choiceKeys: string[] = ["A", "B", "C", "D"]): string {
  const answers = ["a", "b", "c", "d", "alpha", "alfa", "bravo", "charlie", "charley", "delta", "true", "false"]
    .filter((token) => {
      const key = STRONG[token] ?? LETTER[token];
      return key ? choiceKeys.includes(key) : false;
    });
  const words = [
    ...answers,
    "repeat",
    "skip",
    "next",
    "explain",
    "pause",
    "stop",
    "i don't know",
    "i dont know",
  ];
  return `#JSGF V1.0;\ngrammar drive;\npublic <command> = ${words.join(" | ")};`;
}

export function parseVoiceCommand(transcript: string, options: ParseOptions = {}): VoiceCommand {
  const choiceKeys = options.choiceKeys ?? ["A", "B", "C", "D"];
  const allowed = new Set(choiceKeys);
  const words = tokenize(transcript);
  if (words.length === 0) return { type: "unrecognized" };

  for (const command of COMMANDS) {
    if (containsPhrase(words, command.phrases)) return { type: command.type };
  }
  if (options.allowKnow && containsPhrase(words, KNOW_PHRASES)) return { type: "know" };

  const fromPattern = answerFromPatterns(words, allowed);
  if (fromPattern) return { type: "answer", key: fromPattern };

  let last: string | null = null;
  for (const word of words) {
    const key = STRONG[word];
    if (key && allowed.has(key)) last = key;
  }
  if (last) return { type: "answer", key: last };

  if (words.length === 1) {
    const only = words[0];
    const key = allowedKey(LETTER[only] ?? WEAK[only], allowed);
    if (key) return { type: "answer", key };
  }

  return { type: "unrecognized" };
}

function answerFromPatterns(words: string[], allowed: Set<string>): string | null {
  const text = words.join(" ");
  const lead = "(?:answer|option|letter|choice)";
  const token = "(a|b|c|d|t|f|ay|eh|be|bee|see|sea|cee|dee|alpha|alfa|bravo|charlie|charley|delta|true|false)";
  const patterns = [
    new RegExp(`^(?:the\\s+)?${lead}\\s+(?:is\\s+)?${token}$`),
    new RegExp(`${lead}\\s+is\\s+${token}$`),
  ];
  for (const pattern of patterns) {
    const match = text.match(pattern);
    if (!match) continue;
    const key = allowedKey(STRONG[match[1]] ?? WEAK[match[1]] ?? LETTER[match[1]], allowed);
    if (key) return key;
  }
  return null;
}

function allowedKey(key: string | undefined, allowed: Set<string>): string | null {
  if (!key || !allowed.has(key)) return null;
  return key;
}

function tokenize(raw: string): string[] {
  return raw
    .toLowerCase()
    .normalize("NFC")
    .replace(/[^a-z0-9'\s]/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .split(" ")
    .filter(Boolean);
}

function containsPhrase(words: string[], phrases: string[][]): boolean {
  return phrases.some((phrase) => {
    for (let i = 0; i <= words.length - phrase.length; i++) {
      let found = true;
      for (let j = 0; j < phrase.length; j++) {
        if (words[i + j] !== phrase[j]) {
          found = false;
          break;
        }
      }
      if (found) return true;
    }
    return false;
  });
}
