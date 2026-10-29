use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use charts::aip::{join_url, links, AipSource, Table};
use charts::airac::days_from_civil;
use charts::amdb::AmdbSource;
use charts::chartfox::ChartfoxSource;
use charts::codec::base64url;
use charts::http::ScriptedHttp;
use charts::navigraph::{FileTokenStore, MemoryTokenStore, NavigraphSource, Poll, TokenStore, Tokens};
use charts::{Category, ChartData, ChartError, ChartKind, ChartSource, Charts, DiskCache};

const NG_INDEX: &str = include_str!("fixtures/navigraph_index.json");
const CF_LEGACY: &str = include_str!("fixtures/chartfox_legacy.json");
const CF_LIST: &str = include_str!("fixtures/chartfox_list.json");
const FAA_META: &str = include_str!("fixtures/faa_metafile.xml");
const ENAIRE: &str = include_str!("fixtures/enaire_index.html");
const DAP: &str = include_str!("fixtures/dap_toc.html");

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("charts-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn jwt(claims: &str) -> String {
    format!("{}.{}.sig", base64url(br#"{"alg":"none"}"#), base64url(claims.as_bytes()))
}

fn clock(at: &Arc<AtomicU64>) -> Box<dyn Fn() -> u64 + Send> {
    let at = at.clone();
    Box::new(move || at.load(Ordering::SeqCst))
}

fn signed_in_navigraph(http: Arc<ScriptedHttp>, now: &Arc<AtomicU64>, expires_at: u64) -> NavigraphSource {
    let store = MemoryTokenStore::default();
    store.save(Some(&Tokens { access: jwt(r#"{"subscriptions":["charts","fmsdata"]}"#), refresh: "r-1".into(), expires_at }));
    NavigraphSource::new(http, "cid", "csecret", Box::new(store)).with_clock(clock(now))
}

#[test]
fn navigraph_device_flow_polls_until_signed_in() {
    let http = Arc::new(
        ScriptedHttp::new()
            .json("POST", "https://identity.api.navigraph.com/connect/deviceauthorization", 200,
                r#"{"device_code":"dc","user_code":"ABCD-EFGH","verification_uri":"https://navigraph.com/code","verification_uri_complete":"https://navigraph.com/code?c=ABCD-EFGH","expires_in":1800,"interval":5}"#)
            .json("POST", "https://identity.api.navigraph.com/connect/token", 400, r#"{"error":"authorization_pending"}"#)
            .json("POST", "https://identity.api.navigraph.com/connect/token", 400, r#"{"error":"slow_down"}"#)
            .json("POST", "https://identity.api.navigraph.com/connect/token", 200, r#"{"access_token":"at-1","refresh_token":"rt-1","expires_in":3600,"token_type":"Bearer"}"#),
    );
    let now = Arc::new(AtomicU64::new(1000));
    let store = Arc::new(MemoryTokenStore::default());
    struct Shared(Arc<MemoryTokenStore>);
    impl TokenStore for Shared {
        fn load(&self) -> Option<Tokens> {
            self.0.load()
        }
        fn save(&self, t: Option<&Tokens>) {
            self.0.save(t)
        }
    }
    let mut ng = NavigraphSource::new(http.clone(), "cid", "csecret", Box::new(Shared(store.clone()))).with_clock(clock(&now)).with_entropy(Box::new(|| [1u8; 32]));
    let code = ng.start_sign_in().unwrap();
    assert_eq!(code.user_code, "ABCD-EFGH");
    assert_eq!(code.interval_s, 5);
    let auth = http.sent()[0].body_text();
    assert!(auth.contains("code_challenge_method=S256") && auth.contains("client_id=cid") && auth.contains("client_secret=csecret"));

    assert_eq!(ng.poll_sign_in().unwrap(), Poll::Wait { retry_in_s: 5 });
    assert_eq!(http.sent().len(), 1, "no poll before the interval");
    now.store(1005, Ordering::SeqCst);
    assert_eq!(ng.poll_sign_in().unwrap(), Poll::Wait { retry_in_s: 5 });
    now.store(1010, Ordering::SeqCst);
    assert_eq!(ng.poll_sign_in().unwrap(), Poll::Wait { retry_in_s: 10 });
    now.store(1020, Ordering::SeqCst);
    assert_eq!(ng.poll_sign_in().unwrap(), Poll::SignedIn);
    let poll = http.sent()[1].body_text();
    assert!(poll.contains("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code") && poll.contains("code_verifier=") && poll.contains("device_code=dc"));
    let saved = store.load().unwrap();
    assert_eq!((saved.access.as_str(), saved.refresh.as_str(), saved.expires_at), ("at-1", "rt-1", 1020 + 3600));
    assert!(ng.signed_in());
}

#[test]
fn navigraph_denied_and_expired_flows_stop() {
    let http = Arc::new(
        ScriptedHttp::new()
            .json("POST", "https://identity.api.navigraph.com/connect/deviceauthorization", 200, r#"{"device_code":"dc","user_code":"U","verification_uri":"https://x.navigraph.com/","expires_in":30,"interval":5}"#)
            .json("POST", "https://identity.api.navigraph.com/connect/token", 400, r#"{"error":"access_denied"}"#),
    );
    let now = Arc::new(AtomicU64::new(0));
    let mut ng = NavigraphSource::new(http, "cid", "", Box::new(MemoryTokenStore::default())).with_clock(clock(&now)).with_entropy(Box::new(|| [2u8; 32]));
    ng.start_sign_in().unwrap();
    now.store(5, Ordering::SeqCst);
    assert_eq!(ng.poll_sign_in().unwrap(), Poll::Denied);
    assert_eq!(ng.poll_sign_in().unwrap(), Poll::NoFlow);
}

#[test]
fn navigraph_without_client_id_is_not_configured() {
    let mut ng = NavigraphSource::new(Arc::new(ScriptedHttp::new()), "", "", Box::new(MemoryTokenStore::default()));
    assert!(matches!(ng.start_sign_in(), Err(ChartError::NotConfigured(_))));
    assert!(matches!(ng.search("XMPL"), Err(ChartError::NotConfigured(_))));
}

#[test]
fn navigraph_index_parses_categories_and_georef() {
    let http = Arc::new(ScriptedHttp::new().json("GET", "https://api.navigraph.com/v2/charts/XMPL?version=STD&rules=IFR", 200, NG_INDEX));
    let now = Arc::new(AtomicU64::new(100));
    let mut ng = signed_in_navigraph(http.clone(), &now, 10_000);
    assert!(ng.has_charts_subscription());
    let list = ng.search("XMPL").unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(list.iter().map(|c| c.category).collect::<Vec<_>>(), [Category::Airport, Category::Approach, Category::Departure]);
    assert_eq!(list[0].runways, ["09", "27"]);
    let g = list[0].georef.as_ref().unwrap();
    let (x, y) = g.project(50.05, 10.05).unwrap();
    assert!((x - 500.0).abs() < 1e-6 && (y - 700.0).abs() < 1e-6, "{x} {y}");
    assert_eq!(g.project(50.2, 10.05), None);
    assert_eq!(g.project(50.09, 10.09), None, "inside an inset");
    assert!(list[1].georef.is_none());
    assert_eq!(http.sent()[0].header_value("Authorization").unwrap(), format!("Bearer {}", jwt(r#"{"subscriptions":["charts","fmsdata"]}"#)));
    assert!(!ng.cacheable());
}

#[test]
fn navigraph_refreshes_before_expiry_and_on_401() {
    let http = Arc::new(
        ScriptedHttp::new()
            .json("POST", "https://identity.api.navigraph.com/connect/token", 200, r#"{"access_token":"at-2","refresh_token":"rt-2","expires_in":3600}"#)
            .json("GET", "https://api.navigraph.com/v2/charts/XMPL", 401, "{}")
            .json("POST", "https://identity.api.navigraph.com/connect/token", 200, r#"{"access_token":"at-3","refresh_token":"rt-3","expires_in":3600}"#)
            .json("GET", "https://api.navigraph.com/v2/charts/XMPL", 200, NG_INDEX)
            .reply("GET", "https://api.navigraph.com/v2/charts/XMPL/xmpl101_n.png", 200, "image/png", vec![0x89, b'P', b'N', b'G']),
    );
    let now = Arc::new(AtomicU64::new(1000));
    let mut ng = signed_in_navigraph(http.clone(), &now, 1030);
    let list = ng.search("XMPL").unwrap();
    let sent = http.sent();
    assert!(sent[0].body_text().contains("grant_type=refresh_token") && sent[0].body_text().contains("refresh_token=r-1"));
    assert_eq!(sent[1].header_value("Authorization"), Some("Bearer at-2"));
    assert!(sent[2].body_text().contains("refresh_token=rt-2"), "refresh tokens are single use");
    assert_eq!(sent[3].header_value("Authorization"), Some("Bearer at-3"));
    let img = ng.fetch(&list[0], true).unwrap();
    assert_eq!(img, ChartData::Bytes { content_type: "image/png".into(), bytes: vec![0x89, b'P', b'N', b'G'] });
}

#[test]
fn navigraph_rejected_refresh_signs_out() {
    let http = Arc::new(ScriptedHttp::new().json("POST", "https://identity.api.navigraph.com/connect/token", 400, r#"{"error":"invalid_grant"}"#));
    let now = Arc::new(AtomicU64::new(5000));
    let mut ng = signed_in_navigraph(http, &now, 10);
    assert_eq!(ng.search("XMPL"), Err(ChartError::NotSignedIn));
    assert!(!ng.signed_in());
}

#[test]
fn navigraph_refuses_foreign_image_hosts() {
    let now = Arc::new(AtomicU64::new(0));
    let mut ng = signed_in_navigraph(Arc::new(ScriptedHttp::new()), &now, 10_000);
    let mut c = charts::Chart::new("navigraph", "XMPL", "x", "x", Category::Airport, ChartKind::Png);
    c.day_url = Some("https://api.navigraph.com.evil.example/x.png".into());
    assert!(matches!(ng.fetch(&c, false), Err(ChartError::Parse(_))));
}

#[test]
fn token_file_is_masked_and_machine_bound() {
    let dir = scratch("tokens");
    let path = dir.join("navigraph.tok");
    let store = FileTokenStore::new(path.clone(), "machine-a");
    let t = Tokens { access: "access-secret-value".into(), refresh: "refresh-secret-value".into(), expires_at: 42 };
    store.save(Some(&t));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("masked1:"));
    assert!(!text.contains("secret"));
    assert_eq!(store.load(), Some(t.clone()));
    assert_eq!(FileTokenStore::new(path.clone(), "machine-b").load(), None);
    assert!(!format!("{t:?}").contains("secret-value"));
    store.save(None);
    assert!(!path.exists());
}

#[test]
fn chartfox_legacy_groups_map_like_fbw() {
    let http = Arc::new(ScriptedHttp::new().json("GET", "https://api.chartfox.org/v2/airports/XMPL/charts", 200, CF_LEGACY));
    let mut cf = ChartfoxSource::new(http.clone(), "tok");
    let list = cf.search("XMPL").unwrap();
    let cats: Vec<(String, Category)> = list.iter().map(|c| (c.name.clone(), c.category)).collect();
    assert!(cats.contains(&("GENERAL INFO".into(), Category::Reference)));
    assert!(cats.contains(&("AERODROME CHART".into(), Category::Airport)));
    assert!(cats.contains(&("SID RWY 09".into(), Category::Departure)));
    assert!(cats.contains(&("ILS RWY 27".into(), Category::Approach)));
    assert_eq!(list.len(), 4);
    assert_eq!(http.sent()[0].header_value("Authorization"), Some("Bearer tok"));
}

#[test]
fn chartfox_list_resolves_chart_urls_and_keeps_token_on_its_host() {
    let http = Arc::new(
        ScriptedHttp::new()
            .json("GET", "https://api.chartfox.org/v2/airports/XMPL/charts", 200, CF_LIST)
            .json("GET", "https://api.chartfox.org/v2/charts/c-2", 200, r#"{"data":{"url":"https://files.example.org/star.pdf"}}"#)
            .reply("GET", "https://files.example.org/star.pdf", 200, "application/pdf", b"%PDF-1.4".to_vec()),
    );
    let mut cf = ChartfoxSource::new(http.clone(), "tok");
    let list = cf.search("XMPL").unwrap();
    assert_eq!(list.iter().map(|c| c.category).collect::<Vec<_>>(), [Category::Airport, Category::Arrival, Category::Approach, Category::Other]);
    let data = cf.fetch(&list[1], false).unwrap();
    assert_eq!(data, ChartData::Bytes { content_type: "application/pdf".into(), bytes: b"%PDF-1.4".to_vec() });
    let sent = http.sent();
    assert_eq!(sent[1].header_value("Authorization"), Some("Bearer tok"));
    assert_eq!(sent[2].header_value("Authorization"), None);
}

#[test]
fn chartfox_needs_a_token_and_reports_401() {
    let mut cf = ChartfoxSource::new(Arc::new(ScriptedHttp::new()), "");
    assert!(matches!(cf.search("XMPL"), Err(ChartError::NotConfigured(_))));
    let mut cf = ChartfoxSource::new(Arc::new(ScriptedHttp::new().json("GET", "https://api.chartfox.org/", 401, "{}")), "old");
    assert_eq!(cf.search("XMPL"), Err(ChartError::NotSignedIn));
}

#[test]
fn builtin_aip_table_is_valid() {
    let t = Table::builtin();
    assert!(t.countries.len() >= 15);
    assert_eq!(t.country_for("KJFK").unwrap().authority, "FAA");
    assert_eq!(t.country_for("PANC").unwrap().authority, "FAA");
    assert_eq!(t.country_for("LEMD").unwrap().authority, "ENAIRE");
    assert_eq!(t.country_for("GCLP").unwrap().authority, "ENAIRE");
    assert_eq!(t.country_for("YSSY").unwrap().country, "Australia");
    assert_eq!(t.country_for("EGLL").unwrap().country, "United Kingdom");
    assert!(t.country_for("ZBAA").is_none());
    for c in &t.countries {
        assert!(c.index.starts_with("https://"), "{}", c.country);
        assert!(!c.prefixes.is_empty() && ["yes", "partial", "no"].contains(&c.verified.as_str()), "{}", c.country);
    }
    assert_eq!(t.categorise("RNAV (GNSS) SID RWY 36L"), Category::Departure);
    assert_eq!(t.categorise("LE_AD_2_LEXA_IAC_1_en"), Category::Approach);
    assert_eq!(t.categorise("INSIDE NOTES"), Category::Other);
}

#[test]
fn faa_dtpp_lists_one_airport_and_skips_deleted() {
    let today = days_from_civil(2026, 10, 22);
    let http = Arc::new(ScriptedHttp::new().reply("GET", "https://aeronav.faa.gov/d-tpp/2610/xml_data/d-tpp_Metafile.xml", 200, "text/xml", FAA_META.as_bytes().to_vec()));
    let mut aip = AipSource::new(http.clone(), Table::builtin()).with_today(Box::new(move || today));
    let list = aip.search("KXMP").unwrap();
    assert_eq!(list.iter().map(|c| (c.name.as_str(), c.category)).collect::<Vec<_>>(), [
        ("AIRPORT DIAGRAM", Category::Airport),
        ("ILS OR LOC RWY 9", Category::Approach),
        ("SAMPL TWO", Category::Arrival)
    ]);
    assert_eq!(list[0].day_url.as_deref(), Some("https://aeronav.faa.gov/d-tpp/2610/00001AD.PDF"));
    assert!(matches!(aip.search("KNOP"), Err(ChartError::NotCovered(_))));
    assert_eq!(http.sent().len(), 1, "index page fetched once");
}

#[test]
fn enaire_index_links_become_charts() {
    let http = Arc::new(
        ScriptedHttp::new()
            .reply("GET", "https://aip.enaire.es/AIP/AIP-en.html", 200, "text/html", ENAIRE.as_bytes().to_vec())
            .reply("GET", "https://aip.enaire.es/AIP/contenido_AIP/AD/AD2/LEXA/LE_AD_2_LEXA_ADC_1_en.pdf", 200, "application/octet-stream", b"%PDF".to_vec()),
    );
    let mut aip = AipSource::new(http, Table::builtin());
    let list = aip.search("LEXA").unwrap();
    assert_eq!(list.iter().map(|c| c.category).collect::<Vec<_>>(), [Category::Airport, Category::Departure, Category::Arrival, Category::Approach]);
    assert_eq!(list[2].name, "AD 2-LEXA STAR 1");
    assert_eq!(list[0].day_url.as_deref(), Some("https://aip.enaire.es/AIP/contenido_AIP/AD/AD2/LEXA/LE_AD_2_LEXA_ADC_1_en.pdf"));
    assert_eq!(aip.fetch(&list[0], false).unwrap(), ChartData::Bytes { content_type: "application/pdf".into(), bytes: b"%PDF".to_vec() });
}

#[test]
fn sectioned_index_keeps_only_the_airport() {
    let http = Arc::new(ScriptedHttp::new().reply("GET", "https://www.airservicesaustralia.com/aip/current/dap/AeroProcChartsTOC.htm", 200, "text/html", DAP.as_bytes().to_vec()));
    let mut aip = AipSource::new(http, Table::builtin());
    let list = aip.search("YXMP").unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[1].day_url.as_deref(), Some("https://www.airservicesaustralia.com/aip/current/dap/XMPXM02-100.pdf"));
    assert_eq!(list[1].category, Category::Approach);
}

#[test]
fn portal_and_unfree_countries() {
    let mut aip = AipSource::new(Arc::new(ScriptedHttp::new()), Table::builtin());
    let list = aip.search("NZAA").unwrap();
    assert_eq!(list[0].kind, ChartKind::Link);
    assert_eq!(aip.fetch(&list[0], false).unwrap(), ChartData::Link("https://www.aip.net.nz/".into()));
    assert!(matches!(aip.search("EDDF"), Err(ChartError::NotCovered(_))));
}

#[test]
fn url_joining_and_link_scan() {
    assert_eq!(join_url("https://a.example/x/y/index.html", "../z/c.pdf"), "https://a.example/x/z/c.pdf");
    assert_eq!(join_url("https://a.example/x/index.html", "/r.pdf"), "https://a.example/r.pdf");
    assert_eq!(join_url("https://a.example/x/index.html", "./q.pdf?a=1&amp;b=2"), "https://a.example/x/q.pdf?a=1&b=2");
    assert_eq!(links(r#"<A HREF="a.pdf"><b>One</b>  Two</A><a href=b.pdf>B</a>"#), [("a.pdf".to_string(), "One Two".to_string()), ("b.pdf".to_string(), "B".to_string())]);
}

#[test]
fn amdb_describes_local_maps_only() {
    let dir = scratch("amdb");
    std::fs::write(dir.join("XMPL.json"), "{}").unwrap();
    let mut amdb = AmdbSource::new(dir.clone());
    let list = amdb.search("XMPL").unwrap();
    assert_eq!(list[0].kind, ChartKind::Native);
    match amdb.fetch(&list[0], false).unwrap() {
        ChartData::Native(d) => {
            assert_eq!(d.path, dir.join("XMPL.json"));
            assert_eq!(d.projection, "EPSG:4326");
            assert!(d.layers.contains(&"runwayelement"));
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(amdb.search("NOPE"), Err(ChartError::NotCovered(_))));
}

#[test]
fn registry_caches_aip_but_never_navigraph() {
    let dir = scratch("cache");
    let http = Arc::new(
        ScriptedHttp::new()
            .reply("GET", "https://aip.enaire.es/AIP/AIP-en.html", 200, "text/html", ENAIRE.as_bytes().to_vec())
            .reply("GET", "https://aip.enaire.es/AIP/contenido_AIP/AD/AD2/LEXA/LE_AD_2_LEXA_ADC_1_en.pdf", 200, "application/pdf", b"%PDF-A".to_vec())
            .json("GET", "https://api.navigraph.com/v2/charts/XMPL", 200, NG_INDEX)
            .reply("GET", "https://api.navigraph.com/v2/charts/XMPL/xmpl101_d.png", 200, "image/png", b"one".to_vec())
            .reply("GET", "https://api.navigraph.com/v2/charts/XMPL/xmpl101_d.png", 200, "image/png", b"two".to_vec()),
    );
    let now = Arc::new(AtomicU64::new(0));
    let mut reg = Charts::new(Some(DiskCache::new(dir.clone(), 1 << 20)));
    reg.add(Box::new(AipSource::new(http.clone(), Table::builtin())));
    reg.add(Box::new(signed_in_navigraph(http.clone(), &now, 10_000)));
    assert_eq!(reg.source_ids(), ["aip", "navigraph"]);
    let aip = reg.search("aip", "lexa").unwrap();
    assert_eq!(reg.fetch(&aip[0], false).unwrap(), reg.fetch(&aip[0], false).unwrap());
    let ng = reg.search("navigraph", "XMPL").unwrap();
    let a = reg.fetch(&ng[0], false).unwrap();
    let b = reg.fetch(&ng[0], false).unwrap();
    assert_ne!(a, b, "Navigraph is fetched every time");
    assert_eq!(http.remaining(), 0);
    assert!(matches!(reg.search("aip", "TOOLONG"), Err(ChartError::NotCovered(_))));
    assert!(charts::group(&aip).iter().map(|g| g.0).eq([Category::Airport, Category::Departure, Category::Arrival, Category::Approach]));
}

#[test]
fn disk_cache_evicts_oldest_over_limit() {
    let dir = scratch("evict");
    let mut cache = DiskCache::new(dir, 10);
    cache.put("a", "application/pdf", b"aaaaaa").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    cache.put("b", "application/pdf", b"bbbbbb").unwrap();
    assert!(cache.get("a").is_none());
    assert_eq!(cache.get("b"), Some(("application/pdf".into(), b"bbbbbb".to_vec())));
    cache.put("huge", "x", &[0u8; 11]).unwrap();
    assert!(cache.get("huge").is_none());
    assert!(cache.size() <= 10);
}

#[test]
fn chart_json_shape() {
    let v = charts::navigraph::parse_index("XMPL", &serde_json::from_str(NG_INDEX).unwrap()).unwrap()[0].to_json();
    assert_eq!(v["category_code"], "APT");
    assert_eq!(v["kind"], "png");
    assert_eq!(v["georef"]["latlng"]["lat2"], 50.1);
    assert!(v.get("day_url").is_none(), "image URLs stay inside the plugin");
}
