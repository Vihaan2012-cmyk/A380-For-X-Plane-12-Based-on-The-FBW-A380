use mfd_pages_d::{atccom, fcubkup, surv};
use std::fs;

fn main() {
    let manifest = std::env::args().nth(1).expect("manifest path");
    for line in fs::read_to_string(&manifest).expect("manifest").lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 3 {
            continue;
        }
        let model = fs::read_to_string(cols[1]).expect("model");
        let node = match cols[0] {
            "connect" => atccom::connect_page(&serde_json::from_str(&model).expect("connect model")),
            "request" => atccom::request_page(&serde_json::from_str(&model).expect("request model")),
            "msg_record" => atccom::msg_record_page(&serde_json::from_str(&model).expect("msg record model")),
            "datis" => atccom::datis_page(&serde_json::from_str(&model).expect("datis model")),
            "controls" => surv::controls_page(&serde_json::from_str(&model).expect("controls model")),
            "status" => surv::status_page(&serde_json::from_str(&model).expect("status model")),
            "afs" => fcubkup::afs_page(&serde_json::from_str(&model).expect("afs model")),
            "efis" => fcubkup::efis_page(&serde_json::from_str(&model).expect("efis model")),
            other => panic!("unknown kind {other}"),
        };
        fs::write(cols[2], node.to_json()).expect("write json");
    }
}
