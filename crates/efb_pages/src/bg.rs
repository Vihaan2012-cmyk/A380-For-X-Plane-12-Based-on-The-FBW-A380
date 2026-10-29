use crate::Node;
use std::collections::BTreeMap;
use std::path::Path;

pub const PAGE_BG: &str = "inline";
pub const LOCK_BG: &str = "inline";

pub fn asset_path(name: &str) -> String {
    format!("bg/{name}.png")
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

#[derive(Clone, Debug, Default)]
pub struct Backgrounds {
    urls: BTreeMap<String, String>,
}

impl Backgrounds {
    pub fn load(dir: &Path) -> Self {
        let mut urls = BTreeMap::new();
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("png") {
                    if let (Some(stem), Ok(bytes)) = (p.file_stem().and_then(|s| s.to_str()), std::fs::read(&p)) {
                        urls.insert(stem.to_string(), format!("data:image/png;base64,{}", base64(&bytes)));
                    }
                }
            }
        }
        Self { urls }
    }

    pub fn url(&self, name: &str) -> String {
        self.urls.get(name).cloned().unwrap_or_else(|| asset_path(name))
    }
}

pub fn page_bg(bgs: &Backgrounds) -> Node {
    Node::el("div", "efb-bg").style(&format!("background-image: url(\"{}\");", bgs.url(PAGE_BG)))
}

pub fn lock_bg(bgs: &Backgrounds) -> Node {
    Node::el("div", "lock-bg").style(&format!("background-image: url(\"{}\");", bgs.url(LOCK_BG)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn missing_asset_falls_back_to_path() {
        let b = Backgrounds::default();
        assert_eq!(b.url("gate"), "bg/gate.png");
        let n = page_bg(&b).to_json();
        assert!(n.contains("efb-bg") && n.contains("bg/inline.png"));
    }
}
