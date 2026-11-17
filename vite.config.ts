import { defineConfig } from "vite";
import preact from "@preact/preset-vite";
import { createReadStream, existsSync, statSync } from "node:fs";
import { join, normalize } from "node:path";

const root = fileUrl();

export default defineConfig({
  base: "/drive-study/",
  plugins: [
    preact(),
    {
      name: "serve-study-files",
      configureServer(server) {
        server.middlewares.use((req, res, next) => {
          const url = req.url?.split("?")[0] ?? "";
          const marker = "/drive-study/";
          const at = url.indexOf(marker);
          if (at === -1) return next();
          const rel = decodeURIComponent(url.slice(at + marker.length));
          if (!rel.startsWith("content/") && !rel.startsWith("audio/") && !rel.startsWith("podcast/")) return next();
          const file = normalize(join(root, rel));
          if (!file.startsWith(root) || !existsSync(file) || !statSync(file).isFile()) return next();
          res.setHeader("content-type", file.endsWith(".json") ? "application/json; charset=utf-8" : file.endsWith(".xml") ? "application/xml" : "application/octet-stream");
          createReadStream(file).pipe(res);
        });
      },
    },
  ],
  test: {
    environment: "node",
    include: ["tests/**/*.test.ts"],
  },
});

function fileUrl(): string {
  return new URL(".", import.meta.url).pathname;
}
