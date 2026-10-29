pub mod aip;
pub mod airac;
pub mod amdb;
pub mod cache;
pub mod chartfox;
pub mod codec;
pub mod http;
pub mod navigraph;

use std::fmt;

pub use cache::DiskCache;
pub use http::{HttpClient, HttpRequest, HttpResponse};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Airport,
    Departure,
    Arrival,
    Approach,
    Reference,
    Other,
}

impl Category {
    pub fn code(self) -> &'static str {
        match self {
            Category::Airport => "APT",
            Category::Departure => "DEP",
            Category::Arrival => "ARR",
            Category::Approach => "APP",
            Category::Reference => "REF",
            Category::Other => "OTHER",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Category::Airport => "Airport",
            Category::Departure => "Departure",
            Category::Arrival => "Arrival",
            Category::Approach => "Approach",
            Category::Reference => "Reference",
            Category::Other => "Other",
        }
    }

    pub fn from_code(code: &str) -> Category {
        match code.trim().to_ascii_uppercase().as_str() {
            "APT" | "AIRPORT" | "GND" | "GROUND" | "TAXI" => Category::Airport,
            "DEP" | "SID" | "DEPARTURE" => Category::Departure,
            "ARR" | "STAR" | "ARRIVAL" => Category::Arrival,
            "APP" | "IAC" | "APPROACH" | "TRANSITION" => Category::Approach,
            "REF" | "REFERENCE" | "GEN" | "GENERAL" | "TEXT" | "TEXTUAL" => Category::Reference,
            _ => Category::Other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChartKind {
    Pdf,
    Png,
    Jpeg,
    Webp,
    Native,
    Link,
}

impl ChartKind {
    pub fn from_path(path: &str) -> ChartKind {
        let lower = path.split(['?', '#']).next().unwrap_or("").to_ascii_lowercase();
        if lower.ends_with(".png") {
            ChartKind::Png
        } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
            ChartKind::Jpeg
        } else if lower.ends_with(".webp") {
            ChartKind::Webp
        } else if lower.ends_with(".pdf") {
            ChartKind::Pdf
        } else {
            ChartKind::Link
        }
    }

    pub fn from_content_type(content_type: &str, fallback: ChartKind) -> ChartKind {
        let ct = content_type.to_ascii_lowercase();
        if ct.contains("pdf") {
            ChartKind::Pdf
        } else if ct.contains("png") {
            ChartKind::Png
        } else if ct.contains("jpeg") || ct.contains("jpg") {
            ChartKind::Jpeg
        } else if ct.contains("webp") {
            ChartKind::Webp
        } else {
            fallback
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PixelRect {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Georef {
    pub pixels: PixelRect,
    pub lat1: f64,
    pub lon1: f64,
    pub lat2: f64,
    pub lon2: f64,
    pub insets: Vec<PixelRect>,
}

impl Georef {
    /// Own-ship position on the chart image, the way FBW's EFB Navigation page places it.
    pub fn project(&self, lat: f64, lon: f64) -> Option<(f64, f64)> {
        let (lat_lo, lat_hi) = (self.lat1.min(self.lat2), self.lat1.max(self.lat2));
        let (lon_lo, lon_hi) = (self.lon1.min(self.lon2), self.lon1.max(self.lon2));
        if !(lat_lo..=lat_hi).contains(&lat) || !(lon_lo..=lon_hi).contains(&lon) {
            return None;
        }
        let d_lat = self.lat2 - self.lat1;
        let d_lon = self.lon2 - self.lon1;
        if d_lat == 0.0 || d_lon == 0.0 {
            return None;
        }
        let dx = self.pixels.x2 - self.pixels.x1;
        let dy = self.pixels.y1 - self.pixels.y2;
        let x = dx * ((lon - self.lon1) / d_lon);
        let y = dy * ((self.lat2 - lat) / d_lat);
        let (px, py) = (self.pixels.x1 + x, self.pixels.y2 + y);
        if self.insets.iter().any(|r| px >= r.x1.min(r.x2) && px <= r.x1.max(r.x2) && py >= r.y1.min(r.y2) && py <= r.y1.max(r.y2)) {
            return None;
        }
        Some((px, py))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chart {
    pub source: String,
    pub id: String,
    pub icao: String,
    pub name: String,
    pub category: Category,
    pub kind: ChartKind,
    pub day_url: Option<String>,
    pub night_url: Option<String>,
    pub georef: Option<Georef>,
    pub procedures: Vec<String>,
    pub runways: Vec<String>,
}

impl Chart {
    pub fn new(source: &str, icao: &str, id: &str, name: &str, category: Category, kind: ChartKind) -> Chart {
        Chart {
            source: source.to_string(),
            id: id.to_string(),
            icao: icao.to_string(),
            name: name.to_string(),
            category,
            kind,
            day_url: None,
            night_url: None,
            georef: None,
            procedures: Vec::new(),
            runways: Vec::new(),
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        let georef = self.georef.as_ref().map(|g| {
            let rect = |r: &PixelRect| serde_json::json!({ "x1": r.x1, "y1": r.y1, "x2": r.x2, "y2": r.y2 });
            serde_json::json!({
                "pixels": rect(&g.pixels),
                "latlng": { "lat1": g.lat1, "lng1": g.lon1, "lat2": g.lat2, "lng2": g.lon2 },
                "insets": g.insets.iter().map(rect).collect::<Vec<_>>(),
            })
        });
        serde_json::json!({
            "source": self.source,
            "id": self.id,
            "icao": self.icao,
            "name": self.name,
            "category": self.category.label(),
            "category_code": self.category.code(),
            "kind": format!("{:?}", self.kind).to_ascii_lowercase(),
            "georef": georef,
            "procedures": self.procedures,
            "runways": self.runways,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AmdbDescriptor {
    pub icao: String,
    pub path: std::path::PathBuf,
    pub projection: &'static str,
    pub layers: Vec<&'static str>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChartData {
    Bytes { content_type: String, bytes: Vec<u8> },
    Native(AmdbDescriptor),
    Link(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChartError {
    Network(String),
    Status(u16, String),
    NotSignedIn,
    NotConfigured(String),
    NotCovered(String),
    NotFound,
    Parse(String),
}

impl fmt::Display for ChartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChartError::Network(e) => write!(f, "network error: {e}"),
            ChartError::Status(s, e) => write!(f, "server replied {s}: {e}"),
            ChartError::NotSignedIn => write!(f, "not signed in"),
            ChartError::NotConfigured(e) => write!(f, "not configured: {e}"),
            ChartError::NotCovered(e) => write!(f, "{e}"),
            ChartError::NotFound => write!(f, "chart not found"),
            ChartError::Parse(e) => write!(f, "unexpected reply: {e}"),
        }
    }
}

impl std::error::Error for ChartError {}

pub fn icao_code(icao: &str) -> Option<String> {
    let code = icao.trim().to_ascii_uppercase();
    (code.len() == 4 && code.chars().all(|c| c.is_ascii_alphanumeric())).then_some(code)
}

pub trait ChartSource {
    fn id(&self) -> &'static str;
    fn search(&mut self, icao: &str) -> Result<Vec<Chart>, ChartError>;
    fn fetch(&mut self, chart: &Chart, night: bool) -> Result<ChartData, ChartError>;
    /// False where the provider's terms forbid storing charts (Navigraph).
    fn cacheable(&self) -> bool;
}

pub struct Charts {
    sources: Vec<Box<dyn ChartSource>>,
    cache: Option<DiskCache>,
}

impl Charts {
    pub fn new(cache: Option<DiskCache>) -> Charts {
        Charts { sources: Vec::new(), cache }
    }

    pub fn add(&mut self, source: Box<dyn ChartSource>) {
        self.sources.push(source);
    }

    pub fn source_ids(&self) -> Vec<&'static str> {
        self.sources.iter().map(|s| s.id()).collect()
    }

    fn source_mut(&mut self, id: &str) -> Result<&mut Box<dyn ChartSource>, ChartError> {
        self.sources.iter_mut().find(|s| s.id() == id).ok_or_else(|| ChartError::NotConfigured(format!("no chart source {id}")))
    }

    pub fn search(&mut self, source: &str, icao: &str) -> Result<Vec<Chart>, ChartError> {
        let icao = icao_code(icao).ok_or_else(|| ChartError::NotCovered("an ICAO code has four letters".into()))?;
        self.source_mut(source)?.search(&icao)
    }

    pub fn fetch(&mut self, chart: &Chart, night: bool) -> Result<ChartData, ChartError> {
        let key = format!("{}|{}|{}|{}", chart.source, chart.icao, chart.id, if night { "n" } else { "d" });
        let cacheable = self.source_mut(&chart.source)?.cacheable();
        if cacheable {
            if let Some((content_type, bytes)) = self.cache.as_ref().and_then(|c| c.get(&key)) {
                return Ok(ChartData::Bytes { content_type, bytes });
            }
        }
        let data = self.source_mut(&chart.source)?.fetch(chart, night)?;
        if cacheable {
            if let (Some(cache), ChartData::Bytes { content_type, bytes }) = (self.cache.as_mut(), &data) {
                let _ = cache.put(&key, content_type, bytes);
            }
        }
        Ok(data)
    }
}

pub fn group(charts: &[Chart]) -> Vec<(Category, Vec<&Chart>)> {
    let order = [Category::Airport, Category::Departure, Category::Arrival, Category::Approach, Category::Reference, Category::Other];
    order
        .iter()
        .filter_map(|cat| {
            let items: Vec<&Chart> = charts.iter().filter(|c| c.category == *cat).collect();
            (!items.is_empty()).then_some((*cat, items))
        })
        .collect()
}
