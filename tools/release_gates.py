"""python -I tools/release_gates.py <staged aircraft folder> [--native-only]"""
import argparse
import csv
import fnmatch
import importlib.util
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LIBRARY = Path(os.environ.get("XPJS_LIBRARY", "E:/fbw-int/clean-avionics"))
FORK = Path(os.environ.get("XPJS_FORK", "E:/fbw-int/fbw-own-api"))
MIRROR = Path(os.environ.get("SIMCHECK_MIRROR", "E:/fbw-build/simcheck/msfs-avionics-mirror/src/sdk"))
SIMCHECK = Path(os.environ.get("SIMCHECK_TOOL", "E:/fbw-tools/session-tools/simcheck_tool"))
FS_BASE_UI = {name: Path(os.environ[var]) for name, var in (("fs2020", "FS_BASE_UI"), ("fs2024", "FS_BASE_UI_2024")) if os.environ.get(var)}
API_MAP = Path(os.environ.get("XPJS_API_MAP", "E:/fbw-int/xplane-js-private/api-map.json"))
JS_DISPLAYS = ("html_ui/", "plugins/fbw_a380_systems/XPHFBW/js/", "plugins/fbw_a380_systems/XPHFBW/ui/")
MS_NOTICE = re.compile(rb"copyright\s*(?:\(c\)|\xc2\xa9|\xa9)?\s*(?:\d{4}(?:-\d{4})?\s*)?microsoft", re.I)
MS_NOTICE_WIDE = re.compile(b"(?:c\x00o\x00p\x00y\x00r\x00i\x00g\x00h\x00t\x00|\xa9\x00).{0,40}?m\x00i\x00c\x00r\x00o\x00s\x00o\x00f\x00t\x00", re.I | re.S)


def package_release():
    spec = importlib.util.spec_from_file_location("package_release", ROOT / "tools" / "package_release.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def glob_re(pattern):
    out = ""
    i = 0
    while i < len(pattern):
        if pattern.startswith("**/", i):
            out += "(?:.*/)?"
            i += 3
        elif pattern.startswith("**", i):
            out += ".*"
            i += 2
        elif pattern[i] == "*":
            out += "[^/]*"
            i += 1
        else:
            out += re.escape(pattern[i])
            i += 1
    return re.compile("^" + out + "$")


def run(cmd, log):
    p = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace")
    log.write(f"$ {' '.join(map(str, cmd))}\n{p.stdout}{p.stderr}\n")
    return p.returncode == 0, (p.stdout.strip().splitlines() or [""])[-1]


def scrub(gates, roots):
    """Reports name files relative to the repos they came from, never by local path."""
    pairs = []
    for path, name in roots.items():
        p = Path(path).resolve()
        for form in (p.as_posix(), str(p), str(p).replace("\\", "\\\\")):
            pairs.append((form, name))
    pairs.sort(key=lambda x: -len(x[0]))
    for f in gates.iterdir():
        if f.name == "gates.log" or f.suffix not in (".json", ".txt", ".csv"):
            continue
        text = f.read_text(encoding="utf-8")
        for form, name in pairs:
            text = re.sub(re.escape(form), lambda _m, n=name: n, text, flags=re.I)
        f.write_text(text, encoding="utf-8")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("aircraft", type=Path)
    ap.add_argument("--native-only", action="store_true")
    ap.add_argument("--out", type=Path, required=True, help="folder for the reports and PROVENANCE.csv (a staged release, never an X-Plane install)")
    args = ap.parse_args()
    aircraft = args.aircraft
    out = args.out.resolve()
    if "x-plane 12/aircraft" in out.as_posix().lower():
        sys.exit("release_gates: refusing to write into an X-Plane aircraft folder")
    gates = out / "gates"
    gates.mkdir(parents=True, exist_ok=True)
    results = []
    with open(gates / "gates.log", "w", encoding="utf-8") as log:
        fbw_roots = [str(FORK / "fbw-a380x/src"), str(FORK / "fbw-common/src")]
        for half in ("lib", "data"):
            ok, last = run([sys.executable, str(SIMCHECK / "gate.py"), str(LIBRARY / half / "src"), str(MIRROR), str(gates / f"a-similarity-{half}.json"), *fbw_roots], log)
            results.append((f"(a) similarity, {half}", ok, last))
        if not FS_BASE_UI:
            results.append(("(b) page-script similarity", False, "FS_BASE_UI (the simulator's page scripts folder) is not set"))
        else:
            pr = package_release()
            served = [LIBRARY / "data/src"]
            for top in ("html_ui", "plugins/fbw_a380_systems/XPHFBW"):
                for p in sorted((aircraft / top).rglob("*")):
                    if p.is_file() and p.suffix in (".js", ".mjs") and not pr.excluded(p.relative_to(aircraft).as_posix(), args.native_only):
                        served.append(p)
            listed = gates / "b-candidates.txt"
            listed.write_text("\n".join(map(str, served)) + "\n", encoding="utf-8")
            cmd = [sys.executable, str(LIBRARY / "tools/gate-fsbase.py"), str(gates / "b-similarity-page-scripts.json")]
            for name, path in FS_BASE_UI.items():
                cmd += ["--ref", f"{name}={path}"]
            ok, last = run(cmd + [f"@{listed}"], log)
            listed.unlink()
            results.append(("(b) page-script similarity", ok, last))
        cmd = ["node", str(LIBRARY / "tools/gate-identifiers.cjs"), str(aircraft / "html_ui"), str(gates / "c-identifiers.json"), "--map", str(API_MAP), "--fbw", str(FORK)]
        for script in sorted((aircraft / "plugins/fbw_a380_systems/XPHFBW/js").glob("**/*.js")):
            cmd += ["--extra", str(script)]
        ok, last = run(cmd, log)
        results.append(("(c) identifiers", ok, last))

        pr = package_release()
        rules = json.loads((ROOT / "distribution/provenance-rules.json").read_text(encoding="utf-8"))
        forbidden = [(glob_re(r["glob"]), r["why"]) for r in rules["forbidden"]]
        ordered = [(glob_re(r["glob"]), r) for r in rules["rules"]]
        rows, unknown, bad = [], [], []
        for dirpath, _, files in os.walk(aircraft):
            for name in sorted(files):
                rel = (Path(dirpath) / name).relative_to(aircraft).as_posix()
                if pr.excluded(rel, args.native_only):
                    continue
                hit = next((why for rx, why in forbidden if rx.match(rel)), None)
                if hit:
                    bad.append((rel, hit))
                    continue
                rule = next((r for rx, r in ordered if rx.match(rel)), None)
                if rule is None:
                    unknown.append(rel)
                    rows.append((rel, "UNKNOWN", "UNKNOWN"))
                else:
                    rows.append((rel, rule["origin"], rule["licence"]))
        for target in (out / "PROVENANCE.csv", gates / "PROVENANCE.csv"):
            with open(target, "w", encoding="utf-8", newline="") as f:
                w = csv.writer(f)
                w.writerow(["file", "origin", "licence"])
                w.writerows(sorted(rows))
        (gates / "d-provenance.json").write_text(json.dumps({"files": len(rows), "unknown": unknown, "forbidden": bad}, indent=1), encoding="utf-8")
        results.append(("(d) provenance", not unknown and not bad, f"{len(rows)} files, {len(unknown)} unknown, {len(bad)} forbidden"))

        maps, refs = [], []
        for dirpath, _, files in os.walk(aircraft):
            for name in files:
                rel = (Path(dirpath) / name).relative_to(aircraft).as_posix()
                if name.endswith(".map"):
                    maps.append(rel)
                elif name.endswith((".js", ".css", ".mjs")) and not pr.excluded(rel, args.native_only):
                    if "sourceMappingURL=" in (Path(dirpath) / name).read_text(encoding="utf-8", errors="replace"):
                        refs.append(rel)
        (gates / "e-source-maps.json").write_text(json.dumps({"map_files": maps, "files_referencing_maps": refs}, indent=1), encoding="utf-8")
        results.append(("(e) no source maps", not maps and not refs, f"{len(maps)} .map files, {len(refs)} references"))

        allowed = {a["path"]: a for a in json.loads((ROOT / "distribution/microsoft-notices-allowed.json").read_text(encoding="utf-8"))["allowed"]}
        never = [p for p in allowed if p.startswith(JS_DISPLAYS)]
        found, over = [], []
        for dirpath, _, files in os.walk(aircraft):
            for name in sorted(files):
                rel = (Path(dirpath) / name).relative_to(aircraft).as_posix()
                if pr.excluded(rel, args.native_only):
                    continue
                data = (Path(dirpath) / name).read_bytes()
                hits = len(MS_NOTICE.findall(data)) + len(MS_NOTICE_WIDE.findall(data))
                if hits:
                    found.append({"file": rel, "notices": hits, "allowed": allowed.get(rel, {}).get("notices", 0)})
                    if hits > allowed.get(rel, {}).get("notices", 0):
                        over.append(rel)
        (gates / "f-microsoft-notices.json").write_text(json.dumps({"found": found, "not_allowed": over, "allow_list_covers_js_displays": never}, indent=1), encoding="utf-8")
        listed = sum(1 for f in found if f["file"] not in over)
        results.append(("(f) no Microsoft copyright notices", not over and not never, f"{len(over)} files not allowed, {listed} allow-listed (distribution/microsoft-notices-allowed.json)" + (f"; the allow-list names {len(never)} JS display files, which it never may" if never else "")))

    scrub(gates, {LIBRARY: "clean-avionics", FORK: "fbw-own-api", aircraft.resolve(): "aircraft", ROOT: "plugin"})
    lines = [f"{'PASS' if ok else 'FAIL'}  {name}: {detail}" for name, ok, detail in results]
    (gates / "GATES.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print("\n".join(lines))
    sys.exit(0 if all(ok for _, ok, _ in results) else 1)


if __name__ == "__main__":
    main()
