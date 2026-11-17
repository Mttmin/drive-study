#!/usr/bin/env node
/**
 * Write podcast/feed.xml and podcast/index.html into the Pages output.
 *
 * Expects dist/audio/podcast/catalog.json plus the mp3s next to it.
 * The catalog is produced by `make audio` on the audio branch.
 *
 *   node scripts/build-podcast-feed.mjs [distDir]
 */
import { readFileSync, writeFileSync, existsSync, mkdirSync, statSync } from "node:fs";
import { join, dirname } from "node:path";

const origin = (process.env.DRIVE_STUDY_ORIGIN || "https://mttmin.github.io/drive-study").replace(
  /\/$/,
  "",
);
const dist = process.argv[2] || "dist";
const catalogPath = join(dist, "audio/podcast/catalog.json");
const coverPath = "audio/podcast/cover.jpg";

if (!existsSync(catalogPath)) {
  console.log("No audio/podcast/catalog.json. Skipping the podcast feed.");
  process.exit(0);
}

const catalog = JSON.parse(readFileSync(catalogPath, "utf8"));
const episodes = (catalog.episodes || []).filter((episode) => {
  const file = join(dist, "audio/podcast", episode.file);
  return existsSync(file) && statSync(file).size > 200;
});

const feedUrl = `${origin}/podcast/feed.xml`;
const coverUrl = `${origin}/${coverPath}`;
const subscribeUrl = `podcast://${feedUrl.slice(feedUrl.indexOf("://") + 3)}`;

function esc(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

function hhmmss(seconds) {
  const total = Math.max(0, Math.round(Number(seconds) || 0));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  return [hours, minutes, secs].map((n) => String(n).padStart(2, "0")).join(":");
}

const items = episodes
  .map((episode) => {
    const filePath = join(dist, "audio/podcast", episode.file);
    const bytes = statSync(filePath).size;
    const url = `${origin}/audio/podcast/${episode.file}`;
    return `    <item>
      <title>${esc(episode.title)}</title>
      <description>${esc(episode.description)}</description>
      <itunes:summary>${esc(episode.description)}</itunes:summary>
      <itunes:author>Matthieu Minguet</itunes:author>
      <itunes:explicit>false</itunes:explicit>
      <itunes:duration>${hhmmss(episode.seconds)}</itunes:duration>
      <itunes:image href="${esc(coverUrl)}"/>
      <enclosure url="${esc(url)}" length="${bytes}" type="audio/mpeg"/>
      <guid isPermaLink="false">${esc(episode.guid)}</guid>
      <pubDate>${esc(episode.pubDate)}</pubDate>
    </item>`;
  })
  .join("\n");

const xml = `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd" xmlns:atom="http://www.w3.org/2005/Atom">
  <channel>
    <title>Drive Study</title>
    <link>${origin}/</link>
    <language>en-us</language>
    <description>Narrated SIE and Series 7 lessons, plus audio quizzes with a pause before each answer. Original study audio for personal use. Not affiliated with FINRA.</description>
    <lastBuildDate>${new Date().toUTCString()}</lastBuildDate>
    <generator>drive-study scripts/build-podcast-feed.mjs</generator>
    <atom:link href="${feedUrl}" rel="self" type="application/rss+xml"/>
    <itunes:author>Matthieu Minguet</itunes:author>
    <itunes:summary>SIE and Series 7 audio lessons and quizzes, built for listening in the car.</itunes:summary>
    <itunes:explicit>false</itunes:explicit>
    <itunes:type>episodic</itunes:type>
    <itunes:owner><itunes:name>Matthieu Minguet</itunes:name></itunes:owner>
    <itunes:image href="${coverUrl}"/>
    <itunes:category text="Education"><itunes:category text="Courses"/></itunes:category>
    <image>
      <url>${coverUrl}</url>
      <title>Drive Study</title>
      <link>${origin}/</link>
    </image>
${items}
  </channel>
</rss>
`;

mkdirSync(join(dist, "podcast"), { recursive: true });
writeFileSync(join(dist, "podcast/feed.xml"), xml);
writeFileSync(
  join(dist, "podcast/index.html"),
  `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Drive Study podcast</title>
  <link rel="alternate" type="application/rss+xml" title="Drive Study" href="feed.xml">
  <style>
    :root { color-scheme: dark; }
    body { margin: 0; min-height: 100vh; display: grid; place-items: center; background: #12181f; color: #f4f1ea; font: 18px/1.45 "Liberation Sans", "Noto Sans", sans-serif; }
    main { width: min(36rem, calc(100% - 2rem)); padding: 1.5rem 0 3rem; }
    h1 { font-size: 2rem; letter-spacing: 0.04em; margin: 0 0 0.4rem; }
    p { color: #c5d0dc; }
    a.button { display: block; margin: 1.4rem 0; padding: 1rem 1.1rem; background: #e0a45a; color: #1b140c; text-decoration: none; font-weight: 700; font-size: 1.15rem; border-radius: 14px; text-align: center; }
    code { word-break: break-all; color: #f4f1ea; }
  </style>
</head>
<body>
  <main>
    <h1>Drive Study</h1>
    <p>SIE and Series 7 lessons and audio quizzes. Subscribe once and Apple Podcasts downloads new episodes for the car.</p>
    <a class="button" href="${subscribeUrl}">Subscribe in Apple Podcasts</a>
    <p>Or add this feed by URL:</p>
    <p><code>${feedUrl}</code></p>
  </main>
</body>
</html>
`,
);
console.log(`Wrote ${episodes.length} episodes to ${join(dist, "podcast/feed.xml")}`);
console.log(subscribeUrl);
