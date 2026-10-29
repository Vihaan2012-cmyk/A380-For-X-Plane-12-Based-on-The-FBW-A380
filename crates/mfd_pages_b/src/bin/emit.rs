use mfd_pages_b::fuel::{fuel_load_page, FuelModel};
use mfd_pages_b::init::{init_page, InitModel};
use mfd_pages_b::perf::{perf_page, PerfModel};
use std::fs;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: emit <init|fuel|perf> <model.json> <out.json>");
        std::process::exit(2);
    }
    let model = fs::read_to_string(&args[2]).expect("model file");
    let tree = match args[1].as_str() {
        "init" => init_page(&serde_json::from_str::<InitModel>(&model).expect("init model")),
        "fuel" => fuel_load_page(&serde_json::from_str::<FuelModel>(&model).expect("fuel model")),
        "perf" => perf_page(&serde_json::from_str::<PerfModel>(&model).expect("perf model")),
        other => panic!("unknown page {}", other),
    };
    fs::write(&args[3], tree.to_json()).expect("write");
}
