use std::io;
use std::path::PathBuf;

use crate::codec::{hex, sha256};

pub const DEFAULT_MAX_BYTES: u64 = 512 * 1024 * 1024;

pub struct DiskCache {
    root: PathBuf,
    max_bytes: u64,
}

impl DiskCache {
    pub fn new(root: PathBuf, max_bytes: u64) -> DiskCache {
        DiskCache { root, max_bytes }
    }

    fn paths(&self, key: &str) -> (PathBuf, PathBuf) {
        let name = hex(&sha256(key.as_bytes()));
        (self.root.join(format!("{name}.bin")), self.root.join(format!("{name}.type")))
    }

    pub fn get(&self, key: &str) -> Option<(String, Vec<u8>)> {
        let (bin, meta) = self.paths(key);
        let bytes = std::fs::read(&bin).ok()?;
        let content_type = std::fs::read_to_string(&meta).unwrap_or_else(|_| "application/octet-stream".into());
        let _ = std::fs::File::options().append(true).open(&bin).and_then(|f| f.set_modified(std::time::SystemTime::now()));
        Some((content_type, bytes))
    }

    pub fn put(&mut self, key: &str, content_type: &str, bytes: &[u8]) -> io::Result<()> {
        if bytes.len() as u64 > self.max_bytes {
            return Ok(());
        }
        std::fs::create_dir_all(&self.root)?;
        let (bin, meta) = self.paths(key);
        let tmp = bin.with_extension("part");
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, &bin)?;
        std::fs::write(&meta, content_type)?;
        self.evict()
    }

    pub fn size(&self) -> u64 {
        self.entries().iter().map(|e| e.1).sum()
    }

    fn entries(&self) -> Vec<(PathBuf, u64, std::time::SystemTime)> {
        let Ok(dir) = std::fs::read_dir(&self.root) else { return Vec::new() };
        dir.flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "bin"))
            .filter_map(|e| {
                let md = e.metadata().ok()?;
                Some((e.path(), md.len(), md.modified().unwrap_or(std::time::UNIX_EPOCH)))
            })
            .collect()
    }

    fn evict(&self) -> io::Result<()> {
        let mut entries = self.entries();
        let mut total: u64 = entries.iter().map(|e| e.1).sum();
        entries.sort_by_key(|e| e.2);
        for (path, len, _) in entries {
            if total <= self.max_bytes {
                break;
            }
            std::fs::remove_file(&path)?;
            let _ = std::fs::remove_file(path.with_extension("type"));
            total -= len;
        }
        Ok(())
    }

    pub fn clear(&self) -> io::Result<()> {
        for (path, _, _) in self.entries() {
            std::fs::remove_file(&path)?;
            let _ = std::fs::remove_file(path.with_extension("type"));
        }
        Ok(())
    }
}
