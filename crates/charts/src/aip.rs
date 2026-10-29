use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;

use crate::airac;
use crate::http::{HttpClient, HttpRequest};
use crate::{Category, Chart, ChartData, ChartError, ChartKind, ChartSource};

pub const BUILTIN_TABLE: &str = include_str!("../data/aip_sources.json");

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    FaaDtpp { chart: String },
    HtmlIndex { link_contains: String, section: Option<(String, String)> },
    Portal,
    None,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Country {
    pub country: String,
    pub authority: String,
    pub prefixes: Vec<String>,
    pub index: String,
    pub kind: Kind,
    pub access: String,
    pub verified: String,
}

#[derive(Clone, Debug)]
pub struct Table {
    pub countries: Vec<Country>,
    tokens: Vec<(String, Category)>,
}

impl Table {
    pub fn builtin() -> Table {
        Table::parse(BUILTIN_TABLE).expect("built-in AIP table parses")
    }

    pub fn parse(text: &str) -> Result<Table, String> {
        let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let s = |o: &Value, k: &str| o[k].as_str().unwrap_or("").to_string();
        let tokens = v["tokens"]
            .as_array()
            .map(|a| a.iter().filter_map(|t| Some((t[0].as_str()?.to_ascii_uppercase(), Category::from_code(t[1].as_str()?)))).collect())
            .unwrap_or_default();
        let mut countries = Vec::new();
        for c in v["countries"].as_array().ok_or("no countries")? {
            let kind = match c["kind"].as_str().unwrap_or("") {
                "faa_dtpp" => Kind::FaaDtpp { chart: s(c, "chart") },
                "html_index" => Kind::HtmlIndex {
                    link_contains: s(c, "link_contains"),
                    section: c["section_start"].as_str().map(|a| (a.to_string(), c["section_end"].as_str().unwrap_or("").to_string())),
                },
                "portal" => Kind::Portal,
                _ => Kind::None,
            };
            countries.push(Country {
                country: s(c, "country"),
                authority: s(c, "authority"),
                prefixes: c["prefixes"].as_array().map(|a| a.iter().filter_map(|p| p.as_str().map(str::to_ascii_uppercase)).collect()).unwrap_or_default(),
                index: s(c, "index"),
                kind,
                access: s(c, "access"),
                verified: s(c, "verified"),
            });
        }
        Ok(Table { countries, tokens })
    }

    pub fn country_for(&self, icao: &str) -> Option<&Country> {
        let icao = icao.to_ascii_uppercase();
        self.countries.iter().filter_map(|c| c.prefixes.iter().filter(|p| icao.starts_with(p.as_str())).map(|p| p.len()).max().map(|n| (n, c))).max_by_key(|(n, _)| *n).map(|(_, c)| c)
    }

    pub fn categorise(&self, text: &str) -> Category {
        let norm: String = text.to_ascii_uppercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' }).collect();
        let norm = format!(" {} ", norm.split_whitespace().collect::<Vec<_>>().join(" "));
        self.tokens.iter().find(|(t, _)| norm.contains(&format!(" {t} "))).map(|(_, c)| *c).unwrap_or(Category::Other)
    }
}

fn fill(template: &str, icao: &str, today: i64) -> String {
    airac::fill(template, icao, today).replace("{prefix2}", icao.get(..2).unwrap_or(""))
}

pub fn join_url(base: &str, href: &str) -> String {
    let href = href.trim().replace("&amp;", "&");
    if href.starts_with("http://") || href.starts_with("https://") {
        return href;
    }
    if let Some(rest) = href.strip_prefix("//") {
        return format!("https://{rest}");
    }
    let (scheme_host, path) = match base.find("://").and_then(|i| base[i + 3..].find('/').map(|j| i + 3 + j)) {
        Some(at) => (&base[..at], &base[at..]),
        None => (base, "/"),
    };
    if href.starts_with('/') {
        return format!("{scheme_host}{href}");
    }
    let mut parts: Vec<&str> = path.split('/').collect();
    parts.pop();
    for seg in href.split('/') {
        match seg {
            "." => {}
            ".." => {
                if parts.len() > 1 {
                    parts.pop();
                }
            }
            s => parts.push(s),
        }
    }
    format!("{scheme_host}{}", parts.join("/"))
}

fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ").replace("&amp;", "&").split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn links(html: &str) -> Vec<(String, String)> {
    let lower = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(i) = lower[at..].find("href=") {
        let start = at + i + 5;
        let Some(q) = html[start..].chars().next() else { break };
        let (value_start, end) = if q == '"' || q == '\'' {
            let vs = start + 1;
            (vs, html[vs..].find(q).map(|e| vs + e).unwrap_or(html.len()))
        } else {
            (start, html[start..].find([' ', '>']).map(|e| start + e).unwrap_or(html.len()))
        };
        let href = html[value_start..end].to_string();
        let text_start = html[end..].find('>').map(|e| end + e + 1).unwrap_or(end);
        let text_end = lower[text_start..].find("</a").map(|e| text_start + e).unwrap_or(text_start);
        out.push((href, strip_tags(&html[text_start..text_end])));
        at = text_end.max(end);
    }
    out
}

pub fn faa_category(code: &str) -> Category {
    match code {
        "APD" | "HOT" => Category::Airport,
        "IAP" => Category::Approach,
        "DP" | "ODP" => Category::Departure,
        "STAR" => Category::Arrival,
        "MIN" | "LAH" => Category::Reference,
        _ => Category::Other,
    }
}

fn tag<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let a = s.find(&open)? + open.len();
    let b = s[a..].find(&format!("</{name}>"))? + a;
    Some(s[a..b].trim())
}

pub fn parse_faa(meta: &str, icao: &str, chart_template: &str, today: i64) -> Vec<Chart> {
    let key = format!("icao_ident=\"{icao}\"");
    let Some(at) = meta.find(&key) else { return Vec::new() };
    let end = meta[at..].find("</airport_name>").map(|e| at + e).unwrap_or(meta.len());
    meta[at..end]
        .split("<record>")
        .skip(1)
        .filter_map(|rec| {
            let (code, name, pdf) = (tag(rec, "chart_code")?, tag(rec, "chart_name")?, tag(rec, "pdf_name")?);
            if pdf.is_empty() || pdf.eq_ignore_ascii_case("DELETED_JOB.PDF") {
                return None;
            }
            let mut c = Chart::new("aip", icao, pdf, name, faa_category(code), ChartKind::Pdf);
            c.day_url = Some(fill(chart_template, icao, today).replace("{file}", pdf));
            Some(c)
        })
        .collect()
}

pub struct AipSource {
    http: Arc<dyn HttpClient>,
    table: Table,
    today: Box<dyn Fn() -> i64 + Send>,
    pages: HashMap<String, String>,
}

impl AipSource {
    pub fn new(http: Arc<dyn HttpClient>, table: Table) -> AipSource {
        AipSource { http, table, today: Box::new(airac::today_days), pages: HashMap::new() }
    }

    pub fn with_today(mut self, today: Box<dyn Fn() -> i64 + Send>) -> AipSource {
        self.today = today;
        self
    }

    pub fn table(&self) -> &Table {
        &self.table
    }

    fn page(&mut self, url: &str) -> Result<String, ChartError> {
        if let Some(p) = self.pages.get(url) {
            return Ok(p.clone());
        }
        let reply = self.http.send(&HttpRequest::get(url)).map_err(ChartError::Network)?;
        if !reply.ok() {
            return Err(ChartError::Status(reply.status, format!("index page {url}")));
        }
        let text = reply.text();
        if self.pages.len() > 16 {
            self.pages.clear();
        }
        self.pages.insert(url.to_string(), text.clone());
        Ok(text)
    }
}

impl ChartSource for AipSource {
    fn id(&self) -> &'static str {
        "aip"
    }

    fn search(&mut self, icao: &str) -> Result<Vec<Chart>, ChartError> {
        let country = self.table.country_for(icao).cloned().ok_or_else(|| ChartError::NotCovered(format!("no free AIP source listed for {icao}")))?;
        let today = (self.today)();
        let index = fill(&country.index, icao, today);
        match &country.kind {
            Kind::None => Err(ChartError::NotCovered(format!("{} ({}) charts are not free to fetch: {}", country.country, country.authority, country.access))),
            Kind::Portal => {
                let mut c = Chart::new("aip", icao, &index, &format!("{} AIP ({})", country.country, country.authority), Category::Reference, ChartKind::Link);
                c.day_url = Some(index);
                Ok(vec![c])
            }
            Kind::FaaDtpp { chart } => {
                let charts = parse_faa(&self.page(&index)?, icao, chart, today);
                if charts.is_empty() {
                    return Err(ChartError::NotCovered(format!("{icao} has no FAA charts")));
                }
                Ok(charts)
            }
            Kind::HtmlIndex { link_contains, section } => {
                let page = self.page(&index)?;
                let body = match section {
                    Some((start, end)) => {
                        let start = fill(start, icao, today);
                        let Some(at) = page.find(&start) else { return Err(ChartError::NotCovered(format!("{icao} is not in the {} index", country.authority))) };
                        let rest = &page[at + start.len()..];
                        let stop = if end.is_empty() { rest.len() } else { rest.find(end.as_str()).unwrap_or(rest.len()) };
                        rest[..stop].to_string()
                    }
                    None => page,
                };
                let want = fill(link_contains, icao, today);
                let mut seen = std::collections::HashSet::new();
                let charts: Vec<Chart> = links(&body)
                    .into_iter()
                    .filter(|(href, _)| href.contains(&want) && href.to_ascii_lowercase().split(['?', '#']).next().is_some_and(|p| p.ends_with(".pdf")))
                    .filter_map(|(href, text)| {
                        let url = join_url(&index, &href);
                        if !seen.insert(url.clone()) {
                            return None;
                        }
                        let file = url.rsplit('/').next().unwrap_or(&url).to_string();
                        let stem = file.trim_end_matches(".pdf").trim_end_matches(".PDF").to_string();
                        let name = if text.is_empty() { stem.clone() } else { text.clone() };
                        let mut c = Chart::new("aip", icao, &file, &name, self.table.categorise(&format!("{name} {stem}")), ChartKind::Pdf);
                        c.day_url = Some(url);
                        Some(c)
                    })
                    .collect();
                if charts.is_empty() {
                    return Err(ChartError::NotCovered(format!("no {} charts found for {icao}", country.authority)));
                }
                Ok(charts)
            }
        }
    }

    fn fetch(&mut self, chart: &Chart, _night: bool) -> Result<ChartData, ChartError> {
        let url = chart.day_url.clone().ok_or(ChartError::NotFound)?;
        if chart.kind == ChartKind::Link {
            return Ok(ChartData::Link(url));
        }
        let reply = self.http.send(&HttpRequest::get(&url)).map_err(ChartError::Network)?;
        if !reply.ok() {
            return Err(ChartError::Status(reply.status, url));
        }
        let ct = if reply.content_type.is_empty() || reply.content_type.contains("octet-stream") { "application/pdf".to_string() } else { reply.content_type };
        Ok(ChartData::Bytes { content_type: ct, bytes: reply.body })
    }

    fn cacheable(&self) -> bool {
        true
    }
}
