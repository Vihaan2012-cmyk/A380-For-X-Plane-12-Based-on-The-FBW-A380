use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::install::{download, github_get, unzip, Ctx, ReleaseFile, AIRCRAFT_DIR};

pub const INDEX: &str = "liveries.json";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Edition {
    pub file: String,
    pub folder: String,
    pub size: u64,
    pub sha256: String,
    #[serde(default, skip_deserializing)]
    pub url: String,
    #[serde(default, skip_deserializing)]
    pub installed: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Livery {
    pub id: String,
    pub airline: String,
    pub name: String,
    #[serde(default)]
    pub registration: String,
    #[serde(default)]
    pub country: String,
    #[serde(default)]
    pub iata: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub engines: String,
    #[serde(default)]
    pub creator: String,
    #[serde(default)]
    pub preview: Option<String>,
    #[serde(default, skip_deserializing)]
    pub preview_url: Option<String>,
    pub editions: BTreeMap<String, Edition>,
}

#[derive(Deserialize)]
struct Index {
    liveries: Vec<Livery>,
}

#[derive(Deserialize)]
struct GhRelease {
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

pub fn folder_name_ok(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', ':']) && !name.starts_with('.')
}

pub fn index_from(text: &str, assets: &BTreeMap<String, String>) -> Result<Vec<Livery>, String> {
    let index: Index = serde_json::from_str(text).map_err(|e| format!("{INDEX}: {e}"))?;
    let mut out = Vec::new();
    for mut livery in index.liveries {
        livery.editions.retain(|_, e| folder_name_ok(&e.folder));
        for e in livery.editions.values_mut() {
            e.url = assets.get(&e.file).cloned().unwrap_or_default();
        }
        livery.editions.retain(|_, e| !e.url.is_empty());
        livery.preview_url = livery.preview.as_ref().and_then(|p| assets.get(p).cloned());
        if !livery.editions.is_empty() {
            out.push(livery);
        }
    }
    Ok(out)
}

pub fn available(repo: &str) -> Result<Vec<Livery>, String> {
    if let Ok(base) = std::env::var("FBW_XP_LIVERIES_URL") {
        let base = base.trim_end_matches('/').to_owned();
        let text = github_get(&format!("{base}/{INDEX}"))?.into_string().map_err(|e| format!("{INDEX}: {e}"))?;
        let index: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{INDEX}: {e}"))?;
        let mut assets = BTreeMap::new();
        for l in index["liveries"].as_array().into_iter().flatten() {
            let names = l["editions"].as_object().into_iter().flatten().filter_map(|(_, e)| e["file"].as_str()).chain(l["preview"].as_str());
            for n in names {
                assets.insert(n.to_owned(), format!("{base}/{n}"));
            }
        }
        return index_from(&text, &assets);
    }
    let list = github_get(&format!("https://api.github.com/repos/{repo}/releases?per_page=30"))?
        .into_string()
        .map_err(|e| format!("GitHub's release list: {e}"))?;
    let releases: Vec<GhRelease> = serde_json::from_str(&list).map_err(|e| format!("GitHub's release list: {e}"))?;
    let release = releases
        .into_iter()
        .filter(|r| !r.draft)
        .find(|r| r.assets.iter().any(|a| a.name == INDEX))
        .ok_or("no release on GitHub has liveries yet")?;
    let assets: BTreeMap<String, String> = release.assets.into_iter().map(|a| (a.name, a.browser_download_url)).collect();
    let text = github_get(&assets[INDEX])?.into_string().map_err(|e| format!("{INDEX}: {e}"))?;
    index_from(&text, &assets)
}

pub fn liveries_dir(xplane: &Path) -> PathBuf {
    xplane.join(AIRCRAFT_DIR).join("liveries")
}

pub fn mark_installed(liveries: &mut [Livery], dir: &Path) {
    for l in liveries {
        for e in l.editions.values_mut() {
            e.installed = dir.join(&e.folder).is_dir();
        }
    }
}

pub fn installed_folders(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|r| r.flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    names.sort();
    names
}

fn find<'a>(liveries: &'a [Livery], id: &str, edition: &str) -> Result<(&'a Livery, &'a Edition), String> {
    let livery = liveries.iter().find(|l| l.id == id).ok_or_else(|| format!("no livery {id} in the latest release"))?;
    let e = livery.editions.get(edition).ok_or_else(|| format!("{} has no {edition} edition", livery.name))?;
    Ok((livery, e))
}

pub fn install(xplane: &Path, repo: &str, id: &str, edition: &str, ctx: &Ctx) -> Result<String, String> {
    if !xplane.join(AIRCRAFT_DIR).is_dir() {
        return Err("Install the A380 first; liveries go into its folder".into());
    }
    ctx.status("Reading the livery list...");
    let liveries = available(repo)?;
    let (livery, e) = find(&liveries, id, edition)?;
    let work = std::env::temp_dir().join("fbw-a380x-xp-liveries");
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let archive = work.join(&e.file);
    let file = ReleaseFile { name: e.file.clone(), size: e.size, sha256: e.sha256.clone(), component: "livery".into(), url: e.url.clone() };
    ctx.status(format!("Downloading {} {} ({edition})...", livery.airline, livery.name));
    let mut done = 0;
    download(&e.url, &archive, &file, ctx, &mut done, e.size.max(1))?;
    ctx.status(format!("Installing {} {}...", livery.airline, livery.name));
    let dir = liveries_dir(xplane);
    let staging = work.join(format!("{}-{edition}", livery.id));
    let _ = std::fs::remove_dir_all(&staging);
    unzip(&archive, &staging)?;
    if !staging.join(&e.folder).is_dir() {
        return Err(format!("{} does not hold the folder {}", e.file, e.folder));
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let target = dir.join(&e.folder);
    let _ = std::fs::remove_dir_all(&target);
    std::fs::rename(staging.join(&e.folder), &target).or_else(|_| copy_dir(&staging.join(&e.folder), &target))?;
    let _ = std::fs::remove_dir_all(&staging);
    let _ = std::fs::remove_file(&archive);
    Ok(format!("{} {} ({edition}) installed", livery.airline, livery.name))
}

pub fn remove(xplane: &Path, folder: &str) -> Result<String, String> {
    if !folder_name_ok(folder) {
        return Err(format!("{folder} is not a livery folder name"));
    }
    let target = liveries_dir(xplane).join(folder);
    match std::fs::remove_dir_all(&target) {
        Ok(()) => Ok(format!("{folder} removed")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(format!("{folder} was not installed")),
        Err(e) => Err(format!("could not remove {}: {e}", target.display())),
    }
}

pub(crate) fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(from).map_err(|e| e.to_string())?.flatten() {
        let path = entry.path();
        let dest = to.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &dest)?;
        } else {
            std::fs::copy(&path, &dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const INDEX_JSON: &str = r#"{"liveries": [
        {"id": "ana-ja381a", "airline": "ANA", "name": "JA381A", "registration": "JA381A", "preview": "livery-ana-ja381a.jpg",
         "editions": {"2K": {"file": "livery-ana-ja381a-2k.zip", "folder": "ANA JA381A", "size": 10, "sha256": "ab"},
                      "4K": {"file": "livery-ana-ja381a-4k.zip", "folder": "ANA JA381A 4K", "size": 20, "sha256": "cd"}}},
        {"id": "bad", "airline": "X", "name": "Y",
         "editions": {"2K": {"file": "livery-bad-2k.zip", "folder": "../escape", "size": 1, "sha256": "ef"}}}
    ]}"#;

    #[test]
    fn the_index_gets_its_download_urls_and_drops_unsafe_folders() {
        let assets: BTreeMap<String, String> = ["livery-ana-ja381a-2k.zip", "livery-ana-ja381a.jpg", "livery-bad-2k.zip"]
            .iter()
            .map(|n| (n.to_string(), format!("https://example/{n}")))
            .collect();
        let liveries = index_from(INDEX_JSON, &assets).unwrap();
        assert_eq!(liveries.len(), 1, "the livery whose only folder escapes the liveries folder is dropped");
        let ana = &liveries[0];
        assert_eq!(ana.editions.len(), 1, "the 4K edition has no uploaded file");
        assert_eq!(ana.editions["2K"].url, "https://example/livery-ana-ja381a-2k.zip");
        assert_eq!(ana.preview_url.as_deref(), Some("https://example/livery-ana-ja381a.jpg"));
    }

    #[test]
    fn installed_editions_are_the_folders_present() {
        let dir = std::env::temp_dir().join(format!("livery-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("ANA JA381A 4K")).unwrap();
        let assets: BTreeMap<String, String> = ["livery-ana-ja381a-2k.zip", "livery-ana-ja381a-4k.zip"].iter().map(|n| (n.to_string(), n.to_string())).collect();
        let mut liveries = index_from(INDEX_JSON, &assets).unwrap();
        mark_installed(&mut liveries, &dir);
        assert!(!liveries[0].editions["2K"].installed);
        assert!(liveries[0].editions["4K"].installed);
        assert_eq!(installed_folders(&dir), vec!["ANA JA381A 4K".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn removal_refuses_paths() {
        let xp = std::env::temp_dir();
        assert!(remove(&xp, "../x").is_err());
        assert!(remove(&xp, "a/b").is_err());
        assert!(remove(&xp, "..").is_err());
    }
}
