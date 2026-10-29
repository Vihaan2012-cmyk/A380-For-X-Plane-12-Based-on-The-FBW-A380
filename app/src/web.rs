//! The local web server the settings page talks to: the page itself, and
//! JSON for settings, actions and status. Bound to 127.0.0.1 on a free port.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Map, Value};

use crate::Shared;

pub fn start(shared: Arc<Shared>) {
    let Ok(listener) = TcpListener::bind(("127.0.0.1", 0)) else { return };
    if let Ok(addr) = listener.local_addr() {
        shared.port.store(addr.port(), Ordering::Relaxed);
    }
    let _ = std::thread::Builder::new().name("settings server".into()).spawn(move || {
        for stream in listener.incoming().flatten() {
            let s = shared.clone();
            let _ = std::thread::Builder::new().spawn(move || {
                let _ = serve(stream, &s);
            });
        }
    });
}

fn serve(mut stream: TcpStream, shared: &Shared) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    // Headers, then as much body as Content-Length says.
    let (head_end, content_length) = loop {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..i]).to_ascii_lowercase();
            let len = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length:").and_then(|v| v.trim().parse::<usize>().ok()))
                .unwrap_or(0);
            break (i + 4, len);
        }
        if buf.len() > 1 << 20 {
            return Ok(());
        }
    };
    while buf.len() < head_end + content_length {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let body = &buf[head_end..(head_end + content_length).min(buf.len())];
    let mut first = head.lines().next().unwrap_or("").split_whitespace();
    let (method, target) = (first.next().unwrap_or(""), first.next().unwrap_or("/"));
    let (path, query) = target.split_once('?').unwrap_or((target, ""));

    match (method, path) {
        ("GET", "/") | ("GET", "/index.html") => match std::fs::read(shared.ui_dir.join("index.html")) {
            Ok(page) => reply(&mut stream, "200 OK", "text/html; charset=utf-8", &page),
            Err(e) => reply(&mut stream, "500 Internal Server Error", "text/plain", e.to_string().as_bytes()),
        },
        // X-Plane 12's own logo, read from the X-Plane install (not bundled).
        ("GET", "/xplane12-logo.png") => {
            let logo = shared.xplane_root.join("Resources").join("bitmaps").join("interface11").join("logo@2x.png");
            match std::fs::read(logo) {
                Ok(png) => reply(&mut stream, "200 OK", "image/png", &png),
                Err(_) => reply(&mut stream, "404 Not Found", "text/plain", b"no logo"),
            }
        }
        ("GET", "/api/settings") => json_reply(&mut stream, &Value::Object(crate::settings::load(&shared.xplane_root))),
        ("POST", "/api/settings") => {
            let values = serde_json::from_slice::<Value>(body).ok().and_then(|v| v.as_object().cloned()).unwrap_or_default();
            match crate::settings::save(&shared.xplane_root, &values) {
                Ok(()) => json_reply(&mut stream, &json!({ "ok": true })),
                Err(e) => json_reply(&mut stream, &json!({ "ok": false, "message": e })),
            }
        }
        ("GET", "/api/status") => json_reply(&mut stream, &status(shared)),
        ("GET", p) if p.starts_with("/fonts/") => {
            let name = percent_decode(&p["/fonts/".len()..]);
            let ok = !name.is_empty() && !name.contains(['/', '\\', ':']) && !name.contains("..") && (name.ends_with(".ttf") || name.ends_with(".otf"));
            let file = shared.aircraft_dir.join("html_ui").join("Fonts").join("fbw-a380x").join("EFB").join(&name);
            match if ok { std::fs::read(file).ok() } else { None } {
                Some(font) => reply(&mut stream, "200 OK", if name.ends_with(".otf") { "font/otf" } else { "font/ttf" }, &font),
                None => reply(&mut stream, "404 Not Found", "text/plain", b"no such font"),
            }
        }
        ("GET", p) if p.starts_with("/ecam/") => {
            let name = percent_decode(&p["/ecam/".len()..]);
            let ok = !name.is_empty() && !name.contains(['\\', ':']) && !name.contains("..") && name.ends_with(".webp") && name.matches('/').count() <= 1;
            let file = shared.ui_dir.join("ecam").join(&name);
            match if ok { std::fs::read(file).ok() } else { None } {
                Some(img) => reply(&mut stream, "200 OK", "image/webp", &img),
                None => reply(&mut stream, "404 Not Found", "text/plain", b"no such image"),
            }
        }
        ("GET", "/api/documents") => json_reply(&mut stream, &documents_tree(&documents_root(shared))),
        ("GET", "/api/document") => {
            let rel = query.split('&').find_map(|kv| kv.strip_prefix("path=")).map(percent_decode).unwrap_or_default();
            match read_document(&documents_root(shared), &rel) {
                Some(text) => reply(&mut stream, "200 OK", "text/markdown; charset=utf-8", text.as_bytes()),
                None => reply(&mut stream, "404 Not Found", "text/plain", b"no such document"),
            }
        }
        ("GET", "/api/charts") => {
            let arg = |k: &str| query.split('&').find_map(|kv| kv.strip_prefix(k)).map(percent_decode).unwrap_or_default();
            let icao = arg("icao=").trim().to_ascii_uppercase();
            let reply_json = if icao.len() != 4 || !icao.chars().all(|c| c.is_ascii_alphanumeric()) {
                json!({ "error": "Enter a four-letter ICAO code" })
            } else if arg("source=") == "faa" {
                crate::charts::faa(&icao)
            } else {
                crate::charts::local(&documents_root(shared), &icao, "/api/docfile?path=")
            };
            json_reply(&mut stream, &reply_json)
        }
        ("GET", "/api/open") => {
            let url = query.split('&').find_map(|kv| kv.strip_prefix("url=")).map(percent_decode).unwrap_or_default();
            if url.starts_with("https://") && !url.contains(['"', ' ', '\n']) {
                let _ = std::process::Command::new("explorer.exe").arg(&url).spawn();
                json_reply(&mut stream, &json!({ "ok": true }))
            } else {
                json_reply(&mut stream, &json!({ "ok": false }))
            }
        }
        ("GET", "/api/nearest") => {
            let num = |k: &str| query.split('&').find_map(|kv| kv.strip_prefix(k)).and_then(|v| v.parse::<f64>().ok()).unwrap_or(f64::NAN);
            json_reply(&mut stream, &crate::runways::nearest(&[shared.aircraft_dir.as_path(), shared.ui_dir.as_path()], num("lat="), num("lon=")))
        }
        ("GET", "/api/runways") => {
            let icao = query.split('&').find_map(|kv| kv.strip_prefix("icao=")).map(percent_decode).unwrap_or_default();
            json_reply(&mut stream, &crate::runways::lookup(&[shared.aircraft_dir.as_path(), shared.ui_dir.as_path()], &icao))
        }
        ("GET", "/api/docfile") => {
            let rel = query.split('&').find_map(|kv| kv.strip_prefix("path=")).map(percent_decode).unwrap_or_default();
            match document_file(&documents_root(shared), &rel) {
                Some((kind, bytes)) => reply(&mut stream, "200 OK", kind, &bytes),
                None => reply(&mut stream, "404 Not Found", "text/plain", b"no such file"),
            }
        }
        // The Study pages' layouts, from the same Rust the plugin serves them
        // from, for when X-Plane (the plugin's port) is not running.
        ("GET", "/study/pages") => reply(&mut stream, "200 OK", "application/json", fbw_a380_systems::study_json::pages().as_bytes()),
        ("GET", "/study/failures") => reply(&mut stream, "200 OK", "application/json", fbw_a380_systems::study_json::failures().as_bytes()),
        ("GET", "/study/breakers") => reply(&mut stream, "200 OK", "application/json", fbw_a380_systems::study_json::breakers().as_bytes()),
        ("GET", "/study/components") => reply(&mut stream, "200 OK", "application/json", fbw_a380_systems::study_json::components().as_bytes()),
        ("GET", "/study/maintenance") => reply(&mut stream, "200 OK", "application/json", fbw_a380_systems::study_json::maintenance().as_bytes()),
        ("GET", "/study/mel") => reply(&mut stream, "200 OK", "application/json", fbw_a380_systems::study_json::mel(query).as_bytes()),
        ("POST", p) if p.starts_with("/api/action/") => json_reply(&mut stream, &action(shared, &p["/api/action/".len()..])),
        _ => reply(&mut stream, "404 Not Found", "text/plain", b"not found"),
    }
}

const MARKDOWN_GUIDE: &str = "# Markdown Guide\nPut `.md` files, pictures (`.png`, `.jpg`, `.webp`) and PDFs in the aircraft's Documents folder. Folders become folders under My Documents.\n\n## Headings\n```\n# Heading 1\n## Heading 2\n### Heading 3\n```\n\n## Styling Text\n**Bold text**, *italic text*, ~~strikethrough~~ and `inline code`.\n\n## Paragraphs and Line Breaks\nLeave a blank line between paragraphs. Lines next to each other stay in one paragraph on separate lines.\n\n## Lists\n### Bulleted List\n- First item\n- Second item\n  - Nested item\n\n### Numbered List\n1. Step one\n2. Step two\n\n## Quotes\n> Quotes start with a `>`.\n\n## Links\n### Link Within the Same Document\n[Back to Headings](#headings)\n\n### Link to Another Document\n`[Open my notes](Notes/My notes.md)` opens another document from the Documents folder.\n\n### External Link\n`[SimBrief](https://www.simbrief.com)`\n\n## Images\n`![Gate chart](Charts/gate.png)` shows a picture stored next to the document.\n\n## Code Blocks\n```\nThree backticks before and after keep the text exactly as written.\n```\n\n## Tables\n| Item | Value |\n|------|-------|\n| V1 | 142 kt |\n| VR | 150 kt |\n\n## EFB Markdown\n### Collapsible Sections\n```\n::: details Aircraft\nText inside the section.\n:::\n```\n\n::: details Aircraft\nRegistration, SELCAL and anything else you want folded away.\n:::\n\n### Normal Checklists\nWrite a checklist line as `- [ ] ITEM ..... VALUE`. Tap a line to tick it.\n\n- [ ] PARKING BRAKE ..... SET\n- [ ] SEAT BELTS ..... ON\n- [ ] BEACON ..... ON\n\n### Checklist Inside a Collapsible Section\n::: details Before Start\n- [ ] DOORS ..... CLOSED\n- [ ] BEACON ..... ON\n:::\n\n### Multiple Collapsible Sections\n::: details Taxi\n- [ ] FLAPS ..... SET\n:::\n\n::: details Line-Up\n- [ ] TCAS ..... TA/RA\n:::\n";

pub(crate) fn documents_root(shared: &Shared) -> std::path::PathBuf {
    shared.aircraft_dir.join("Documents")
}

fn documents_tree(root: &std::path::Path) -> Value {
    if !root.is_dir() {
        let _ = std::fs::create_dir_all(root);
        let _ = std::fs::write(root.join("Markdown Guide.md"), MARKDOWN_GUIDE);
    } else if std::fs::read_to_string(root.join("Markdown Guide.md")).is_ok_and(|t| t.starts_with("# Markdown Guide\nMarkdown is a simple way")) {
        let _ = std::fs::write(root.join("Markdown Guide.md"), MARKDOWN_GUIDE);
    }
    fn walk(dir: &std::path::Path, rel: &str, depth: usize) -> Vec<Value> {
        let mut entries: Vec<_> = std::fs::read_dir(dir).map(|r| r.flatten().collect()).unwrap_or_default();
        entries.sort_by_key(|e| e.file_name().to_string_lossy().to_lowercase());
        let mut out = Vec::new();
        for e in entries {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let child = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            let path = e.path();
            if path.is_dir() && depth < 6 {
                out.push(json!({ "name": name, "path": child, "kids": walk(&path, &child, depth + 1) }));
            } else if let Some(kind) = path.extension().map(|x| x.to_string_lossy().to_ascii_lowercase()).and_then(|x| match x.as_str() {
                "md" | "txt" => Some("text"),
                "png" | "jpg" | "jpeg" | "webp" | "gif" => Some("image"),
                "pdf" => Some("pdf"),
                _ => None,
            }) {
                let title = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or(name);
                out.push(json!({ "name": title, "path": child, "type": kind }));
            }
        }
        out
    }
    json!({ "root": root.to_string_lossy(), "kids": walk(root, "", 0) })
}

fn read_document(root: &std::path::Path, rel: &str) -> Option<String> {
    let rel = rel.trim_start_matches(['/', '\\']);
    if rel.is_empty() || rel.split(['/', '\\']).any(|p| p == "..") || std::path::Path::new(rel).is_absolute() || rel.contains(':') {
        return None;
    }
    std::fs::read_to_string(root.join(rel)).ok()
}

fn document_file(root: &std::path::Path, rel: &str) -> Option<(&'static str, Vec<u8>)> {
    let rel = rel.trim_start_matches(['/', '\\']);
    if rel.is_empty() || rel.split(['/', '\\']).any(|p| p == "..") || std::path::Path::new(rel).is_absolute() || rel.contains(':') {
        return None;
    }
    let path = root.join(rel);
    let kind = match path.extension()?.to_string_lossy().to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "pdf" => "application/pdf",
        "md" | "txt" => "text/plain; charset=utf-8",
        _ => return None,
    };
    std::fs::read(path).ok().map(|b| (kind, b))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(v);
                    i += 3;
                    continue;
                }
                out.push(b'%');
            }
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn reply(stream: &mut TcpStream, status: &str, kind: &str, body: &[u8]) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)
}

fn json_reply(stream: &mut TcpStream, value: &Value) -> std::io::Result<()> {
    reply(stream, "200 OK", "application/json", value.to_string().as_bytes())
}

fn status(shared: &Shared) -> Value {
    let st = shared.status.lock().unwrap();
    let mut out = Map::new();
    out.insert("xplane".into(), Value::Bool(st.xplane_connected));
    out.insert(
        "systems".into(),
        if st.systems_running {
            // `serve_systems` (main.rs) runs FlyByWire's systems on a thread
            // of this very process; these are its own live counters
            // (src/remote/server.rs), not a round trip to another process.
            use fbw_a380_systems::remote::server::{LAST_TICK_MICROS, TICKS_SERVED, VARIABLE_COUNT};
            json!({
                "ticks": TICKS_SERVED.load(Ordering::Relaxed),
                "lastTickMicros": LAST_TICK_MICROS.load(Ordering::Relaxed),
                "variables": VARIABLE_COUNT.load(Ordering::Relaxed),
            })
        } else {
            Value::Null
        },
    );
    let screens = crate::views::LIVE_SCREENS.load(Ordering::Relaxed);
    out.insert(
        "displays".into(),
        if screens > 0 { json!({ "screens": screens, "fps": crate::views::SCREEN_FPS.load(Ordering::Relaxed) }) } else { Value::Null },
    );
    if let Some(e) = &st.systems_error {
        out.insert("systemsError".into(), Value::String(e.clone()));
    }
    out.insert("cabinIncluded".into(), Value::Bool(shared.aircraft_dir.join("objects").join("cabin_included.txt").is_file()));
    Value::Object(out)
}

fn open(target: &str) {
    let _ = std::process::Command::new("explorer").arg(target).spawn();
}

fn select_in_explorer(path: &std::path::Path) {
    use std::os::windows::process::CommandExt;
    let _ = std::process::Command::new("explorer").raw_arg(format!("/select,\"{}\"", path.display())).spawn();
}

fn action(shared: &Shared, name: &str) -> Value {
    let root = &shared.xplane_root;
    match name {
        "open-logs" => {
            let log = root.join("Log.txt");
            if !log.is_file() {
                return json!({ "ok": false, "message": format!("No Log.txt in {}", root.display()) });
            }
            select_in_explorer(&log);
            json!({ "ok": true, "message": "Opened X-Plane's Log.txt" })
        }
        "open-dumps" => {
            let dumps = fbw_a380_systems::state_dump::root();
            let _ = std::fs::create_dir_all(&dumps);
            open(&dumps.to_string_lossy());
            json!({ "ok": true, "message": "Opened the state dumps (turn them on in Settings to record)" })
        }
        "report" => {
            open("https://github.com/flybywiresim/aircraft/issues");
            json!({ "ok": true })
        }
        "discord" => {
            open("https://discord.gg/2aUM4E7GQw");
            json!({ "ok": true })
        }
        "restart-displays" => {
            crate::views::restart_displays();
            json!({ "ok": true, "message": "Restarting the instrument displays" })
        }
        "navigraph" => {
            json!({ "ok": false, "message": "Navigraph account linking isn't available yet; set a SimBrief user ID above instead to use INIT > CPNY F-PLN REQUEST" })
        }
        "installed-version" => {
            let version = std::fs::read_to_string(shared.aircraft_dir.join("fbw_xp_installed_version.txt")).ok().map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
            json!({ "ok": version.is_some(), "version": version })
        }
        "run-updater" => {
            let programs = std::env::var("LOCALAPPDATA").map(std::path::PathBuf::from).unwrap_or_default().join("Programs").join("a380x-xp-installer");
            let updater = programs.join("A380 for X-Plane 12.exe");
            if !updater.is_file() {
                return json!({ "ok": false, "message": "The A380 for X-Plane 12 installer is not installed on this computer" });
            }
            match std::process::Command::new(&updater).spawn() {
                Ok(_) => json!({ "ok": true, "message": "The installer is open: update from there after closing X-Plane" }),
                Err(e) => json!({ "ok": false, "message": format!("Could not start the installer: {e}") }),
            }
        }
        "restart-all" | "restart-systems" | "reset-airframe" | "licenses" => {
            json!({ "ok": false, "message": "Not available yet in this build" })
        }
        other => json!({ "ok": false, "message": format!("Unknown action {other}") }),
    }
}
