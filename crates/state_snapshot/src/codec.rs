use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde_json::Value;

use crate::{migrate, Snapshot, CURRENT_VERSION, FORMAT};

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Corrupt(String),
    WrongFormat(String),
    Newer { saved: u32, reader: u32 },
    Migration(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "i/o: {e}"),
            Error::Corrupt(e) => write!(f, "corrupt: {e}"),
            Error::WrongFormat(e) => write!(f, "not a saved state: {e}"),
            Error::Newer { saved, reader } => write!(f, "saved by a newer version (format {saved}, this one reads {reader})"),
            Error::Migration(e) => write!(f, "upgrade failed: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

pub fn encode(snapshot: &Snapshot) -> Result<Vec<u8>, Error> {
    let json = serde_json::to_vec(snapshot).map_err(|e| Error::Corrupt(e.to_string()))?;
    let mut gz = GzEncoder::new(Vec::with_capacity(json.len() / 4), Compression::new(6));
    gz.write_all(&json)?;
    Ok(gz.finish()?)
}

pub fn decode_value(bytes: &[u8]) -> Result<Value, Error> {
    if bytes.len() < 2 || bytes[0] != 0x1f || bytes[1] != 0x8b {
        return Err(Error::WrongFormat("not gzip".into()));
    }
    let mut json = Vec::new();
    GzDecoder::new(bytes).read_to_end(&mut json).map_err(|e| Error::Corrupt(e.to_string()))?;
    let value: Value = serde_json::from_slice(&json).map_err(|e| Error::Corrupt(e.to_string()))?;
    match value.get("format").and_then(Value::as_str) {
        Some(FORMAT) => Ok(value),
        Some(other) => Err(Error::WrongFormat(other.to_string())),
        None => Err(Error::WrongFormat("no format field".into())),
    }
}

pub fn decode(bytes: &[u8]) -> Result<Snapshot, Error> {
    from_value(decode_value(bytes)?)
}

pub fn from_value(mut value: Value) -> Result<Snapshot, Error> {
    let saved = value.get("version").and_then(Value::as_u64).ok_or_else(|| Error::Corrupt("no version".into()))? as u32;
    if saved > CURRENT_VERSION {
        return Err(Error::Newer { saved, reader: CURRENT_VERSION });
    }
    migrate::upgrade(&mut value, saved).map_err(Error::Migration)?;
    serde_json::from_value(value).map_err(|e| Error::Corrupt(e.to_string()))
}

static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.join(format!(".{file_name}.{}.{}.tmp", std::process::id(), TMP_SEQ.fetch_add(1, Ordering::Relaxed)));
    let result = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

pub fn save(path: &Path, snapshot: &Snapshot) -> Result<u64, Error> {
    let bytes = encode(snapshot)?;
    write_atomic(path, &bytes)?;
    Ok(bytes.len() as u64)
}

pub fn load(path: &Path) -> Result<Snapshot, Error> {
    decode(&std::fs::read(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AircraftInfo, SaveReason};

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("state_snapshot_codec_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn sample() -> Snapshot {
        let mut s = Snapshot::new("Before TOD", SaveReason::Manual, AircraftInfo { name: "A380 for X-Plane 12".into(), ..Default::default() });
        s.put_raw("numbers", 1, serde_json::json!({ "pi": std::f64::consts::PI, "tiny": 1.0e-300, "big": 123456789.123456789 }));
        s
    }

    #[test]
    fn encode_then_decode_is_exact_including_floats() {
        let s = sample();
        let back = decode(&encode(&s).unwrap()).unwrap();
        assert_eq!(back, s);
        assert_eq!(back.sections["numbers"].data["pi"].as_f64().unwrap().to_bits(), std::f64::consts::PI.to_bits());
    }

    #[test]
    fn junk_truncated_and_foreign_files_are_errors_not_panics() {
        assert!(matches!(decode(b"{}"), Err(Error::WrongFormat(_))));
        let bytes = encode(&sample()).unwrap();
        assert!(matches!(decode(&bytes[..bytes.len() / 2]), Err(Error::Corrupt(_))));
        let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
        gz.write_all(br#"{"format":"something-else","version":1}"#).unwrap();
        assert!(matches!(decode(&gz.finish().unwrap()), Err(Error::WrongFormat(_))));
    }

    #[test]
    fn a_file_from_a_newer_version_is_refused() {
        let mut v = serde_json::to_value(sample()).unwrap();
        v["version"] = serde_json::json!(CURRENT_VERSION + 1);
        assert!(matches!(from_value(v), Err(Error::Newer { .. })));
    }

    #[test]
    fn an_atomic_write_replaces_the_old_file_and_leaves_no_temp_behind() {
        let d = temp_dir("atomic");
        let p = d.join("slot.a380state");
        save(&p, &sample()).unwrap();
        let mut second = sample();
        second.name = "second".into();
        save(&p, &second).unwrap();
        assert_eq!(load(&p).unwrap().name, "second");
        let leftovers: Vec<_> = std::fs::read_dir(&d).unwrap().filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().ends_with(".tmp")).collect();
        assert!(leftovers.is_empty());
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn a_failed_write_keeps_the_previous_file() {
        let d = temp_dir("failed");
        let p = d.join("slot.a380state");
        save(&p, &sample()).unwrap();
        let blocked = d.join("blocked");
        std::fs::create_dir_all(blocked.join("x.a380state")).unwrap();
        assert!(write_atomic(&blocked.join("x.a380state"), b"data").is_err());
        assert_eq!(load(&p).unwrap().name, "Before TOD");
        std::fs::remove_dir_all(&d).ok();
    }
}
