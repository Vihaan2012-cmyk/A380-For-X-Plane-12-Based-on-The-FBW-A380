use std::sync::Arc;

use serde_json::Value;

use crate::http::{HttpClient, HttpRequest};
use crate::{Category, Chart, ChartData, ChartError, ChartKind, ChartSource};

pub const API: &str = "https://api.chartfox.org";

/// Legacy grouped-list type keys, as FBW's `ChartFox.tsx` reads them.
fn legacy_group(key: &str) -> Category {
    match key {
        "0" | "1" => Category::Reference,
        "2" => Category::Airport,
        "6" => Category::Departure,
        "7" => Category::Arrival,
        "8" => Category::Approach,
        _ => Category::Other,
    }
}

fn type_category(v: &Value) -> Category {
    if let Some(s) = v.as_str() {
        return Category::from_code(s);
    }
    match v.as_u64() {
        Some(n) => legacy_group(&n.to_string()),
        None => Category::Other,
    }
}

pub struct ChartfoxSource {
    http: Arc<dyn HttpClient>,
    token: String,
    base: String,
}

impl ChartfoxSource {
    pub fn new(http: Arc<dyn HttpClient>, token: &str) -> ChartfoxSource {
        ChartfoxSource { http, token: token.trim().into(), base: API.into() }
    }

    pub fn with_base(mut self, base: &str) -> ChartfoxSource {
        self.base = base.trim_end_matches('/').into();
        self
    }

    fn own_host(&self, url: &str) -> bool {
        url.starts_with(&format!("{}/", self.base))
    }

    fn get(&self, url: &str) -> Result<crate::HttpResponse, ChartError> {
        let mut req = HttpRequest::get(url).header("Accept", "application/json");
        if self.own_host(url) {
            if self.token.is_empty() {
                return Err(ChartError::NotConfigured("ChartFox needs an access token".into()));
            }
            req = req.bearer(&self.token);
        }
        self.http.send(&req).map_err(ChartError::Network)
    }
}

fn chart_from(icao: &str, c: &Value, category: Category) -> Option<Chart> {
    let name = c["name"].as_str()?;
    let id = c["id"].as_str().map(str::to_string).or_else(|| c["id"].as_u64().map(|n| n.to_string())).or_else(|| c["url"].as_str().map(str::to_string))?;
    let url = c["url"].as_str().or_else(|| c["view_url"].as_str()).map(str::to_string);
    let kind = url.as_deref().map(ChartKind::from_path).filter(|k| *k != ChartKind::Link).unwrap_or(ChartKind::Pdf);
    let mut chart = Chart::new("chartfox", icao, &id, name, category, kind);
    chart.day_url = url;
    if let Some(r) = c["runway"].as_str() {
        chart.runways.push(r.into());
    }
    Some(chart)
}

pub fn parse_list(icao: &str, v: &Value) -> Result<Vec<Chart>, ChartError> {
    let mut out = Vec::new();
    let root = if v["data"].is_null() { v } else { &v["data"] };
    if let Some(groups) = root["charts"].as_object().or_else(|| root.as_object().filter(|o| o.values().all(|g| g["charts"].is_array()) && !o.is_empty())) {
        for (key, group) in groups {
            for c in group["charts"].as_array().into_iter().flatten() {
                out.extend(chart_from(icao, c, legacy_group(key)));
            }
        }
        return Ok(out);
    }
    let list = root.as_array().or_else(|| root["charts"].as_array()).ok_or_else(|| ChartError::Parse("no chart list".into()))?;
    for c in list {
        let cat = [&c["type_key"], &c["type"], &c["category"]].into_iter().map(type_category).find(|k| *k != Category::Other).unwrap_or(Category::Other);
        out.extend(chart_from(icao, c, cat));
    }
    Ok(out)
}

impl ChartSource for ChartfoxSource {
    fn id(&self) -> &'static str {
        "chartfox"
    }

    fn search(&mut self, icao: &str) -> Result<Vec<Chart>, ChartError> {
        let reply = self.get(&format!("{}/v2/airports/{icao}/charts", self.base))?;
        match reply.status {
            200..=299 => parse_list(icao, &reply.json().ok_or_else(|| ChartError::Parse("chart list is not JSON".into()))?),
            401 | 403 => Err(ChartError::NotSignedIn),
            404 => Err(ChartError::NotCovered(format!("ChartFox has no charts for {icao}"))),
            s => Err(ChartError::Status(s, reply.text())),
        }
    }

    fn fetch(&mut self, chart: &Chart, _night: bool) -> Result<ChartData, ChartError> {
        let url = match &chart.day_url {
            Some(u) => u.clone(),
            None => {
                let reply = self.get(&format!("{}/v2/charts/{}", self.base, crate::codec::percent_encode(&chart.id)))?;
                if !reply.ok() {
                    return Err(ChartError::Status(reply.status, reply.text()));
                }
                let v = reply.json().unwrap_or(Value::Null);
                let d = if v["data"].is_null() { &v } else { &v["data"] };
                d["url"].as_str().or_else(|| d["view_url"].as_str()).ok_or(ChartError::NotFound)?.to_string()
            }
        };
        if !url.starts_with("https://") {
            return Err(ChartError::Parse("chart URL is not https".into()));
        }
        let reply = self.get(&url)?;
        if !reply.ok() {
            return Err(ChartError::Status(reply.status, reply.text()));
        }
        let ct = if reply.content_type.is_empty() { "application/pdf".to_string() } else { reply.content_type };
        Ok(ChartData::Bytes { content_type: ct, bytes: reply.body })
    }

    fn cacheable(&self) -> bool {
        false
    }
}
