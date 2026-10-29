use std::path::Path;

use crate::taws::{ne_nm, Obstacle, ObstacleSource, TerrainSampler};

pub const MAGIC: &[u8; 4] = b"OBS1";
pub const VERSION: u16 = 1;
const HEADER_LEN: usize = 16;
const INDEX_LEN: usize = 12;
const RECORD_LEN: usize = 8;
const UNKNOWN_AMSL: i16 = i16::MIN;
const FRACTION: f64 = 65536.;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Format(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "obstacle database: {e}"),
            Error::Format(why) => write!(f, "obstacle database: {why}"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Record {
    pub lat: f64,
    pub lon: f64,
    pub agl_ft: f64,
    pub amsl_ft: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
struct Tile {
    key: u32,
    first: u32,
    count: u32,
}

pub struct ObstacleDb {
    data: Vec<u8>,
    tiles: Vec<Tile>,
    records_at: usize,
    total: usize,
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn tile_key(lat_tile: i32, lon_tile: i32) -> u32 {
    ((lat_tile + 90) * 360 + (lon_tile + 180).rem_euclid(360)) as u32
}

impl ObstacleDb {
    pub fn load(path: &Path) -> Result<ObstacleDb, Error> {
        ObstacleDb::from_bytes(std::fs::read(path).map_err(Error::Io)?)
    }

    pub fn from_bytes(data: Vec<u8>) -> Result<ObstacleDb, Error> {
        if data.len() < HEADER_LEN || &data[..4] != MAGIC {
            return Err(Error::Format("not an OBS1 file"));
        }
        if u16_at(&data, 4) != VERSION {
            return Err(Error::Format("unsupported version"));
        }
        let n_tiles = u32_at(&data, 8) as usize;
        let total = u32_at(&data, 12) as usize;
        let records_at = HEADER_LEN + n_tiles * INDEX_LEN;
        if data.len() != records_at + total * RECORD_LEN {
            return Err(Error::Format("length does not match header"));
        }
        let mut tiles = Vec::with_capacity(n_tiles);
        let mut expected_first = 0u32;
        for i in 0..n_tiles {
            let at = HEADER_LEN + i * INDEX_LEN;
            let tile = Tile { key: u32_at(&data, at), first: u32_at(&data, at + 4), count: u32_at(&data, at + 8) };
            if tile.first != expected_first || tiles.last().is_some_and(|t: &Tile| t.key >= tile.key) || tile.key >= 180 * 360 {
                return Err(Error::Format("tile index out of order"));
            }
            expected_first += tile.count;
            tiles.push(tile);
        }
        if expected_first as usize != total {
            return Err(Error::Format("tile counts do not add up"));
        }
        Ok(ObstacleDb { data, tiles, records_at, total })
    }

    pub fn len(&self) -> usize {
        self.total
    }

    pub fn is_empty(&self) -> bool {
        self.total == 0
    }

    fn tile(&self, key: u32) -> Option<Tile> {
        self.tiles.binary_search_by_key(&key, |t| t.key).ok().map(|i| self.tiles[i])
    }

    fn record(&self, tile: Tile, i: u32) -> Record {
        let at = self.records_at + (tile.first + i) as usize * RECORD_LEN;
        let lat_tile = (tile.key / 360) as f64 - 90.;
        let lon_tile = (tile.key % 360) as f64 - 180.;
        let amsl = u16_at(&self.data, at + 6) as i16;
        Record {
            lat: lat_tile + u16_at(&self.data, at) as f64 / FRACTION,
            lon: lon_tile + u16_at(&self.data, at + 2) as f64 / FRACTION,
            agl_ft: u16_at(&self.data, at + 4) as f64,
            amsl_ft: (amsl != UNKNOWN_AMSL).then_some(amsl as f64),
        }
    }

    pub fn within(&self, lat_deg: f64, lon_deg: f64, radius_nm: f64, out: &mut Vec<Record>) {
        if !(lat_deg.is_finite() && lon_deg.is_finite() && radius_nm >= 0.) {
            return;
        }
        let dlat = radius_nm / 60.;
        let lat_lo = ((lat_deg - dlat).floor() as i32).max(-90);
        let lat_hi = ((lat_deg + dlat).floor() as i32).min(89);
        let worst_cos = (lat_deg.abs() + dlat).min(90.).to_radians().cos();
        let dlon = if worst_cos > 1e-6 { radius_nm / (60. * worst_cos) } else { 360. };
        let (lon_lo, lon_hi) = if dlon >= 179. {
            (-180, 179)
        } else {
            ((lon_deg - dlon).floor() as i32, (lon_deg + dlon).floor() as i32)
        };
        let r2 = radius_nm * radius_nm;
        for lat_tile in lat_lo..=lat_hi {
            for lon_tile in lon_lo..=lon_hi {
                let Some(tile) = self.tile(tile_key(lat_tile, lon_tile)) else { continue };
                for i in 0..tile.count {
                    let r = self.record(tile, i);
                    let (n, e) = ne_nm(lat_deg, lon_deg, r.lat, r.lon);
                    if n * n + e * e <= r2 {
                        out.push(r);
                    }
                }
            }
        }
    }
}

impl ObstacleSource for ObstacleDb {
    fn obstacles_near(&self, lat_deg: f64, lon_deg: f64, radius_nm: f64, out: &mut Vec<Obstacle>) {
        let mut found = Vec::new();
        self.within(lat_deg, lon_deg, radius_nm, &mut found);
        out.extend(found.into_iter().filter_map(|r| r.amsl_ft.map(|top_ft| Obstacle { lat: r.lat, lon: r.lon, top_ft })));
    }
}

pub struct WithTerrain<'a, T: TerrainSampler + ?Sized> {
    pub db: &'a ObstacleDb,
    pub terrain: &'a T,
}

impl<T: TerrainSampler + ?Sized> ObstacleSource for WithTerrain<'_, T> {
    fn obstacles_near(&self, lat_deg: f64, lon_deg: f64, radius_nm: f64, out: &mut Vec<Obstacle>) {
        let mut found = Vec::new();
        self.db.within(lat_deg, lon_deg, radius_nm, &mut found);
        for r in found {
            let top_ft = r.amsl_ft.or_else(|| self.terrain.elevation_ft(r.lat, r.lon).map(|base| base + r.agl_ft));
            if let Some(top_ft) = top_ft {
                out.push(Obstacle { lat: r.lat, lon: r.lon, top_ft });
            }
        }
    }
}
