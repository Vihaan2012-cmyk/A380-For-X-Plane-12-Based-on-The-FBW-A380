//! The EFB page's web routes that need nothing but files and the plugin's
//! library, served by XPHFBW (app/src/web.rs) and by the plugin itself when
//! the screens run in X-Plane's own browser (src/browser_screens).

use std::path::PathBuf;

use serde_json::{json, Value};

pub struct Paths {
    pub xplane_root: PathBuf,
    pub aircraft_dir: PathBuf,
    pub ui_dir: PathBuf,
}

pub struct Reply {
    pub status: &'static str,
    pub kind: &'static str,
    pub body: Vec<u8>,
}

fn found(kind: &'static str, body: Vec<u8>) -> Option<Reply> {
    Some(Reply { status: "200 OK", kind, body })
}

fn missing(text: &str) -> Option<Reply> {
    Some(Reply { status: "404 Not Found", kind: "text/plain", body: text.as_bytes().to_vec() })
}

fn json_reply(value: &Value) -> Option<Reply> {
    found("application/json", value.to_string().into_bytes())
}

fn arg(query: &str, key: &str) -> String {
    query.split('&').find_map(|kv| kv.strip_prefix(key)).map(percent_decode).unwrap_or_default()
}

pub fn documents_root(paths: &Paths) -> PathBuf {
    paths.aircraft_dir.join("Documents")
}

/// A route of the page, or `None` when it is not one of these.
pub fn route(paths: &Paths, method: &str, path: &str, query: &str, body: &[u8]) -> Option<Reply> {
    match (method, path) {
        ("GET", "/xplane12-logo.png") => {
            let logo = paths.xplane_root.join("Resources").join("bitmaps").join("interface11").join("logo@2x.png");
            match std::fs::read(logo) {
                Ok(png) => found("image/png", png),
                Err(_) => missing("no logo"),
            }
        }
        ("GET", "/api/settings") => json_reply(&Value::Object(crate::efb_settings::load(&paths.xplane_root))),
        ("POST", "/api/settings") => {
            let values = serde_json::from_slice::<Value>(body).ok().and_then(|v| v.as_object().cloned()).unwrap_or_default();
            match crate::efb_settings::save(&paths.xplane_root, &values) {
                Ok(()) => json_reply(&json!({ "ok": true })),
                Err(e) => json_reply(&json!({ "ok": false, "message": e })),
            }
        }
        ("GET", p) if p.starts_with("/fonts/") => {
            let name = percent_decode(&p["/fonts/".len()..]);
            let ok = !name.is_empty() && !name.contains(['/', '\\', ':']) && !name.contains("..") && (name.ends_with(".ttf") || name.ends_with(".otf"));
            let file = paths.aircraft_dir.join("html_ui").join("Fonts").join("fbw-a380x").join("EFB").join(&name);
            match if ok { std::fs::read(file).ok() } else { None } {
                Some(font) => found(if name.ends_with(".otf") { "font/otf" } else { "font/ttf" }, font),
                None => missing("no such font"),
            }
        }
        ("GET", p) if p.starts_with("/ecam/") => {
            let name = percent_decode(&p["/ecam/".len()..]);
            let ok = !name.is_empty() && !name.contains(['\\', ':']) && !name.contains("..") && name.ends_with(".webp") && name.matches('/').count() <= 1;
            let file = paths.ui_dir.join("ecam").join(&name);
            match if ok { std::fs::read(file).ok() } else { None } {
                Some(img) => found("image/webp", img),
                None => missing("no such image"),
            }
        }
        ("GET", "/api/documents") => json_reply(&documents_tree(&documents_root(paths))),
        ("GET", "/api/document") => match read_document(&documents_root(paths), &arg(query, "path=")) {
            Some(text) => found("text/markdown; charset=utf-8", text.into_bytes()),
            None => missing("no such document"),
        },
        ("GET", "/api/charts") => {
            let icao = arg(query, "icao=").trim().to_ascii_uppercase();
            json_reply(&if icao.len() != 4 || !icao.chars().all(|c| c.is_ascii_alphanumeric()) {
                json!({ "error": "Enter a four-letter ICAO code" })
            } else if arg(query, "source=") == "faa" {
                crate::charts::faa(&icao)
            } else {
                crate::charts::local(&documents_root(paths), &icao, "/api/docfile?path=")
            })
        }
        ("GET", "/api/open") => {
            let url = arg(query, "url=");
            if url.starts_with("https://") && !url.contains(['"', ' ', '\n']) {
                open(&url);
                json_reply(&json!({ "ok": true }))
            } else {
                json_reply(&json!({ "ok": false }))
            }
        }
        ("GET", "/api/nearest") => {
            let num = |k: &str| query.split('&').find_map(|kv| kv.strip_prefix(k)).and_then(|v| v.parse::<f64>().ok()).unwrap_or(f64::NAN);
            json_reply(&crate::runways::nearest(&[paths.aircraft_dir.as_path(), paths.ui_dir.as_path()], num("lat="), num("lon=")))
        }
        ("GET", "/api/runways") => json_reply(&crate::runways::lookup(&[paths.aircraft_dir.as_path(), paths.ui_dir.as_path()], &arg(query, "icao="))),
        ("GET", "/api/docfile") => match document_file(&documents_root(paths), &arg(query, "path=")) {
            Some((kind, bytes)) => found(kind, bytes),
            None => missing("no such file"),
        },
        ("GET", "/study/pages") => found("application/json", crate::study_json::pages().into_bytes()),
        ("GET", "/study/failures") => found("application/json", crate::study_json::failures().into_bytes()),
        ("GET", "/study/breakers") => found("application/json", crate::study_json::breakers().into_bytes()),
        ("GET", "/study/components") => found("application/json", crate::study_json::components().into_bytes()),
        ("GET", "/study/maintenance") => found("application/json", crate::study_json::maintenance().into_bytes()),
        ("GET", "/study/mel") => found("application/json", crate::study_json::mel(query).into_bytes()),
        _ => None,
    }
}

const MARKDOWN_GUIDE: &str = "# Markdown Guide\nPut `.md` files, pictures (`.png`, `.jpg`, `.webp`) and PDFs in the aircraft's Documents folder. Folders become folders under My Documents.\n\n## Headings\n```\n# Heading 1\n## Heading 2\n### Heading 3\n```\n\n## Styling Text\n**Bold text**, *italic text*, ~~strikethrough~~ and `inline code`.\n\n## Paragraphs and Line Breaks\nLeave a blank line between paragraphs. Lines next to each other stay in one paragraph on separate lines.\n\n## Lists\n### Bulleted List\n- First item\n- Second item\n  - Nested item\n\n### Numbered List\n1. Step one\n2. Step two\n\n## Quotes\n> Quotes start with a `>`.\n\n## Links\n### Link Within the Same Document\n[Back to Headings](#headings)\n\n### Link to Another Document\n`[Open my notes](Notes/My notes.md)` opens another document from the Documents folder.\n\n### External Link\n`[SimBrief](https://www.simbrief.com)`\n\n## Images\n`![Gate chart](Charts/gate.png)` shows a picture stored next to the document.\n\n## Code Blocks\n```\nThree backticks before and after keep the text exactly as written.\n```\n\n## Tables\n| Item | Value |\n|------|-------|\n| V1 | 142 kt |\n| VR | 150 kt |\n\n## EFB Markdown\n### Collapsible Sections\n```\n::: details Aircraft\nText inside the section.\n:::\n```\n\n::: details Aircraft\nRegistration, SELCAL and anything else you want folded away.\n:::\n\n### Normal Checklists\nWrite a checklist line as `- [ ] ITEM ..... VALUE`. Tap a line to tick it.\n\n- [ ] PARKING BRAKE ..... SET\n- [ ] SEAT BELTS ..... ON\n- [ ] BEACON ..... ON\n\n### Checklist Inside a Collapsible Section\n::: details Before Start\n- [ ] DOORS ..... CLOSED\n- [ ] BEACON ..... ON\n:::\n\n### Multiple Collapsible Sections\n::: details Taxi\n- [ ] FLAPS ..... SET\n:::\n\n::: details Line-Up\n- [ ] TCAS ..... TA/RA\n:::\n";

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

fn open(target: &str) {
    let opener = if cfg!(windows) { "explorer" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    let _ = std::process::Command::new(opener).arg(target).spawn();
}

#[cfg(windows)]
fn select_in_explorer(path: &std::path::Path) {
    use std::os::windows::process::CommandExt;
    let _ = std::process::Command::new("explorer").raw_arg(format!("/select,\"{}\"", path.display())).spawn();
}

#[cfg(not(windows))]
fn select_in_explorer(path: &std::path::Path) {
    open(&path.to_string_lossy());
}

pub fn action(paths: &Paths, name: &str, restart_displays: &dyn Fn()) -> Value {
    let root = &paths.xplane_root;
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
            let dumps = crate::state_dump::root();
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
            restart_displays();
            json!({ "ok": true, "message": "Restarting the instrument displays" })
        }
        "navigraph" => {
            json!({ "ok": false, "message": "Navigraph account linking isn't available yet; set a SimBrief user ID above instead to use INIT > CPNY F-PLN REQUEST" })
        }
        "installed-version" => {
            let version = std::fs::read_to_string(paths.aircraft_dir.join("fbw_xp_installed_version.txt")).ok().map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
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
