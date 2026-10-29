use mfd_pages_a::{fpln_screen, FplnModel, ShellModel};
use serde::Deserialize;

#[derive(Deserialize, Default)]
#[serde(default)]
struct CaptureModel {
    shell: ShellModel,
    fpln: FplnModel,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let text = std::fs::read_to_string(&args[1]).expect("model");
    let model: CaptureModel = serde_json::from_str(&text).expect("model json");
    let tree = fpln_screen(&model.shell, &model.fpln);
    std::fs::write(&args[2], tree.to_json()).expect("write");
}
