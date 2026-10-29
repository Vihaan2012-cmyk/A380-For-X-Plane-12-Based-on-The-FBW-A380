use std::path::PathBuf;

use gpws::obstacles::{ObstacleDb, Record, WithTerrain};
use gpws::taws::{offset_nm, Obstacle, ObstacleSource};

fn fixture() -> ObstacleDb {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/obstacles/obstacles.bin");
    ObstacleDb::load(&path).expect("fixture loads")
}

fn within(db: &ObstacleDb, lat: f64, lon: f64, r: f64) -> Vec<Record> {
    let mut out = Vec::new();
    db.within(lat, lon, r, &mut out);
    out
}

#[test]
fn the_fixture_holds_faa_and_osm_obstacles_without_dismantled_or_duplicates() {
    let db = fixture();
    assert_eq!(db.len(), 5);
    let all = within(&db, 0., 0., 60. * 200.);
    assert_eq!(all.len(), 5);
    assert!(all.iter().all(|r| !(r.lat > 40.515 && r.lat < 40.52)), "the dismantled tower is dropped");
    assert_eq!(all.iter().filter(|r| (r.lat - 40.5).abs() < 0.001 && (r.lon + 74.).abs() < 0.001).count(), 1, "the OSM copy of an FAA tower is dropped");
}

#[test]
fn faa_records_keep_height_and_elevation_to_a_few_metres() {
    let db = fixture();
    let near = within(&db, 40.5, -74., 0.2);
    assert_eq!(near.len(), 1);
    let r = near[0];
    assert!((r.lat - 40.5).abs() < 2e-5 && (r.lon + 74.).abs() < 2e-5);
    assert_eq!(r.agl_ft, 1000.);
    assert_eq!(r.amsl_ft, Some(1050.));
}

#[test]
fn radius_is_a_circle_not_the_tile() {
    let db = fixture();
    assert_eq!(within(&db, 40.5, -74., 0.5).len(), 1);
    assert_eq!(within(&db, 40.5, -74., 0.7).len(), 2);
    let (lat, lon) = offset_nm(40.5, -74., 0., 0.6);
    assert_eq!(within(&db, lat, lon, 0.5).len(), 0);
}

#[test]
fn queries_cross_the_antimeridian() {
    let db = fixture();
    let east = within(&db, -16.5, 179.99, 1.);
    let west = within(&db, -16.5, -179.99, 1.);
    assert_eq!(east.len(), 2);
    assert_eq!(west.len(), 2);
}

#[test]
fn osm_heights_in_feet_and_metres_are_converted() {
    let db = fixture();
    let near = within(&db, -16.5, -179.995, 0.1);
    assert_eq!(near.len(), 1);
    assert_eq!(near[0].agl_ft, 656.);
    assert_eq!(near[0].amsl_ft, None);
    let chimney = within(&db, -16.5, 179.995, 0.1);
    assert_eq!(chimney[0].agl_ft, 328.);
    assert_eq!(chimney[0].amsl_ft, Some(361.));
}

#[test]
fn the_taws_source_needs_terrain_for_obstacles_without_a_base_elevation() {
    let db = fixture();
    let mut bare: Vec<Obstacle> = Vec::new();
    db.obstacles_near(-33.87, 151.21, 1., &mut bare);
    assert!(bare.is_empty());

    let terrain = |_lat: f64, _lon: f64| Some(100.);
    let source = WithTerrain { db: &db, terrain: &terrain };
    let mut with_ground = Vec::new();
    source.obstacles_near(-33.87, 151.21, 1., &mut with_ground);
    assert_eq!(with_ground.len(), 1);
    assert_eq!(with_ground[0].top_ft, 100. + 886.);

    let mut faa = Vec::new();
    source.obstacles_near(40.5, -74., 0.2, &mut faa);
    assert_eq!(faa[0].top_ft, 1050.);
}

#[test]
fn damaged_files_are_refused() {
    let good = std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/obstacles/obstacles.bin")).unwrap();
    assert!(ObstacleDb::from_bytes(good[..good.len() - 1].to_vec()).is_err());
    let mut bad_magic = good.clone();
    bad_magic[0] = b'X';
    assert!(ObstacleDb::from_bytes(bad_magic).is_err());
    assert!(ObstacleDb::from_bytes(Vec::new()).is_err());
    let empty = [b"OBS1".as_slice(), &[1, 0, 0, 0], &[0; 8]].concat();
    assert!(ObstacleDb::from_bytes(empty).unwrap().is_empty());
}
