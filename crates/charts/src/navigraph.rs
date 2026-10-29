use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

use crate::codec::{base64_decode, base64url, jwt_claims, pkce_pair, sha256};
use crate::http::{HttpClient, HttpRequest};
use crate::{Category, Chart, ChartData, ChartError, ChartKind, ChartSource, Georef, PixelRect};

pub const IDENTITY: &str = "https://identity.api.navigraph.com";
pub const API: &str = "https://api.navigraph.com";
const SCOPE: &str = "openid charts offline_access";
const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
const REFRESH_MARGIN_S: u64 = 60;

#[derive(Clone, PartialEq)]
pub struct Tokens {
    pub access: String,
    pub refresh: String,
    pub expires_at: u64,
}

fn mask_for_log(s: &str) -> String {
    if s.len() <= 8 {
        "****".into()
    } else {
        format!("{}…{}", &s[..4], &s[s.len() - 4..])
    }
}

impl fmt::Debug for Tokens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tokens").field("access", &mask_for_log(&self.access)).field("refresh", &mask_for_log(&self.refresh)).field("expires_at", &self.expires_at).finish()
    }
}

pub trait TokenStore: Send {
    fn load(&self) -> Option<Tokens>;
    fn save(&self, tokens: Option<&Tokens>);
}

#[derive(Default)]
pub struct MemoryTokenStore(std::sync::Mutex<Option<Tokens>>);

impl TokenStore for MemoryTokenStore {
    fn load(&self) -> Option<Tokens> {
        self.0.lock().unwrap().clone()
    }
    fn save(&self, tokens: Option<&Tokens>) {
        *self.0.lock().unwrap() = tokens.cloned();
    }
}

/// Tokens on disk XOR-masked with a key derived from the machine, so the file
/// is not readable as plain text or usable when copied elsewhere. Obfuscation, not encryption.
pub struct FileTokenStore {
    path: PathBuf,
    key: [u8; 32],
}

impl FileTokenStore {
    pub fn new(path: PathBuf, machine_salt: &str) -> FileTokenStore {
        FileTokenStore { path, key: sha256(format!("a380-charts-navigraph|{machine_salt}").as_bytes()) }
    }

    pub fn for_this_machine(path: PathBuf) -> FileTokenStore {
        let salt: String = ["COMPUTERNAME", "HOSTNAME", "USERNAME", "USER"].iter().filter_map(|k| std::env::var(k).ok()).collect::<Vec<_>>().join("|");
        FileTokenStore::new(path, &salt)
    }

    fn xor(&self, data: &[u8]) -> Vec<u8> {
        data.iter().enumerate().map(|(i, b)| b ^ self.key[i % 32]).collect()
    }

    pub fn mask(&self, tokens: &Tokens) -> String {
        let plain = serde_json::json!({ "a": tokens.access, "r": tokens.refresh, "e": tokens.expires_at }).to_string();
        format!("masked1:{}", base64url(&self.xor(plain.as_bytes())))
    }

    pub fn unmask(&self, text: &str) -> Option<Tokens> {
        let raw = base64_decode(text.trim().strip_prefix("masked1:")?)?;
        let v: Value = serde_json::from_slice(&self.xor(&raw)).ok()?;
        Some(Tokens { access: v["a"].as_str()?.into(), refresh: v["r"].as_str()?.into(), expires_at: v["e"].as_u64()? })
    }
}

impl TokenStore for FileTokenStore {
    fn load(&self) -> Option<Tokens> {
        self.unmask(&std::fs::read_to_string(&self.path).ok()?)
    }
    fn save(&self, tokens: Option<&Tokens>) {
        match tokens {
            Some(t) => {
                if let Some(dir) = self.path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = std::fs::write(&self.path, self.mask(t));
            }
            None => {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_at: u64,
    pub interval_s: u64,
}

#[derive(Clone)]
struct PendingFlow {
    device_code: String,
    verifier: String,
    shown: DeviceCode,
    next_poll_at: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Poll {
    Wait { retry_in_s: u64 },
    SignedIn,
    Denied,
    Expired,
    NoFlow,
}

pub type Clock = Box<dyn Fn() -> u64 + Send>;
pub type Entropy = Box<dyn Fn() -> [u8; 32] + Send>;

pub fn system_clock() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn system_entropy() -> [u8; 32] {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).expect("system random source");
    b
}

pub struct NavigraphSource {
    http: Arc<dyn HttpClient>,
    client_id: String,
    client_secret: String,
    store: Box<dyn TokenStore>,
    tokens: Option<Tokens>,
    flow: Option<PendingFlow>,
    clock: Clock,
    entropy: Entropy,
}

impl NavigraphSource {
    pub fn new(http: Arc<dyn HttpClient>, client_id: &str, client_secret: &str, store: Box<dyn TokenStore>) -> NavigraphSource {
        let tokens = store.load();
        NavigraphSource {
            http,
            client_id: client_id.trim().into(),
            client_secret: client_secret.trim().into(),
            store,
            tokens,
            flow: None,
            clock: Box::new(system_clock),
            entropy: Box::new(system_entropy),
        }
    }

    pub fn with_clock(mut self, clock: Clock) -> NavigraphSource {
        self.clock = clock;
        self
    }

    pub fn with_entropy(mut self, entropy: Entropy) -> NavigraphSource {
        self.entropy = entropy;
        self
    }

    pub fn configured(&self) -> bool {
        !self.client_id.is_empty()
    }

    pub fn signed_in(&self) -> bool {
        self.tokens.is_some()
    }

    pub fn sign_out(&mut self) {
        self.tokens = None;
        self.flow = None;
        self.store.save(None);
    }

    fn credentials(&self) -> Result<(), ChartError> {
        if self.configured() {
            Ok(())
        } else {
            Err(ChartError::NotConfigured("Navigraph needs a client ID in navigraph.ini".into()))
        }
    }

    fn post(&self, path: &str, fields: &[(&str, &str)]) -> Result<(u16, Value), ChartError> {
        let mut all: Vec<(&str, &str)> = fields.to_vec();
        all.push(("client_id", &self.client_id));
        if !self.client_secret.is_empty() {
            all.push(("client_secret", &self.client_secret));
        }
        let reply = self.http.send(&HttpRequest::post_form(&format!("{IDENTITY}{path}"), &all)).map_err(ChartError::Network)?;
        Ok((reply.status, reply.json().unwrap_or(Value::Null)))
    }

    pub fn start_sign_in(&mut self) -> Result<DeviceCode, ChartError> {
        self.credentials()?;
        let (verifier, challenge) = pkce_pair(&(self.entropy)());
        let (status, v) = self.post("/connect/deviceauthorization", &[("code_challenge", &challenge), ("code_challenge_method", "S256"), ("scope", SCOPE)])?;
        if !(200..300).contains(&status) {
            return Err(ChartError::Status(status, v["error"].as_str().unwrap_or("device authorization refused").into()));
        }
        let field = |k: &str| v[k].as_str().map(str::to_string).ok_or_else(|| ChartError::Parse(format!("missing {k}")));
        let now = (self.clock)();
        let shown = DeviceCode {
            user_code: field("user_code")?,
            verification_uri: field("verification_uri")?,
            verification_uri_complete: v["verification_uri_complete"].as_str().map(str::to_string).unwrap_or_else(|| field("verification_uri").unwrap_or_default()),
            expires_at: now + v["expires_in"].as_u64().unwrap_or(300),
            interval_s: v["interval"].as_u64().unwrap_or(5),
        };
        self.flow = Some(PendingFlow { device_code: field("device_code")?, verifier, shown: shown.clone(), next_poll_at: now + shown.interval_s });
        Ok(shown)
    }

    pub fn pending_code(&self) -> Option<&DeviceCode> {
        self.flow.as_ref().map(|f| &f.shown)
    }

    pub fn poll_sign_in(&mut self) -> Result<Poll, ChartError> {
        let now = (self.clock)();
        let Some(flow) = self.flow.clone() else { return Ok(Poll::NoFlow) };
        if now >= flow.shown.expires_at {
            self.flow = None;
            return Ok(Poll::Expired);
        }
        if now < flow.next_poll_at {
            return Ok(Poll::Wait { retry_in_s: flow.next_poll_at - now });
        }
        let (status, v) = self.post("/connect/token", &[("grant_type", DEVICE_GRANT), ("device_code", &flow.device_code), ("code_verifier", &flow.verifier), ("scope", SCOPE)])?;
        if (200..300).contains(&status) {
            self.accept_tokens(&v)?;
            self.flow = None;
            return Ok(Poll::SignedIn);
        }
        match v["error"].as_str().unwrap_or("") {
            "authorization_pending" => {
                self.flow.as_mut().unwrap().next_poll_at = now + flow.shown.interval_s;
                Ok(Poll::Wait { retry_in_s: flow.shown.interval_s })
            }
            "slow_down" => {
                let f = self.flow.as_mut().unwrap();
                f.shown.interval_s += 5;
                f.next_poll_at = now + f.shown.interval_s;
                Ok(Poll::Wait { retry_in_s: f.shown.interval_s })
            }
            "access_denied" => {
                self.flow = None;
                Ok(Poll::Denied)
            }
            "expired_token" => {
                self.flow = None;
                Ok(Poll::Expired)
            }
            other => Err(ChartError::Status(status, other.to_string())),
        }
    }

    fn accept_tokens(&mut self, v: &Value) -> Result<(), ChartError> {
        let access = v["access_token"].as_str().ok_or_else(|| ChartError::Parse("missing access_token".into()))?;
        let refresh = v["refresh_token"].as_str().map(str::to_string).or_else(|| self.tokens.as_ref().map(|t| t.refresh.clone())).unwrap_or_default();
        let tokens = Tokens { access: access.into(), refresh, expires_at: (self.clock)() + v["expires_in"].as_u64().unwrap_or(3600) };
        self.store.save(Some(&tokens));
        self.tokens = Some(tokens);
        Ok(())
    }

    fn refresh(&mut self) -> Result<(), ChartError> {
        let refresh = self.tokens.as_ref().map(|t| t.refresh.clone()).filter(|r| !r.is_empty()).ok_or(ChartError::NotSignedIn)?;
        let (status, v) = self.post("/connect/token", &[("grant_type", "refresh_token"), ("refresh_token", &refresh)])?;
        if (200..300).contains(&status) {
            return self.accept_tokens(&v);
        }
        if status == 400 || status == 401 {
            self.sign_out();
            return Err(ChartError::NotSignedIn);
        }
        Err(ChartError::Status(status, v["error"].as_str().unwrap_or("refresh failed").into()))
    }

    fn access_token(&mut self) -> Result<String, ChartError> {
        self.credentials()?;
        let t = self.tokens.as_ref().ok_or(ChartError::NotSignedIn)?;
        if (self.clock)() + REFRESH_MARGIN_S >= t.expires_at {
            self.refresh()?;
        }
        Ok(self.tokens.as_ref().ok_or(ChartError::NotSignedIn)?.access.clone())
    }

    fn get_authed(&mut self, url: &str) -> Result<crate::HttpResponse, ChartError> {
        let token = self.access_token()?;
        let reply = self.http.send(&HttpRequest::get(url).bearer(&token)).map_err(ChartError::Network)?;
        if reply.status != 401 {
            return Ok(reply);
        }
        self.refresh()?;
        let token = self.access_token()?;
        self.http.send(&HttpRequest::get(url).bearer(&token)).map_err(ChartError::Network)
    }

    pub fn has_charts_subscription(&self) -> bool {
        let Some(claims) = self.tokens.as_ref().and_then(|t| jwt_claims(&t.access)) else { return false };
        match &claims["subscriptions"] {
            Value::Array(a) => a.iter().any(|s| s.as_str() == Some("charts")),
            Value::String(s) => s.split([' ', ',']).any(|x| x == "charts"),
            _ => false,
        }
    }

    pub fn username(&mut self) -> Result<String, ChartError> {
        let reply = self.get_authed(&format!("{IDENTITY}/connect/userinfo"))?;
        reply.json().and_then(|v| v["preferred_username"].as_str().map(str::to_string)).ok_or_else(|| ChartError::Parse("no preferred_username".into()))
    }
}

pub fn parse_index(icao: &str, v: &Value) -> Result<Vec<Chart>, ChartError> {
    let list = v["charts"].as_array().or_else(|| v.as_array()).ok_or_else(|| ChartError::Parse("no charts array".into()))?;
    let num = |o: &Value, k: &str| o[k].as_f64();
    let rect = |o: &Value| Some(PixelRect { x1: num(o, "x1")?, y1: num(o, "y1")?, x2: num(o, "x2")?, y2: num(o, "y2")? });
    let strings = |o: &Value| -> Vec<String> {
        o.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string).or_else(|| x["ident"].as_str().map(str::to_string))).collect()).unwrap_or_default()
    };
    Ok(list
        .iter()
        .filter_map(|c| {
            let id = c["id"].as_str()?;
            let mut chart = Chart::new("navigraph", icao, id, c["name"].as_str().unwrap_or(id), Category::from_code(c["category"].as_str().unwrap_or("")), ChartKind::Png);
            chart.day_url = c["image_day_url"].as_str().map(str::to_string);
            chart.night_url = c["image_night_url"].as_str().map(str::to_string);
            chart.procedures = strings(&c["procedures"]);
            chart.runways = strings(&c["runways"]);
            if c["is_georeferenced"].as_bool() == Some(true) {
                let pv = &c["bounding_boxes"]["planview"];
                let ll = &pv["latlng"];
                chart.georef = rect(&pv["pixels"]).and_then(|pixels| {
                    Some(Georef {
                        pixels,
                        lat1: num(ll, "lat1")?,
                        lon1: num(ll, "lng1")?,
                        lat2: num(ll, "lat2")?,
                        lon2: num(ll, "lng2")?,
                        insets: c["bounding_boxes"]["insets"].as_array().map(|a| a.iter().filter_map(|i| rect(&i["pixels"])).collect()).unwrap_or_default(),
                    })
                });
            }
            Some(chart)
        })
        .collect())
}

fn is_navigraph_url(url: &str) -> bool {
    url.strip_prefix("https://").and_then(|r| r.split(['/', '?']).next()).is_some_and(|h| {
        let h = h.to_ascii_lowercase();
        !h.contains(['@', ':']) && (h == "navigraph.com" || h.ends_with(".navigraph.com"))
    })
}

impl ChartSource for NavigraphSource {
    fn id(&self) -> &'static str {
        "navigraph"
    }

    fn search(&mut self, icao: &str) -> Result<Vec<Chart>, ChartError> {
        let reply = self.get_authed(&format!("{API}/v2/charts/{icao}?version=STD&rules=IFR"))?;
        match reply.status {
            200..=299 => parse_index(icao, &reply.json().ok_or_else(|| ChartError::Parse("chart index is not JSON".into()))?),
            404 => Err(ChartError::NotCovered(format!("Navigraph has no charts for {icao}"))),
            s => Err(ChartError::Status(s, reply.text())),
        }
    }

    fn fetch(&mut self, chart: &Chart, night: bool) -> Result<ChartData, ChartError> {
        let url = if night { chart.night_url.as_ref().or(chart.day_url.as_ref()) } else { chart.day_url.as_ref() }.ok_or(ChartError::NotFound)?.clone();
        if !is_navigraph_url(&url) {
            return Err(ChartError::Parse("chart image is not on navigraph.com".into()));
        }
        let reply = self.get_authed(&url)?;
        if !reply.ok() {
            return Err(ChartError::Status(reply.status, reply.text()));
        }
        Ok(ChartData::Bytes { content_type: if reply.content_type.is_empty() { "image/png".into() } else { reply.content_type }, bytes: reply.body })
    }

    fn cacheable(&self) -> bool {
        false
    }
}
