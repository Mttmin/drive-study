/// <reference types="vite/client" />

interface SpeechRecognitionAlternative {
  transcript: string;
}

interface SpeechRecognitionResult {
  isFinal: boolean;
  length: number;
  [index: number]: SpeechRecognitionAlternative;
}

interface SpeechRecognitionResultList {
  length: number;
  [index: number]: SpeechRecognitionResult;
}

interface SpeechRecognitionEvent extends Event {
  resultIndex: number;
  results: SpeechRecognitionResultList;
}

interface SpeechRecognition extends EventTarget {
  lang: string;
  continuous: boolean;
  interimResults: boolean;
  maxAlternatives: number;
  grammars: SpeechGrammarList;
  start(): void;
  stop(): void;
  abort(): void;
  onresult: ((event: SpeechRecognitionEvent) => void) | null;
  onerror: ((event: Event & { error?: string }) => void) | null;
  onend: (() => void) | null;
}

interface SpeechRecognitionCtor {
  new (): SpeechRecognition;
}

interface SpeechGrammarList {
  addFromString(grammar: string, weight: number): void;
}

interface SpeechGrammarListCtor {
  new (): SpeechGrammarList;
}

declare global {
  interface Window {
    SpeechRecognition?: SpeechRecognitionCtor;
    webkitSpeechRecognition?: SpeechRecognitionCtor;
    SpeechGrammarList?: SpeechGrammarListCtor;
    webkitSpeechGrammarList?: SpeechGrammarListCtor;
  }
}

export {};
