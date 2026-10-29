use crate::palette;
use crate::projection::LatLon;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AmdbClass {
    Water,
    Apron,
    Shoulder,
    Taxiway,
    Stand,
    Runway,
    Building,
    Marking,
    Guidance,
    Holding,
}

impl AmdbClass {
    pub fn of_layer(layer: &str) -> Option<AmdbClass> {
        Some(match layer {
            "water" => AmdbClass::Water,
            "apronelement" | "deicingarea" | "serviceroad" | "constructionarea" => AmdbClass::Apron,
            "runwayshoulder" | "taxiwayshoulder" | "blastpad" | "stopway" => AmdbClass::Shoulder,
            "taxiwayelement" => AmdbClass::Taxiway,
            "parkingstandarea" => AmdbClass::Stand,
            "runwayelement" | "runwayintersection" | "runwaydisplacedarea" => AmdbClass::Runway,
            "verticalpolygonalstructure" => AmdbClass::Building,
            "runwaymarking" | "paintedcenterline" | "runwaythreshold" => AmdbClass::Marking,
            "taxiwayguidanceline" | "standguidanceline" | "runwayexitline" => AmdbClass::Guidance,
            "taxiwayholdingposition" => AmdbClass::Holding,
            _ => return None,
        })
    }

    pub fn style(self) -> (&'static str, &'static str, &'static str) {
        match self {
            AmdbClass::Water => (palette::FIELD, "none", "0"),
            AmdbClass::Apron => (palette::CARD, "none", "0"),
            AmdbClass::Shoulder => ("#303236", "none", "0"),
            AmdbClass::Taxiway => (palette::CTL, "none", "0"),
            AmdbClass::Stand => ("#34363a", palette::DIM, "1"),
            AmdbClass::Runway => ("#4a4c50", "none", "0"),
            AmdbClass::Building => (palette::DIM, "none", "0"),
            AmdbClass::Marking => ("none", palette::TEXT, "1"),
            AmdbClass::Guidance => ("none", palette::WARN, "1"),
            AmdbClass::Holding => ("none", palette::BAD, "2"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AmdbFeature {
    pub class: AmdbClass,
    pub rings: Vec<Vec<LatLon>>,
    pub closed: bool,
    pub label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AmdbAirport {
    pub icao: String,
    pub arp: Option<LatLon>,
    pub features: Vec<AmdbFeature>,
}

fn ring(v: &Value) -> Vec<LatLon> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|p| {
                    let p = p.as_array()?;
                    Some(LatLon::new(p.get(1)?.as_f64()?, p.first()?.as_f64()?))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn label_of(props: &Value, class: AmdbClass) -> Option<String> {
    let key = match class {
        AmdbClass::Guidance => "idlin",
        AmdbClass::Stand => "idstd",
        AmdbClass::Runway => "idrwy",
        _ => return None,
    };
    props.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
}

pub fn parse(icao: &str, data: &Value) -> AmdbAirport {
    let mut features = Vec::new();
    let mut arp = None;
    let Some(layers) = data.as_object() else {
        return AmdbAirport { icao: icao.to_string(), arp, features };
    };
    for (name, fc) in layers {
        let feats = fc.get("features").and_then(Value::as_array).cloned().unwrap_or_default();
        if name == "aerodromereferencepoint" {
            arp = feats.first().and_then(|f| ring(&Value::Array(vec![f.get("geometry")?.get("coordinates")?.clone()])).first().copied());
            continue;
        }
        let Some(class) = AmdbClass::of_layer(name) else { continue };
        for f in feats {
            let Some(g) = f.get("geometry") else { continue };
            let coords = g.get("coordinates").cloned().unwrap_or(Value::Null);
            let label = f.get("properties").and_then(|p| label_of(p, class));
            let (rings, closed): (Vec<Vec<LatLon>>, bool) = match g.get("type").and_then(Value::as_str) {
                Some("Polygon") => (coords.as_array().map(|r| r.iter().map(ring).collect()).unwrap_or_default(), true),
                Some("MultiPolygon") => (
                    coords
                        .as_array()
                        .map(|ps| ps.iter().filter_map(Value::as_array).flat_map(|p| p.iter().map(ring)).collect())
                        .unwrap_or_default(),
                    true,
                ),
                Some("LineString") => (vec![ring(&coords)], false),
                Some("MultiLineString") => (coords.as_array().map(|r| r.iter().map(ring).collect()).unwrap_or_default(), false),
                _ => continue,
            };
            let rings: Vec<Vec<LatLon>> = rings.into_iter().filter(|r| r.len() >= 2).collect();
            if !rings.is_empty() {
                features.push(AmdbFeature { class, rings, closed, label });
            }
        }
    }
    features.sort_by_key(|f| f.class);
    AmdbAirport { icao: icao.to_string(), arp, features }
}
