use std::path::PathBuf;

use crate::{AmdbDescriptor, Category, Chart, ChartData, ChartError, ChartKind, ChartSource};

pub const DIAGRAM_LAYERS: [&str; 8] = [
    "apronelement",
    "taxiwayelement",
    "runwayelement",
    "parkingstandarea",
    "verticalpolygonalstructure",
    "taxiwayguidanceline",
    "standguidanceline",
    "parkingstandlocation",
];

/// The OANS's local AMDB folder (`<ICAO>.json`, EPSG:4326 GeoJSON per layer). Draws nothing itself.
pub struct AmdbSource {
    root: PathBuf,
}

impl AmdbSource {
    pub fn new(root: PathBuf) -> AmdbSource {
        AmdbSource { root }
    }

    fn file(&self, icao: &str) -> PathBuf {
        self.root.join(format!("{}.json", icao.to_ascii_uppercase()))
    }
}

impl ChartSource for AmdbSource {
    fn id(&self) -> &'static str {
        "amdb"
    }

    fn search(&mut self, icao: &str) -> Result<Vec<Chart>, ChartError> {
        if !self.file(icao).is_file() {
            return Err(ChartError::NotCovered(format!("no airport map for {icao} in the AMDB folder")));
        }
        Ok(vec![Chart::new("amdb", icao, &format!("AMDB-{icao}"), "Airport map", Category::Airport, ChartKind::Native)])
    }

    fn fetch(&mut self, chart: &Chart, _night: bool) -> Result<ChartData, ChartError> {
        let path = self.file(&chart.icao);
        if !path.is_file() {
            return Err(ChartError::NotFound);
        }
        Ok(ChartData::Native(AmdbDescriptor { icao: chart.icao.clone(), path, projection: "EPSG:4326", layers: DIAGRAM_LAYERS.to_vec() }))
    }

    fn cacheable(&self) -> bool {
        false
    }
}
