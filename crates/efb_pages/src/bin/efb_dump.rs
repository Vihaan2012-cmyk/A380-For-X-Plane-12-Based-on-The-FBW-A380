use efb_pages::{flight::{flight_plan_content, FlightPlanView}, home::home_page, settings::{settings_page, SettingsTable}, EfbModel};
use serde_json::Value;

fn read(p: &str) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

fn json(p: Option<&String>) -> Value {
    p.map(|p| serde_json::from_str(&read(p)).expect("json")).unwrap_or(Value::Null)
}

fn view<T: serde::de::DeserializeOwned + Default>() -> T {
    match std::env::var("EFB_VIEW") {
        Ok(p) => serde_json::from_str(&read(&p)).expect("view json"),
        Err(_) => T::default(),
    }
}

fn failures_model(v: Value) -> efb_pages::failures::FailuresModel {
    use efb_pages::failures::{Failure, FailuresModel, Pending};
    if let Some(list) = v.get("failures").filter(|l| l.is_array()) {
        let list: Vec<Failure> = serde_json::from_value(list.clone()).unwrap();
        let pending: Vec<Pending> = v.get("pending").map(|p| serde_json::from_value(p.clone()).unwrap()).unwrap_or_default();
        return FailuresModel::from_list(list, pending);
    }
    serde_json::from_value(v).unwrap()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: efb_dump <page> <out.json> [model.json]");
        std::process::exit(2);
    }
    if let Ok(fx) = std::env::var("EFB_FIXTURE") {
        let v: Value = serde_json::from_str(&read(&fx)).expect("fixture");
        let breakers = std::env::var("EFB_CTX_BREAKERS").is_ok().then(|| {
            v["breakers"]["breakers"].as_array().map(|l| l.iter().filter(|c| c["bus"].as_str().is_some_and(|b| !b.is_empty())).map(|c| (c["bus"].as_str().unwrap().to_string(), c["name"].as_str().unwrap_or("").to_string())).collect()).unwrap_or_default()
        });
        let failures = v["failures"]["failures"].as_array().cloned().unwrap_or_default();
        efb_pages::diagrams::set_ctx(efb_pages::diagrams::Ctx { breakers, failures, ..Default::default() });
    }
    let raw = || json(args.get(3));
    let model = || -> EfbModel { args.get(3).map(|p| serde_json::from_str(&read(p)).expect("model json")).unwrap_or_default() };
    let tree = match args[1].as_str() {
        "home" => home_page(&model()),
        "flight" => flight_plan_content(&model(), &FlightPlanView::default()),
        "payload" => efb_pages::payload::loadsheet_content(&efb_pages::payload::LoadsheetSource::NoPage { preview: true }),
        "docs" => {
            let st: efb_pages::docs::DocsState = match std::env::var("EFB_DOCS_STATE") {
                Ok(p) => efb_pages::docs::DocsState::from_json(&read(&p)),
                Err(_) => Default::default(),
            };
            efb_pages::docs::docs_page(&efb_pages::docs::DocsData::bundled(), &model(), &st)
        }
        "failures" | "failures-nav" | "families" => {
            let fm = failures_model(raw());
            if args[1] == "families" {
                std::fs::write(&args[2], serde_json::to_string(&fm.families).unwrap()).expect("write");
                return;
            }
            let mut v: efb_pages::failures::FailuresView = view();
            if let (Ok(t), Ok(h)) = (std::env::var("EFB_SCROLL"), std::env::var("EFB_VH")) {
                v.scroll_top = t.parse().unwrap();
                v.viewport = Some(h.parse().unwrap());
            }
            if args[1] == "failures-nav" { efb_pages::failures::failures_nav(&fm, &v) } else { efb_pages::failures::failures_content_with(&fm, &v, &efb_pages::diagrams::fail_diagram) }
        }
        "ground" => efb_pages::ground::ground_content(&serde_json::from_value(raw()).unwrap()),
        "settings" => settings_page(&SettingsTable::bundled(), &model()),
        "rail" | "topbar" | "lock" | "footer" => {
            let v = raw();
            let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            let tab = s("tab");
            match args[1].as_str() {
                "rail" => efb_pages::shell::rail(&tab),
                "topbar" => efb_pages::shell::topbar(&tab, &efb_pages::shell::Clock { utc: s("utc"), local: s("local"), chrono: s("chrono"), chrono_running: false, progress: None }),
                "footer" => efb_pages::shell::footer(Some(false)),
                _ => efb_pages::shell::lock(&Default::default(), &efb_pages::shell::LockClock { date: s("date"), clock: s("clock"), utc: s("utc") }),
            }
        }
        "status" => efb_pages::misc::status_tab(&serde_json::from_value(raw()).unwrap(), true),
        "diagnostics" => efb_pages::misc::diagnostics_tab(&serde_json::from_value(raw()).unwrap(), true),
        "support" => efb_pages::misc::support_tab(true),
        "charts" => efb_pages::misc::charts_tab(&serde_json::from_value(raw()).unwrap(), true),
        "saves" => efb_pages::misc::saves_content(&serde_json::from_value(raw()).unwrap()),
        "throttle" => efb_pages::misc::throttle_content(&serde_json::from_value(raw()).unwrap()),
        other => match extra(other, raw(), args.get(3)) {
            Some(n) => n,
            None => {
                eprintln!("unknown page {other}");
                std::process::exit(2);
            }
        },
    };
    std::fs::write(&args[2], tree.to_json()).expect("write");
}

fn extra(page: &str, raw: Value, path: Option<&String>) -> Option<efb_pages::Node> {
    use efb_pages::{maint, study};
    let ordered = || path.map(|p| efb_pages::jv::parse_ordered(&read(p))).unwrap_or(Value::Null);
    let vars = || raw.get("vars").and_then(|v| v.get("vars")).and_then(Value::as_object).cloned().unwrap_or_default();
    Some(match page {
        "study" => {
            let mut pages: Vec<Value> = raw["pages"]["pages"].as_array().cloned().unwrap_or_default();
            study::merge_deep(&mut pages);
            let v: study::StudyView = view();
            study::study_tab(&pages, &vars(), &v, true)
        }
        "components" => {
            let v: maint::ListView = view();
            maint::components_tab(raw["components"]["components"].as_array().map(Vec::as_slice), &v, true)
        }
        "wear" => {
            let o = ordered();
            maint::wear_tab(o.get("wear"), true)
        }
        "breakers" => {
            let v: maint::ListView = view();
            let list = raw["breakers"]["breakers"].as_array().map(|l| maint::breaker_list(l));
            maint::breakers_tab(list.as_deref(), &v, true)
        }
        "maintenance" => {
            let v: maint::ListView = view();
            let mel: std::collections::BTreeMap<String, Vec<Value>> = serde_json::from_value(raw["melItems"].clone()).unwrap_or_default();
            let search: std::collections::BTreeMap<String, maint::DefectSearch> = serde_json::from_value(raw["search"].clone()).unwrap_or_default();
            maint::maintenance_tab(raw.get("maintenance"), &mel, &search, &v, true)
        }
        "atc" => {
            let v: efb_pages::ops::AtcView = view();
            efb_pages::ops::atc_content(&v)
        }
        "cargo" => {
            let v: efb_pages::ops::CargoView = view();
            efb_pages::ops::cargo_content(&v)
        }
        "fap" => {
            let v: efb_pages::fap::FapView = view();
            let m: efb_pages::EfbModel = serde_json::from_value(raw.get("model").cloned().unwrap_or(Value::Null)).unwrap_or_default();
            efb_pages::fap::fap_content(&v, &m, &efb_pages::settings::SettingsTable::bundled())
        }
        "pushback" => {
            let v: efb_pages::pushback::PbView = view();
            efb_pages::pushback::pushback_content(&v)
        }
        "perf" => {
            let mut v: efb_pages::perf::PerfView = view();
            let mut to = efb_pages::perf::initial("to");
            to.extend(std::mem::take(&mut v.to));
            let mut ldg = efb_pages::perf::initial("ldg");
            ldg.extend(std::mem::take(&mut v.ldg));
            let mut applied = Default::default();
            efb_pages::perf::operator_defaults(&mut to, &mut ldg, &mut applied, &v.settings, &efb_pages::settings::SettingsTable::bundled().defaults);
            v.to = to;
            v.ldg = ldg;
            efb_pages::perf::perf_content(&v)
        }
        "loadsheet" => {
            let state = raw.get("state").and_then(Value::as_object).cloned().unwrap_or_default();
            let vars = raw.get("vars").and_then(Value::as_object).cloned();
            efb_pages::payload::loadsheet_page(&raw["page"], vars.as_ref(), &state, raw.get("said").and_then(Value::as_str).unwrap_or(""))
        }
        _ => return None,
    })
}
