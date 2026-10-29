//! Navigraph through the plugin: XPHFBW's screens send every Navigraph
//! request here (`app/js/msfs-runtime.js`), and only `*.navigraph.com` is
//! forwarded. The client credentials (`client_id=`, `client_secret=`) come
//! from `navigraph.ini` next to the plugin, the file the aircraft ships
//! with, or else from `Output/preferences/navigraph.ini`.

const USER_CONFIG: &str = "Output/preferences/navigraph.ini";
const RECEIVE_MS: u32 = 60_000;
const FORWARDED: [&str; 3] = ["authorization", "content-type", "accept"];

pub struct Reply {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

fn credentials(ini: &str) -> Option<(String, String)> {
    let map = crate::efb::parse_ini(ini);
    let id = map.get("client_id").map(|s| s.trim().to_string()).filter(|s| !s.is_empty())?;
    let secret = map.get("client_secret").map(|s| s.trim().to_string()).unwrap_or_default();
    Some((id, secret))
}

fn configured() -> Option<(String, String)> {
    let shipped = crate::sound_pack::plugin_dir().map(|d| d.join("navigraph.ini"));
    shipped
        .into_iter()
        .chain([std::path::PathBuf::from(USER_CONFIG)])
        .find_map(|p| credentials(&std::fs::read_to_string(p).unwrap_or_default()))
}

fn allowed(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    !host.contains(['/', '@', ':', '?', '#']) && (host == "navigraph.com" || host.ends_with(".navigraph.com"))
}

fn encode(value: &str) -> String {
    value.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

fn with_credentials(body: &str, id: &str, secret: &str) -> String {
    let kept: Vec<&str> = body.split('&').filter(|kv| !kv.is_empty() && !kv.starts_with("client_id=") && !kv.starts_with("client_secret=")).collect();
    let mut out = kept.join("&");
    if !out.is_empty() {
        out.push('&');
    }
    out.push_str(&format!("client_id={}", encode(id)));
    if !secret.is_empty() {
        out.push_str(&format!("&client_secret={}", encode(secret)));
    }
    out
}

fn identity_endpoint(host: &str, path: &str) -> bool {
    host.eq_ignore_ascii_case("identity.api.navigraph.com") && path.starts_with("/connect/")
}

pub fn forward(method: &str, rest: &str, query: &str, request_head: &str, body: &str) -> Reply {
    let fail = |status: u16, message: &str| Reply { status, content_type: "application/json".into(), body: serde_json::json!({ "error": message }).to_string().into_bytes() };
    let (host, path) = rest.split_once('/').map(|(h, p)| (h, format!("/{p}"))).unwrap_or((rest, "/".to_string()));
    if !allowed(host) {
        return fail(403, "only navigraph.com is forwarded");
    }
    let mut body = body.to_string();
    if identity_endpoint(host, &path) && method.eq_ignore_ascii_case("POST") {
        let Some((id, secret)) = configured() else {
            return fail(503, "Navigraph sign-in needs a Navigraph developer client in plugins/fbw_a380_systems/navigraph.ini");
        };
        body = with_credentials(&body, &id, &secret);
    }
    let mut headers = String::new();
    for line in request_head.lines().skip(1) {
        if let Some((name, value)) = line.split_once(':') {
            if FORWARDED.contains(&name.trim().to_ascii_lowercase().as_str()) {
                headers.push_str(&format!("{}: {}\r\n", name.trim(), value.trim()));
            }
        }
    }
    let url = if query.is_empty() { format!("https://{host}{path}") } else { format!("https://{host}{path}?{query}") };
    match crate::net::request_bytes(method, &url, body.as_bytes(), &headers, RECEIVE_MS) {
        Some((status, content_type, out)) => Reply { status, content_type: if content_type.is_empty() { "application/octet-stream".into() } else { content_type }, body: out },
        None => fail(502, "Navigraph could not be reached"),
    }
}

pub fn is_navigraph_url(url: &str) -> bool {
    url.strip_prefix("https://").and_then(|r| r.split(['/', '?']).next()).is_some_and(allowed)
}

pub fn forward_url(method: &str, url: &str, headers: &serde_json::Value, body: &str) -> Reply {
    let rest = url.strip_prefix("https://").unwrap_or(url);
    let (rest, query) = rest.split_once('?').unwrap_or((rest, ""));
    let mut head = String::from("FETCH\r\n");
    if let Some(map) = headers.as_object() {
        for (name, value) in map {
            if let Some(v) = value.as_str() {
                if !name.contains([':', '\r', '\n']) && !v.contains(['\r', '\n']) {
                    head.push_str(&format!("{name}: {v}\r\n"));
                }
            }
        }
    }
    forward(method, rest, query, &head, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_navigraph_hosts_are_forwarded() {
        assert!(allowed("api.navigraph.com"));
        assert!(allowed("identity.api.navigraph.com"));
        assert!(!allowed("navigraph.com.evil.example"));
        assert!(!allowed("example.com"));
        assert!(!allowed("api.navigraph.com@evil.example"));
    }

    #[test]
    fn credentials_replace_whatever_the_page_sent() {
        let out = with_credentials("grant_type=device_code&client_id=undefined&device_code=abc", "my id", "s&t");
        assert_eq!(out, "grant_type=device_code&device_code=abc&client_id=my%20id&client_secret=s%26t");
        assert_eq!(with_credentials("", "x", ""), "client_id=x");
    }

    #[test]
    fn plugin_fetches_to_navigraph_are_recognised() {
        assert!(is_navigraph_url("https://api.navigraph.com/v2/charts?x=1"));
        assert!(is_navigraph_url("https://identity.api.navigraph.com/connect/token"));
        assert!(!is_navigraph_url("http://api.navigraph.com/"));
        assert!(!is_navigraph_url("https://api.navigraph.com.evil.example/"));
        assert!(!is_navigraph_url("https://www.simbrief.com/api"));
    }

    #[test]
    fn a_non_navigraph_url_is_refused_without_a_request() {
        let r = forward_url("GET", "https://example.com/x", &serde_json::json!({}), "");
        assert_eq!(r.status, 403);
    }

    #[test]
    fn credentials_come_from_the_ini() {
        assert_eq!(credentials("client_id = abc\nclient_secret = def\n"), Some(("abc".into(), "def".into())));
        assert_eq!(credentials("client_secret = def\n"), None);
    }
}
