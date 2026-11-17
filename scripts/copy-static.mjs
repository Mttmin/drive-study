import { cpSync, mkdirSync, rmSync } from "node:fs";

mkdirSync("dist", { recursive: true });
rmSync("dist/content", { recursive: true, force: true });
rmSync("dist/audio", { recursive: true, force: true });
cpSync("content", "dist/content", { recursive: true });
cpSync("audio", "dist/audio", { recursive: true });
console.log("Copied content and audio into dist.");
