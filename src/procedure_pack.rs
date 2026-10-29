//! Procedure steps for the ECAM procedures FlyByWire ships without any,
//! kept outside the aircraft: a small pack fetched at start, saved when it changed, into
//! `plugins/fbw_a380_systems/local/`, plus whatever the user built from
//! their own manual (`fcom_procedures.json`, which wins where both have an
//! entry). Without either file the pages stay as FlyByWire left them.

use serde_json::{Map, Value};

const PACK_BASE: &str = "https://raw.githubusercontent.com/Vihaan2012-cmyk/A380-XP12-ECAM-Procedures/main/";
pub const PACK_FILE: &str = "ecam_procedures.json";
pub const OWN_FILE: &str = "fcom_procedures.json";

fn local_dir() -> Option<std::path::PathBuf> {
    crate::sound_pack::plugin_dir().map(|d| d.join("local"))
}

fn entries(text: &str) -> Map<String, Value> {
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(map)) => map.into_iter().filter(|(_, v)| v.get("i").is_some_and(Value::is_array)).collect(),
        _ => Map::new(),
    }
}

pub fn merge(pack: &str, own: &str) -> String {
    let mut out = entries(pack);
    out.extend(entries(own));
    Value::Object(out).to_string()
}

pub fn merged_json() -> String {
    let read = |name: &str| local_dir().and_then(|d| std::fs::read_to_string(d.join(name)).ok()).unwrap_or_default();
    merge(&read(PACK_FILE), &read(OWN_FILE))
}

fn ewd_item(line: &Value) -> Option<Value> {
    let name = line.get(0)?.as_str()?;
    let value = line.get(1).and_then(Value::as_str).unwrap_or("");
    let level = line.get(2).and_then(Value::as_u64).unwrap_or(0);
    let flags = line.get(3).and_then(Value::as_str).unwrap_or("");
    let mut item = Map::new();
    item.insert("name".into(), Value::from(name));
    item.insert("sensed".into(), Value::from(false));
    if !value.is_empty() {
        item.insert("labelNotCompleted".into(), Value::from(value));
    }
    if level > 0 {
        item.insert("level".into(), Value::from(level));
    }
    if flags.contains('c') {
        item.insert("condition".into(), Value::from(true));
    }
    let style = if flags.contains('h') {
        Some("SubHeadline")
    } else if flags.contains('r') {
        Some("LandAsap")
    } else if flags.contains('g') {
        Some("LandAnsa")
    } else if flags.contains('a') {
        Some("Amber")
    } else if flags.contains('b') {
        Some("Cyan")
    } else {
        None
    };
    if let Some(style) = style {
        item.insert("style".into(), Value::from(style));
    }
    Some(Value::Object(item))
}

pub fn ewd_items(merged: &str) -> Map<String, Value> {
    entries(merged)
        .into_iter()
        .filter_map(|(id, p)| {
            let items: Vec<Value> = p.get("i")?.as_array()?.iter().filter_map(ewd_item).collect();
            (!items.is_empty() && id.chars().all(|c| c.is_ascii_digit())).then(|| (id, Value::Array(items)))
        })
        .collect()
}

pub fn source_patches() -> Vec<crate::source_patch::SourcePatch> {
    patches_for(ewd_items(&merged_json()))
}

fn patches_for(items: Map<String, Value>) -> Vec<crate::source_patch::SourcePatch> {
    use crate::deep::ecam::patches::{EWD, PROC_SPREAD_FIND, SYSTEMS_HOST, UPDATE_START_FIND};
    if items.is_empty() {
        return Vec::new();
    }
    let ids: Vec<&String> = items.keys().collect();
    let fill = format!(
        "{PROC_SPREAD_FIND}\n  (function (p, x) {{ for (var k in x) {{ if (p[k] && p[k].items && p[k].items.length === 0) p[k].items = x[k]; }} }})(EcamAbnormalSensedProcedures, {});",
        Value::Object(items.clone())
    );
    let pad = format!(
        "{UPDATE_START_FIND}\n      if (!this.__procedurePackPadded) {{\n        this.__procedurePackPadded = true;\n        try {{\n          var __packIds = {};\n          for (var __i = 0; __i < __packIds.length; __i++) {{\n            var __it = this.ewdAbnormal[__packIds[__i]];\n            var __pr = EcamAbnormalProcedures[__packIds[__i]];\n            if (!__it || !__pr || !__pr.items) continue;\n            (function (it, n) {{\n              var show = it.whichItemsToShow, checked = it.whichItemsChecked;\n              it.whichItemsToShow = function () {{ var a = show ? show.call(it) : []; var o = []; for (var j = 0; j < n; j++) o[j] = j < a.length ? a[j] : true; return o; }};\n              it.whichItemsChecked = function () {{ var a = checked ? checked.call(it) : []; var o = []; for (var j = 0; j < n; j++) o[j] = j < a.length ? a[j] : false; return o; }};\n            }})(__it, __pr.items.length);\n          }}\n        }} catch (e) {{}}\n      }}",
        serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into())
    );
    let patch = |path: &str, find: &str, replace: String, reason: &str| crate::source_patch::SourcePatch {
        path: path.to_string(),
        find: find.to_string(),
        replace,
        reason: reason.to_string(),
    };
    vec![
        patch(EWD, PROC_SPREAD_FIND, fill.clone(), "procedure pack: steps for the ECAM procedures FlyByWire ships empty"),
        patch(SYSTEMS_HOST, PROC_SPREAD_FIND, fill, "procedure pack: steps for the ECAM procedures FlyByWire ships empty"),
        patch(SYSTEMS_HOST, UPDATE_START_FIND, pad, "procedure pack: show and check vectors sized to the filled procedures"),
    ]
}

pub fn changed(installed: &str, downloaded: &str) -> bool {
    !entries(downloaded).is_empty() && installed != downloaded
}

fn update() {
    let Some(dir) = local_dir() else { return };
    let Some((200, body)) = crate::net::request("GET", &format!("{PACK_BASE}procedures.json"), "") else { return };
    let installed = std::fs::read_to_string(dir.join(PACK_FILE)).unwrap_or_default();
    if entries(&body).is_empty() {
        crate::log("procedure pack: the download was not a procedure list; kept what was there");
        return;
    }
    if !changed(&installed, &body) {
        return;
    }
    let tmp = dir.join(format!("{PACK_FILE}.part"));
    let written = std::fs::create_dir_all(&dir)
        .and_then(|_| std::fs::write(&tmp, body.as_bytes()))
        .and_then(|_| std::fs::rename(&tmp, dir.join(PACK_FILE)));
    match written {
        Ok(()) => crate::log(&format!("procedure pack: updated, {} procedures", entries(&body).len())),
        Err(e) => crate::log(&format!("procedure pack: could not save it ({e})")),
    }
}

pub fn start() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        let _ = std::thread::Builder::new().name("fbw-procedure-pack".into()).spawn(update);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_users_own_procedures_win_over_the_pack() {
        let pack = r#"{"1":{"i":[["A","OFF",0,""]]},"2":{"i":[["B","ON",0,""]]}}"#;
        let own = r#"{"2":{"i":[["B2","AUTO",0,""]]}}"#;
        let merged: Value = serde_json::from_str(&merge(pack, own)).unwrap();
        assert_eq!(merged["1"]["i"][0][0], "A");
        assert_eq!(merged["2"]["i"][0][0], "B2");
    }

    #[test]
    fn a_broken_or_missing_file_adds_nothing() {
        assert_eq!(merge("", ""), "{}");
        assert_eq!(merge("<html>rate limited</html>", r#"{"1":{"i":[]}}"#), r#"{"1":{"i":[]}}"#);
        assert_eq!(merge(r#"{"1":{"no":"steps"}}"#, ""), "{}");
    }

    #[test]
    fn steps_become_flybywire_ecam_items() {
        let merged = r#"{"280900002":{"i":[["LAND ANSA","",0,"g"],["IF LEAK","",0,"c"],["ENG 1 MASTER","OFF",1,""],["EXPECT IMBALANCE","",1,"b"]]},"x":{"i":[["A","B",0,""]]}}"#;
        let items = ewd_items(merged);
        assert!(!items.contains_key("x"));
        let it = items["280900002"].as_array().unwrap();
        assert_eq!(it[0]["style"], "LandAnsa");
        assert_eq!(it[1]["condition"], true);
        assert_eq!(it[2]["labelNotCompleted"], "OFF");
        assert_eq!(it[2]["level"], 1);
        assert_eq!(it[2]["sensed"], false);
        assert_eq!(it[3]["style"], "Cyan");
    }

    #[test]
    #[ignore]
    fn the_patches_apply_to_the_installed_flybywire_javascript() {
        let aircraft = std::path::Path::new("D:/Steam Games/steamapps/common/X-Plane 12/Aircraft/A380 for X-Plane 12");
        let pack = std::fs::read_to_string(aircraft.join("plugins/fbw_a380_systems/local").join(OWN_FILE)).unwrap();
        let mut patches = crate::deep::ecam::patches::source_patches(&crate::deep::registry().alerts);
        patches.extend(patches_for(ewd_items(&pack)));
        let out = std::env::var("PACK_PATCH_OUT").unwrap();
        for (rel, name) in [(crate::deep::ecam::patches::EWD, "ewd.js"), (crate::deep::ecam::patches::SYSTEMS_HOST, "SystemsHost.js")] {
            let text = std::fs::read_to_string(aircraft.join("html_ui").join(rel.trim_start_matches('/'))).unwrap();
            let mut failed = Vec::new();
            let patched = crate::source_patch::apply(&patches, rel, text, |ok, msg| {
                if !ok {
                    failed.push(msg.to_string())
                }
            });
            assert!(failed.is_empty(), "{failed:?}");
            assert!(patched.contains("EcamAbnormalSensedProcedures, {\"2"));
            std::fs::write(std::path::Path::new(&out).join(name), patched).unwrap();
        }
    }

    #[test]
    fn the_pack_is_saved_only_when_its_content_changed() {
        let a = r#"{"1":{"i":[["A","OFF",0,""]]}}"#;
        let b = r#"{"1":{"i":[["A","ON",0,""]]}}"#;
        assert!(changed("", a));
        assert!(!changed(a, a));
        assert!(changed(a, b));
        assert!(!changed(a, "404: Not Found"));
    }
}
