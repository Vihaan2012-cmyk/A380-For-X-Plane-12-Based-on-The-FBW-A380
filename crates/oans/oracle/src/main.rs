mod scenarios;
mod transpile;

use std::path::{Path, PathBuf};

use rquickjs::loader::{ImportAttributes, Loader, Resolver};
use rquickjs::module::Declared;
use rquickjs::{CatchResultExt, Context, Ctx, Function, Module, Runtime};
use serde_json::{json, Value};

const FBW: &str = "E:/fbw-int/fbw-aircraft";

fn alias(name: &str) -> Option<String> {
    let common = format!("{FBW}/fbw-common/src/systems");
    let prefixes = [
        ("@fbw/shared/", format!("{common}/shared/src/")),
        ("@fbw/oans/", format!("{common}/oans/")),
        ("@fbw/oanc/", format!("{common}/instruments/src/OANC/")),
        ("@fbw/a380x/", format!("{FBW}/fbw-a380x/src/systems/")),
    ];
    for (p, dir) in prefixes {
        if let Some(rest) = name.strip_prefix(p) {
            return Some(format!("{dir}{rest}"));
        }
    }
    None
}

struct Res {
    shim: PathBuf,
}

impl Resolver for Res {
    fn resolve<'js>(&mut self, _ctx: &Ctx<'js>, base: &str, name: &str, _attributes: Option<ImportAttributes<'js>>) -> rquickjs::Result<String> {
        let shim = |f: &str| format!("{}/{f}", self.shim.display()).replace('\\', "/");
        let r = match name {
            "@microsoft/msfs-sdk" => shim("msfs_sdk.js"),
            "@flybywiresim/fbw-sdk" => shim("fbw_sdk.js"),
            "@fmgc/flightplanning/NavigationDatabaseService" | "@fmgc/NavigationDatabase" => shim("fmgc_db.js"),
            "./msfs_sdk.js" => shim("msfs_sdk.js"),
            "./fbw_sdk.js" => shim("fbw_sdk.js"),
            "msfs-geo" => "virtual:msfs-geo".to_string(),
            "@turf/turf" => "virtual:turf".to_string(),
            "geojson" => "virtual:empty".to_string(),
            "." | "./" => shim("label_style.js"),
            "virtual:driver" => name.to_string(),
            n if n.ends_with("types/ProcedureLeg") => shim("procedure_leg.js"),
            _ => {
                if let Some(p) = alias(name) {
                    p
                } else if name.starts_with('.') {
                    let dir = Path::new(base).parent().unwrap_or(Path::new("."));
                    let joined = normalize(&dir.join(name));
                    let candidates = [joined.clone(), format!("{joined}.ts"), format!("{joined}.tsx"), format!("{joined}.js"), format!("{joined}/index.ts")];
                    let found = candidates.into_iter().find(|c| Path::new(c).is_file()).ok_or_else(|| rquickjs::Error::new_resolving_message(base, name, "not found"))?;
                    if found.ends_with("shared/src/index.ts") {
                        shim("shared_index.js")
                    } else if found.ends_with("systems/navdata/index.ts") {
                        shim("navdata.js")
                    } else {
                        found
                    }
                } else if let Some(p) = node_module(base, name) {
                    p
                } else {
                    "virtual:empty".to_string()
                }
            }
        };
        Ok(r)
    }
}

fn node_module(base: &str, name: &str) -> Option<String> {
    let nm = format!("{FBW}/node_modules");
    if let Some(rest) = name.strip_prefix("@turf/") {
        return Some(format!("{nm}/@turf/{rest}/dist/esm/index.js"));
    }
    if !base.contains("node_modules") {
        return None;
    }
    let pkg: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(format!("{nm}/{name}/package.json")).ok()?).ok()?;
    let entry = pkg["module"].as_str().or_else(|| pkg["main"].as_str())?;
    Some(normalize(&Path::new(&format!("{nm}/{name}")).join(entry)))
}

fn normalize(p: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    for c in p.to_string_lossy().replace('\\', "/").split('/') {
        match c {
            "." | "" => {
                if parts.is_empty() {
                    parts.push(String::new());
                }
            }
            ".." => {
                parts.pop();
            }
            other => parts.push(other.to_string()),
        }
    }
    parts.join("/")
}

struct Load {
    shim: PathBuf,
}

impl Loader for Load {
    fn load<'js>(&mut self, ctx: &Ctx<'js>, name: &str, _attributes: Option<ImportAttributes<'js>>) -> rquickjs::Result<Module<'js, Declared>> {
        let code = match name {
            "virtual:empty" => "export {};".to_string(),
            "virtual:turf" => "export { lineOffset } from '@turf/line-offset'; export { booleanDisjoint } from '@turf/boolean-disjoint'; export { booleanContains } from '@turf/boolean-contains'; export { lineString, point, polygon } from '@turf/helpers'; export { along } from '@turf/along'; export { length } from '@turf/length';".to_string(),
            "virtual:msfs-geo" => {
                let src = std::fs::read_to_string(format!("{FBW}/node_modules/msfs-geo/dist/index.js")).map_err(|e| rquickjs::Error::new_loading_message(name, e.to_string()))?;
                let mut names: Vec<String> = Vec::new();
                for line in src.lines() {
                    if let Some(rest) = line.strip_prefix("exports.") {
                        if let Some((n, _)) = rest.split_once(" =") {
                            names.push(n.to_string());
                        }
                    }
                }
                let mut out = String::from("const __g = (function () { const exports = {}; const module = { exports }; const require = () => ({});
");
                out.push_str(&src);
                out.push_str("
return exports; })();
");
                for n in names {
                    out.push_str(&format!("export const {n} = __g.{n};
"));
                }
                out
            }
            "virtual:driver" => std::fs::read_to_string(self.shim.join("driver.js")).map_err(|e| rquickjs::Error::new_loading_message(name, e.to_string()))?,
            _ => {
                let path = Path::new(name);
                let text = std::fs::read_to_string(path).map_err(|e| rquickjs::Error::new_loading_message(name, e.to_string()))?;
                if matches!(path.extension().and_then(|e| e.to_str()), Some("ts" | "tsx")) {
                    transpile::transpile(path, &text).map_err(|m| rquickjs::Error::new_loading_message(name, m))?
                } else {
                    text
                }
            }
        };
        Module::declare(ctx.clone(), name, code)
    }
}

fn drain(ctx: &Ctx<'_>) {
    while ctx.execute_pending_job() {}
}

fn run_scenario(shim: &Path, sc: &Value) -> Result<Value, String> {
    let rt = Runtime::new().map_err(|e| e.to_string())?;
    rt.set_loader(Res { shim: shim.to_path_buf() }, Load { shim: shim.to_path_buf() });
    let ctx = Context::full(&rt).map_err(|e| e.to_string())?;
    let mut snapshots = Vec::new();
    ctx.with(|ctx| -> Result<(), String> {
        ctx.eval::<(), _>("globalThis.babelHelpers = { defineProperty(o, k, v) { Object.defineProperty(o, k, { value: v, enumerable: true, configurable: true, writable: true }); return o; } };").catch(&ctx).map_err(|e| e.to_string())?;
        let promise: rquickjs::Promise = Module::import(&ctx, "virtual:driver").catch(&ctx).map_err(|e| e.to_string())?;
        drain(&ctx);
        if let rquickjs::promise::PromiseState::Rejected = promise.state() {
            let err = promise.result::<()>().unwrap_or(Ok(())).catch(&ctx);
            return Err(err.err().map_or_else(|| "driver failed".to_string(), |e| e.to_string()));
        }
        let init: Function = ctx.globals().get("__init").map_err(|e| format!("driver failed to load: {e}"))?;
        init.call::<_, ()>((sc.to_string(),)).catch(&ctx).map_err(|e| e.to_string())?;
        drain(&ctx);
        let step: Function = ctx.globals().get("__step").map_err(|e| e.to_string())?;
        let snap: Function = ctx.globals().get("__snapshot").map_err(|e| e.to_string())?;
        for (i, s) in sc["steps"].as_array().ok_or("steps")?.iter().enumerate() {
            step.call::<_, ()>((s.to_string(),)).catch(&ctx).map_err(|e| format!("step {i} {s}: {e}"))?;
            drain(&ctx);
            let text: String = snap.call(()).catch(&ctx).map_err(|e| e.to_string())?;
            let mut v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            v["step"] = json!(i);
            snapshots.push(v);
        }
        Ok(())
    })?;
    Ok(json!({ "scenario": sc, "snapshots": snapshots }))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out_dir = PathBuf::from(args.get(1).cloned().unwrap_or_else(|| "E:/fbw-build/oans-oracle".into()));
    let shim = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("shim");
    std::fs::create_dir_all(&out_dir).expect("output dir");
    let mut failed = false;
    for sc in scenarios::all() {
        let name = sc["name"].as_str().unwrap_or("scenario").to_string();
        match run_scenario(&shim, &sc) {
            Ok(golden) => {
                let path = out_dir.join(format!("{name}.json"));
                std::fs::write(&path, serde_json::to_string_pretty(&golden).unwrap()).expect("write golden");
                println!("recorded {}", path.display());
            }
            Err(e) => {
                eprintln!("{name}: {e}");
                failed = true;
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}
