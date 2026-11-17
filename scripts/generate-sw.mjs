import { generateSW } from "workbox-build";

const base = "/drive-study/";

const { count, size, warnings } = await generateSW({
  globDirectory: "dist",
  globPatterns: ["**/*.{js,css,html,svg,png,webmanifest,json,xml,txt}"],
  globIgnores: ["**/audio/clips/**", "**/audio/podcast/**"],
  swDest: "dist/sw.js",
  navigateFallback: `${base}index.html`,
  navigateFallbackDenylist: [/\/audio\//, /\/podcast\//],
  modifyURLPrefix: { "": base },
  maximumFileSizeToCacheInBytes: 8 * 1024 * 1024,
  clientsClaim: true,
  skipWaiting: true,
  cleanupOutdatedCaches: true,
  runtimeCaching: [
    {
      urlPattern: /\/audio\/clips\//,
      handler: "CacheFirst",
      options: {
        cacheName: "drive-study-audio-v1",
        cacheableResponse: { statuses: [200] },
        rangeRequests: true,
        expiration: { maxEntries: 8000, maxAgeSeconds: 60 * 60 * 24 * 365 },
      },
    },
  ],
});

for (const warning of warnings) console.warn(warning);
console.log(`Precached ${count} files (${size} bytes).`);
