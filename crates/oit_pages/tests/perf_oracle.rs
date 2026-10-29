use oit_pages::perf_model::*;
use serde_json::Value;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/perf_oracle.json")).unwrap()
}

fn f(v: &Value, k: &str) -> f64 {
    match &v[k] {
        Value::String(s) if s == "inf" => f64::INFINITY,
        Value::String(s) if s == "-inf" => f64::NEG_INFINITY,
        x => x.as_f64().unwrap_or_else(|| panic!("missing {k}")),
    }
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap()
}

fn close(a: f64, b: f64, what: &str) {
    if a.is_infinite() || b.is_infinite() {
        assert_eq!(a, b, "{what}");
        return;
    }
    let tol = 1e-9 * b.abs().max(1.0);
    assert!((a - b).abs() <= tol, "{what}: rust {a} oracle {b}");
}

fn calibration(v: &Value) -> PerfCalibration {
    PerfCalibration { thrust: f(v, "thrust"), roll: f(v, "roll"), brake: f(v, "brake"), takeoffs: 0, landings: 0 }
}

#[test]
fn atmosphere_matches_oracle() {
    for row in fixture()["atmos"].as_array().unwrap() {
        let a = atmos(f(row, "elev"), f(row, "qnh"), f(row, "oat"));
        let o = &row["atm"];
        close(a.pa, f(o, "pa"), "pa");
        close(a.delta, f(o, "delta"), "delta");
        close(a.isa, f(o, "isa"), "isa");
        close(a.sigma, f(o, "sigma"), "sigma");
        close(a.rho, f(o, "rho"), "rho");
    }
}

#[test]
fn vls_and_trim_match_oracle() {
    let fx = fixture();
    for row in fx["vls"].as_array().unwrap() {
        let tons = f(row, "tons");
        let cg = f(row, "cg");
        let v = match s(row, "conf") {
            "1+F" => vls_to(ToConf::OnePlusF, tons, cg),
            "2" => vls_to(ToConf::Two, tons, cg),
            "3" => vls_to(ToConf::Three, tons, cg),
            _ => vls_ldg(LdgConf::Full, tons, cg),
        };
        close(v, f(row, "vls"), &format!("vls {} {tons} {cg}", s(row, "conf")));
    }
    for row in fx["trim"].as_array().unwrap() {
        close(trim_for(f(row, "cg")), f(row, "trim"), "trim");
    }
}

#[test]
fn takeoff_matches_oracle() {
    for row in fixture()["takeoff"].as_array().unwrap() {
        let i = &row["input"];
        let o = &row["out"];
        let conf = match s(i, "conf") {
            "1+F" => ToConfChoice::OnePlusF,
            "2" => ToConfChoice::Two,
            "3" => ToConfChoice::Three,
            _ => ToConfChoice::Auto,
        };
        let ai = match s(i, "ai") {
            "ENG" => AntiIce::Eng,
            "ENG+WING" => AntiIce::EngWing,
            _ => AntiIce::Off,
        };
        let input = TakeoffInput {
            atm: atmos(f(i, "elev"), f(i, "qnh"), f(i, "oat")),
            oat: f(i, "oat"),
            tow_t: f(i, "tow"),
            cg: f(i, "cg"),
            conf,
            flex: i["flex"].as_bool().unwrap(),
            derate_pct: 0.0,
            rating_kn: 356.81,
            packs: i["packs"].as_bool().unwrap(),
            ai,
            slope: f(i, "slope"),
            cond: if s(i, "cond") == "WET" { TO_COND_WET } else { TO_COND_DRY },
            toda: f(i, "toda"),
            asda: f(i, "asda"),
            lineup: lineup(s(i, "angle")),
            hw: f(i, "hw"),
            cal: calibration(&i["cal"]),
        };
        let r = takeoff(&input);
        let tag = format!("{} {} {}", f(i, "tow"), s(i, "conf"), s(i, "cond"));
        assert_eq!(r.ok, o["ok"].as_bool().unwrap(), "{tag} ok");
        let limits: Vec<&str> = o["limits"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
        assert_eq!(r.limits.iter().map(|x| x.as_str()).collect::<Vec<_>>(), limits, "{tag} limits");
        close(r.v1, f(o, "v1"), &format!("{tag} v1"));
        close(r.vr, f(o, "vr"), &format!("{tag} vr"));
        close(r.v2, f(o, "v2"), &format!("{tag} v2"));
        assert_eq!(r.conf.label(), s(o, "conf"), "{tag} conf");
        match o["flex"].as_f64() {
            Some(x) => close(r.flex.expect("flex"), x, &format!("{tag} flex")),
            None => assert!(r.flex.is_none(), "{tag} flex"),
        }
        close(r.tod_aeo, f(o, "todAeo"), &format!("{tag} todAeo"));
        close(r.tod_oei, f(o, "todOei"), &format!("{tag} todOei"));
        close(r.asd, f(o, "asd"), &format!("{tag} asd"));
        close(r.toda, f(o, "toda"), &format!("{tag} toda"));
        close(r.asda, f(o, "asda"), &format!("{tag} asda"));
        close(r.grad2, f(o, "grad2"), &format!("{tag} grad2"));
        close(r.trim, f(o, "trim"), &format!("{tag} trim"));
        close(r.limit_tow_t, f(o, "limitTowT"), &format!("{tag} limitTowT"));
    }
}

#[test]
fn landing_matches_oracle() {
    for row in fixture()["landing"].as_array().unwrap() {
        let i = &row["input"];
        let o = &row["out"];
        let input = LandingInput {
            atm: atmos(f(i, "elev"), f(i, "qnh"), f(i, "oat")),
            lw_t: f(i, "lw"),
            cg: f(i, "cg"),
            conf: if s(i, "conf") == "FULL" { LdgConf::Full } else { LdgConf::Three },
            mu: f(i, "mu"),
            rev: i["rev"].as_bool().unwrap(),
            autoland: i["autoland"].as_bool().unwrap(),
            slope: f(i, "slope"),
            hw: f(i, "hw"),
            lda: f(i, "lda"),
            cal: calibration(&i["cal"]),
        };
        let modes: Vec<&str> = row["modes"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
        let r = landing(&input, &modes);
        close(r.vls, f(o, "vls"), "vls");
        close(r.vapp, f(o, "vapp"), "vapp");
        for (a, b) in r.rows.iter().zip(o["rows"].as_array().unwrap()) {
            assert_eq!(a.ab, s(b, "ab"));
            close(a.actual, f(b, "actual"), &format!("{} actual", a.ab));
            close(a.inflight, f(b, "inflight"), &format!("{} inflight", a.ab));
            close(a.dispatch, f(b, "dispatch"), &format!("{} dispatch", a.ab));
        }
        close(crosswind_limit_kt(f(i, "mu"), i["autoland"].as_bool().unwrap()), f(row, "xwind"), "xwind");
    }
}

#[test]
fn calibration_text_cases() {
    assert!(calibration_text(&NO_CALIBRATION).starts_with("NO X-PLANE CALIBRATION YET"));
    let c = PerfCalibration::clamped(Some(1.5), Some(0.5), None, 3, 2);
    assert_eq!((c.thrust, c.roll, c.brake), (1.3, 0.7, 1.0));
    assert_eq!(calibration_text(&c), "X-PLANE CALIBRATION FROM 3 T/O AND 2 LDG: THRUST 1.30, ROLL 0.70, BRAKING 1.00");
}

#[test]
fn inflight_matches_oracle() {
    use oit_pages::inflight_model as m;
    for row in fixture()["inflight"].as_array().unwrap() {
        let (w, fl, mach, isa, eng, ff) = (f(row, "w"), f(row, "fl"), f(row, "mach"), f(row, "isa"), f(row, "eng"), f(row, "ff"));
        let tag = format!("{w} FL{fl} M{mach} isa{isa} eng{eng}");
        let c = m::cruise(w, fl, mach, isa, ff, eng);
        let o = &row["cruise"];
        close(c.tas_kt, f(o, "tasKt"), &format!("{tag} tas"));
        close(c.cl, f(o, "cl"), &format!("{tag} cl"));
        close(c.drag_n, f(o, "dragN"), &format!("{tag} drag"));
        close(c.fuel_flow_kg_h, f(o, "fuelFlowKgH"), &format!("{tag} ff"));
        close(c.specific_range_nm_per_t, f(o, "specificRangeNmPerT"), &format!("{tag} sr"));
        close(c.thrust_per_engine_n, f(o, "thrustPerEngineN"), &format!("{tag} thrust"));
        close(m::rec_max_fl(w, mach, isa), f(row, "recMax"), &format!("{tag} recMax"));
        close(m::optimum_fl(w, mach, isa), f(row, "opt"), &format!("{tag} opt"));
        close(m::engine_out_ceiling_fl(w, eng, isa), f(row, "eoCeil"), &format!("{tag} eoCeil"));
        let h = m::holding(w, fl, isa, eng);
        close(h.kcas, f(&row["hold"], "kcas"), &format!("{tag} hold kcas"));
        close(h.fuel_flow_kg_h, f(&row["hold"], "fuelFlowKgH"), &format!("{tag} hold ff"));
        close(m::green_dot_kcas(w, fl), f(row, "gd"), &format!("{tag} gd"));
        let d = m::descent(fl, 600.0, w, 15.0);
        close(d.distance_nm, f(&row["des"], "distanceNm"), &format!("{tag} des dist"));
        close(d.time_min, f(&row["des"], "timeMin"), &format!("{tag} des time"));
        close(d.fuel_kg, f(&row["des"], "fuelKg"), &format!("{tag} des fuel"));
        let p = m::plan(w, 90000.0, 1800.0, fl, mach, isa, -20.0, ff);
        close(p.trip_fuel_kg, f(&row["plan"], "tripFuelKg"), &format!("{tag} plan fuel"));
        close(p.time_min, f(&row["plan"], "timeMin"), &format!("{tag} plan time"));
        close(p.efob_kg, f(&row["plan"], "efobKg"), &format!("{tag} plan efob"));
    }
}
