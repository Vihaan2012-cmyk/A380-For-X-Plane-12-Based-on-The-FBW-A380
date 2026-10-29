//! The OIT's OPS LIBRARY and EXPORT BOX: the aircraft's `Documents` folder
//! (the EFB's My Documents) read as a library, and exports written to
//! `Documents/Exports` so the EFB lists them too.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

const MAX_TEXT: u64 = 2 * 1024 * 1024;
const MAX_EXPORT: usize = 512 * 1024;

fn documents_root() -> Option<PathBuf> {
    Some(crate::sound_pack::plugin_dir()?.parent()?.parent()?.join("Documents"))
}

pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn safe_relative(rel: &str) -> bool {
    !rel.is_empty() && !rel.contains(['\\', ':']) && !rel.split('/').any(|p| p.is_empty() || p == "." || p == "..")
}

fn kind_of(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_string_lossy().to_ascii_lowercase().as_str() {
        "md" | "txt" => Some("text"),
        "pdf" => Some("pdf"),
        "png" | "jpg" | "jpeg" | "webp" | "gif" => Some("image"),
        _ => None,
    }
}

fn walk(dir: &Path, rel: &str, depth: usize, out: &mut Vec<Value>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir).map(|r| r.flatten().collect()).unwrap_or_default();
    entries.sort_by_key(|e| e.file_name().to_string_lossy().to_lowercase());
    for e in entries {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let child = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
        let path = e.path();
        if path.is_dir() {
            if depth < 6 {
                walk(&path, &child, depth + 1, out);
            }
        } else if let Some(kind) = kind_of(&path) {
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            out.push(json!({ "path": child, "name": path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or(name), "folder": rel, "type": kind, "bytes": size }));
        }
    }
}

pub fn tree_json_in(root: &Path) -> String {
    let mut files = Vec::new();
    walk(root, "", 0, &mut files);
    json!({ "files": files }).to_string()
}

pub fn file_text_in(root: &Path, rel: &str) -> Option<String> {
    if !safe_relative(rel) {
        return None;
    }
    let path = root.join(rel);
    if kind_of(&path) != Some("text") || std::fs::metadata(&path).ok()?.len() > MAX_TEXT {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

pub fn file_bytes_in(root: &Path, rel: &str) -> Option<(&'static str, Vec<u8>)> {
    if !safe_relative(rel) {
        return None;
    }
    let path = root.join(rel);
    let mime = match path.extension()?.to_string_lossy().to_ascii_lowercase().as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => return None,
    };
    Some((mime, std::fs::read(path).ok()?))
}

pub fn file_bytes(query: &str) -> Option<(&'static str, Vec<u8>)> {
    let rel = query.split('&').find_map(|kv| kv.strip_prefix("path=")).map(percent_decode)?;
    file_bytes_in(&documents_root()?, &rel)
}

fn export_name(name: &str) -> Option<String> {
    let clean: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || " -_.".contains(c) { c } else { '_' }).collect();
    let clean = clean.trim().trim_matches('.').to_string();
    if clean.is_empty() || clean.len() > 80 {
        return None;
    }
    Some(if clean.to_ascii_lowercase().ends_with(".txt") { clean } else { format!("{clean}.txt") })
}

pub fn export_in(root: &Path, body: &str) -> Result<String, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let name = v.get("name").and_then(Value::as_str).and_then(export_name).ok_or("bad name")?;
    let text = v.get("text").and_then(Value::as_str).ok_or("missing text")?;
    if text.len() > MAX_EXPORT {
        return Err("too large".into());
    }
    let dir = root.join("Exports");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(&name), text).map_err(|e| e.to_string())?;
    Ok(format!("Exports/{name}"))
}

pub fn route(method: &str, path: &str, query: &str, body: &str) -> Option<(u16, String)> {
    if !path.starts_with("/oit/") {
        return None;
    }
    let Some(root) = documents_root() else { return Some((503, "{\"error\":\"no aircraft folder\"}".into())) };
    Some(match (method.to_ascii_uppercase().as_str(), path) {
        ("GET", "/oit/docs") => (200, tree_json_in(&root)),
        ("GET", "/oit/doc") => {
            let rel = query.split('&').find_map(|kv| kv.strip_prefix("path=")).map(percent_decode).unwrap_or_default();
            match file_text_in(&root, &rel) {
                Some(text) => (200, json!({ "path": rel, "text": text }).to_string()),
                None => (404, "{\"error\":\"no such document\"}".into()),
            }
        }
        ("GET", "/oit/charts") => {
            let arg = |k: &str| query.split('&').find_map(|kv| kv.strip_prefix(k)).unwrap_or("");
            let icao: String = arg("icao=").chars().filter(char::is_ascii_alphanumeric).take(4).collect::<String>().to_ascii_uppercase();
            if icao.len() != 4 {
                return Some((400, json!({ "error": "an ICAO code has four letters" }).to_string()));
            }
            let v = if arg("source=") == "faa" { crate::charts::faa(&icao) } else { crate::charts::local(&root, &icao, "/oit/file?path=") };
            (200, v.to_string())
        }
        ("POST", "/oit/export") => match export_in(&root, body) {
            Ok(rel) => (200, json!({ "ok": true, "path": rel }).to_string()),
            Err(e) => (400, json!({ "ok": false, "message": e }).to_string()),
        },
        _ => (404, "{}".into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("fbw-oit-files-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("Charts")).unwrap();
        d
    }

    #[test]
    fn the_library_lists_documents_and_reads_only_text_inside_the_folder() {
        let d = temp();
        std::fs::write(d.join("Notes.md"), "# Notes").unwrap();
        std::fs::write(d.join("Charts/gate.png"), [0u8; 4]).unwrap();
        std::fs::write(d.join("skip.exe"), [0u8; 4]).unwrap();
        let tree: Value = serde_json::from_str(&tree_json_in(&d)).unwrap();
        let paths: Vec<&str> = tree["files"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap()).collect();
        assert_eq!(paths, ["Charts/gate.png", "Notes.md"]);
        assert_eq!(file_text_in(&d, "Notes.md").as_deref(), Some("# Notes"));
        assert!(file_text_in(&d, "Charts/gate.png").is_none());
        assert!(file_text_in(&d, "../secret.md").is_none());
        assert!(file_text_in(&d, "C:/x.md").is_none());
        assert_eq!(file_bytes_in(&d, "Charts/gate.png").map(|(m, b)| (m, b.len())), Some(("image/png", 4)));
        assert!(file_bytes_in(&d, "skip.exe").is_none());
        assert!(file_bytes_in(&d, "../Charts/gate.png").is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_export_lands_in_exports_with_a_safe_name() {
        let d = temp();
        assert_eq!(export_in(&d, r#"{"name":"LOADSHEET FINAL","text":"hello"}"#).unwrap(), "Exports/LOADSHEET FINAL.txt");
        assert_eq!(std::fs::read_to_string(d.join("Exports/LOADSHEET FINAL.txt")).unwrap(), "hello");
        assert_eq!(export_in(&d, r#"{"name":"../../evil","text":"x"}"#).unwrap(), "Exports/_.._evil.txt");
        assert!(export_in(&d, r#"{"name":"","text":"x"}"#).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn percent_decoding_reads_spaces_and_bytes() {
        assert_eq!(percent_decode("My%20Notes%2Fa.md"), "My Notes/a.md");
        assert_eq!(percent_decode("a+b"), "a b");
        assert_eq!(percent_decode("bad%zz"), "bad%zz");
    }
}
