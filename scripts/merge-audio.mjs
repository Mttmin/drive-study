import { execSync } from "node:child_process";

function run(command) {
  return execSync(command, { stdio: ["ignore", "pipe", "pipe"] }).toString();
}

try {
  run("git rev-parse --is-inside-work-tree");
} catch {
  console.log("Not a git checkout. Keeping the audio stub.");
  process.exit(0);
}

try {
  execSync("git fetch origin audio --depth=1", { stdio: "inherit" });
} catch {
  console.log("Could not fetch origin/audio. Keeping the audio stub.");
  process.exit(0);
}

try {
  run("git cat-file -e origin/audio:audio/manifest.json");
} catch {
  console.log("origin/audio has no audio/manifest.json. Keeping the audio stub.");
  process.exit(0);
}

execSync("git archive origin/audio audio | tar -x -C dist", { stdio: "inherit" });
console.log("Merged origin/audio into dist/audio.");
