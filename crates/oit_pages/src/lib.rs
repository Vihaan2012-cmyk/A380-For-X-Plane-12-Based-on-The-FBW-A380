pub mod avncs;
pub mod flt_ops;
pub mod inflight_model;
pub mod navigator;
pub mod ops_pages;
pub mod perf_common;
pub mod perf_pages;
pub mod perf_model;
pub mod shell;
pub mod to_compute;
pub mod widgets;

pub use shell::{build, OitModel};

pub fn build_json(model_json: &str) -> Result<String, serde_json::Error> {
    let model: OitModel = serde_json::from_str(model_json)?;
    Ok(build(&model).to_json())
}
