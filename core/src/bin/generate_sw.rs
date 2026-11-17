use std::fs;
use std::path::Path;
use std::process::ExitCode;

use sha1::{Digest, Sha1};

fn main() -> ExitCode {
    let dist = std::env::args().nth(1).unwrap_or_else(|| "dist".into());
    let root = Path::new(&dist);
    if !root.join("index.html").is_file() {
        eprintln!("No {dist}/index.html. Build the site first.");
        return ExitCode::from(1);
    }
    let mut files = Vec::new();
    walk(root, root, &mut files);
    files.sort();
    let mut hasher = Sha1::new();
    let mut urls = Vec::new();
    for rel in &files {
        if rel == "sw.js" || rel.starts_with("audio/clips/") || rel.starts_with("audio/podcast/") {
            continue;
        }
        let path = root.join(rel);
        if let Ok(meta) = fs::metadata(&path) {
            hasher.update(rel.as_bytes());
            hasher.update(meta.len().to_le_bytes());
            urls.push(format!("/drive-study/{rel}"));
        }
    }
    let stamp = hasher.finalize();
    let stamp = format!("{:02x}{:02x}{:02x}{:02x}", stamp[0], stamp[1], stamp[2], stamp[3]);
    let list = urls
        .iter()
        .map(|url| format!("\"{url}\""))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!(
        r#"const CACHE="drive-study-shell-{stamp}";
const PRECACHE=[{list}];
self.addEventListener("install",(event)=>{{event.waitUntil(caches.open(CACHE).then((cache)=>cache.addAll(PRECACHE)));self.skipWaiting();}});
self.addEventListener("activate",(event)=>{{event.waitUntil(caches.keys().then((keys)=>Promise.all(keys.filter((key)=>key.startsWith("drive-study-shell-")&&key!==CACHE).map((key)=>caches.delete(key)))));self.clients.claim();}});
self.addEventListener("fetch",(event)=>{{
  const url=new URL(event.request.url);
  if(url.origin!==location.origin) return;
  if(url.pathname.includes("/audio/clips/")||url.pathname.includes("/audio/podcast/")){{
    event.respondWith(caches.open("drive-study-audio-v1").then(async (cache)=>{{
      const hit=await cache.match(event.request);
      if(hit) return hit;
      const response=await fetch(event.request);
      if(response.ok && event.request.method==="GET") cache.put(event.request, response.clone());
      return response;
    }}));
    return;
  }}
  if(event.request.mode==="navigate"){{
    event.respondWith(fetch(event.request).catch(()=>caches.match("/drive-study/index.html")));
    return;
  }}
  event.respondWith(caches.match(event.request).then((hit)=>hit||fetch(event.request)));
}});
"#
    );
    if fs::write(root.join("sw.js"), source).is_err() {
        eprintln!("Could not write {dist}/sw.js");
        return ExitCode::from(1);
    }
    println!("Wrote {dist}/sw.js with {} precached files.", urls.len());
    ExitCode::SUCCESS
}

fn walk(dir: &Path, root: &Path, acc: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, root, acc);
        } else if let Ok(rel) = path.strip_prefix(root) {
            acc.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}
