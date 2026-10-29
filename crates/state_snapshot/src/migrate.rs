use serde_json::Value;

use crate::CURRENT_VERSION;

pub type Migration = fn(&mut Value) -> Result<(), String>;

/// `MIGRATIONS[i]` turns a version `i + 1` file into version `i + 2`.
pub const MIGRATIONS: &[Migration] = &[];

const _: () = assert!(MIGRATIONS.len() as u32 + 1 == CURRENT_VERSION);

pub fn upgrade(value: &mut Value, from: u32) -> Result<(), String> {
    upgrade_with(MIGRATIONS, value, from)
}

pub fn upgrade_with(table: &[Migration], value: &mut Value, from: u32) -> Result<(), String> {
    if from == 0 {
        return Err("version 0 is not a valid file".into());
    }
    let target = table.len() as u32 + 1;
    for v in from..target {
        table[(v - 1) as usize](value).map_err(|e| format!("{v} -> {}: {e}", v + 1))?;
        value["version"] = Value::from(v + 1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rename_sections(v: &mut Value) -> Result<(), String> {
        let old = v.get_mut("blocks").and_then(Value::as_object_mut).ok_or("no blocks")?.clone();
        v.as_object_mut().unwrap().remove("blocks");
        v["sections"] = Value::Object(old);
        Ok(())
    }

    fn add_summary(v: &mut Value) -> Result<(), String> {
        v["summary"] = json!({});
        Ok(())
    }

    #[test]
    fn every_step_runs_in_order_and_bumps_the_version() {
        let table: &[Migration] = &[rename_sections, add_summary];
        let mut v = json!({ "version": 1, "blocks": { "a": 1 } });
        upgrade_with(table, &mut v, 1).unwrap();
        assert_eq!(v, json!({ "version": 3, "sections": { "a": 1 }, "summary": {} }));
        let mut w = json!({ "version": 2, "sections": {} });
        upgrade_with(table, &mut w, 2).unwrap();
        assert_eq!(w["version"], 3);
        assert!(w.get("blocks").is_none());
    }

    #[test]
    fn a_failing_step_names_itself() {
        let table: &[Migration] = &[rename_sections];
        let mut v = json!({ "version": 1 });
        assert_eq!(upgrade_with(table, &mut v, 1).unwrap_err(), "1 -> 2: no blocks");
    }

    #[test]
    fn the_current_version_needs_no_step() {
        let mut v = json!({ "version": CURRENT_VERSION });
        upgrade(&mut v, CURRENT_VERSION).unwrap();
        assert_eq!(v["version"], CURRENT_VERSION);
    }
}
