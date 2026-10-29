#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Fpln,
    Init,
    FuelLoad,
    Perf,
    Wind,
    VertRev,
    Hold,
    DirTo,
    Airways,
    Departure,
    Arrival,
    PosMonitor,
    PosIrs,
    PosNavaids,
    SecIndex,
    DataStatus,
    DataWaypoint,
    DataNavaid,
    DataAirport,
    AtcConnect,
    AtcRequest,
    AtcMsgRecord,
    AtcDatis,
    SurvControls,
    SurvStatus,
    FcuAfs,
    FcuEfis,
    NotFound,
}

pub const FMS_CATEGORY_PAGES: &[(&str, Page)] = &[
    ("f-pln", Page::Fpln),
    ("init", Page::Init),
    ("fuel-load", Page::FuelLoad),
    ("perf", Page::Perf),
    ("wind", Page::Wind),
    ("f-pln-vert-rev", Page::VertRev),
    ("f-pln-hold", Page::Hold),
    ("f-pln-direct-to", Page::DirTo),
    ("f-pln-airways", Page::Airways),
    ("f-pln-departure", Page::Departure),
    ("f-pln-arrival", Page::Arrival),
];

/// FBW's MFD URI split: sys / category / page / extra.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Uri {
    pub uri: String,
    pub sys: String,
    pub category: String,
    pub page: String,
    pub extra: String,
}

impl Uri {
    pub fn parse(uri: &str) -> Uri {
        let parts: Vec<&str> = uri.split('/').collect();
        Uri {
            uri: uri.to_string(),
            sys: parts.first().unwrap_or(&"").to_string(),
            category: parts.get(1).unwrap_or(&"").to_string(),
            page: parts.get(2).unwrap_or(&"").to_string(),
            extra: if parts.len() > 3 { parts[3..].join("/") } else { String::new() },
        }
    }

    pub fn page(&self) -> Page {
        match (self.sys.as_str(), self.category.as_str(), self.page.as_str()) {
            ("fms", "active" | "sec1" | "sec2" | "sec3", p) => FMS_CATEGORY_PAGES.iter().find(|(n, _)| *n == p).map(|(_, pg)| *pg).unwrap_or(Page::NotFound),
            ("fms", "position", "monitor") => Page::PosMonitor,
            ("fms", "position", "irs") => Page::PosIrs,
            ("fms", "position", "navaids") => Page::PosNavaids,
            ("fms", "sec", "index") => Page::SecIndex,
            ("fms", "data", "status") => Page::DataStatus,
            ("fms", "data", "waypoint") => Page::DataWaypoint,
            ("fms", "data", "navaid") => Page::DataNavaid,
            ("fms", "data", "airport") => Page::DataAirport,
            ("atccom", "connect", _) => Page::AtcConnect,
            ("atccom", "request", _) => Page::AtcRequest,
            ("atccom", "msg-record", _) => Page::AtcMsgRecord,
            ("atccom", "d-atis", _) => Page::AtcDatis,
            ("surv", "controls", _) => Page::SurvControls,
            ("surv", "status-switching", _) => Page::SurvStatus,
            ("fcubkup", "afs", _) => Page::FcuAfs,
            ("fcubkup", "efis", _) => Page::FcuEfis,
            _ => Page::NotFound,
        }
    }

    /// FBW flight plan index the page edits: 1 while a temporary plan exists on ACTIVE, 3.. for SEC.
    pub fn sec(&self) -> Option<usize> {
        match self.category.as_str() {
            "sec1" => Some(1),
            "sec2" => Some(2),
            "sec3" => Some(3),
            _ => None,
        }
    }
}

pub fn fms_uri(category: &str, page: &str) -> String {
    format!("fms/{category}/{page}")
}

/// The page selector items of the FMS header, in FBW order (menu index -> URI).
pub fn header_menu_uri(selector: usize, item: usize) -> Option<&'static str> {
    let list: &[&str] = match selector {
        0 => &["fms/active/f-pln", "fms/active/perf", "fms/active/fuel-load", "fms/active/wind", "fms/active/init"],
        1 => &["fms/position/monitor", "", "fms/position/navaids", "fms/position/irs", "", ""],
        2 => &["fms/sec/index/1", "fms/sec/index/2", "fms/sec/index/3"],
        3 => &["fms/data/status", "fms/data/waypoint", "fms/data/navaid", "", "fms/data/airport", ""],
        _ => &[],
    };
    list.get(item).copied().filter(|u| !u.is_empty())
}

pub fn sys_uri(system: usize) -> &'static str {
    match system {
        1 => "atccom/connect",
        2 => "surv/controls",
        3 => "fcubkup/afs",
        _ => "fms/active/init",
    }
}

pub fn sub_header_uri(sys: &str, selector: usize) -> Option<&'static str> {
    let list: &[&str] = match sys {
        "atccom" => &["atccom/connect", "atccom/request", "atccom/report-modify/position", "atccom/msg-record", "atccom/d-atis/list", "atccom/emer"],
        "surv" => &["surv/controls", "surv/status-switching"],
        "fcubkup" => &["fcubkup/afs", "fcubkup/efis"],
        _ => &[],
    };
    list.get(selector).copied()
}
