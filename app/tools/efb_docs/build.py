import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
FBW = HERE.parents[3] / "fbw-aircraft"
UI = HERE.parents[1] / "ui" / "index.html"
START, END = '<script id="efb-docs">', "</script><!--efb-docs-->"
CODE = re.compile(r"\x1b[^m]*m")
SYSTEM = re.compile(r"\x1b4m(.*?)\x1bm")
ENTRY = re.compile(r"^\s{4}(\d{9}): \{", re.M)
INHIB = re.compile(r"flightPhaseInhib: \[([0-9, ]*)\]")
LISTS = re.compile(r"(inopSysAllPhases|inopSysApprLdg|redundLoss|limitationsAllPhases|limitationsApprLdg|limitationsPfd|info): \([^)]*\) =>(.*?)(?=\n\s{6}[a-zA-Z]+[?]?:|\n\s{4}\})", re.S)
FAILURE = re.compile(r"\n\s{6}failure: (\d)")
SYSPAGE = re.compile(r"\n\s{6}sysPage: SdPages\.(\w+)")
ID9 = re.compile(r"'(\d{9})'")
FWS = "fbw-a380x/src/systems/systems-host/CpiomC/FlightWarningSystem/FwsAbnormalSensed.ts"


def extract():
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "docs.cjs"
        esbuild = FBW / "node_modules" / ".bin" / ("esbuild.cmd" if sys.platform == "win32" else "esbuild")
        subprocess.run([str(esbuild), str(HERE / "entry.ts"), "--bundle", "--platform=node", "--format=cjs",
                        "--log-level=warning", f"--outfile={out}"], check=True)
        return json.loads(subprocess.run(["node", str(out)], check=True, capture_output=True, text=True).stdout)


def line(it):
    row = [CODE.sub("", it.get("name", "")).strip(), CODE.sub("", it.get("labelNotCompleted", "") or "").strip()]
    flags = ""
    if it.get("condition"):
        flags += "c"
    if it.get("style") in ("Cyan",):
        flags += "b"
    if it.get("style") in ("SubHeadline", "CenteredSubHeadline", "Headline"):
        flags += "h"
    if it.get("style") in ("Amber",):
        flags += "a"
    if it.get("style") in ("Red", "LandAsap"):
        flags += "r"
    if it.get("style") in ("Green", "GreenTable", "LandAnsa"):
        flags += "g"
    return [row[0], row[1], int(it.get("level", 0) or 0), flags]


def procedure(pid, p):
    raw = p.get("title", "")
    level = "red" if "\x1b<2m" in raw else "amber" if "\x1b<4m" in raw else "white"
    m = SYSTEM.search(raw)
    system = m.group(1).strip() if m else ""
    title = re.sub(r"\s+", " ", CODE.sub("", raw)).strip()
    rec = CODE.sub("", p.get("recommendation", "") or "").strip()
    return {"id": pid, "t": title, "s": system, "l": level, "r": rec, "i": [line(it) for it in p.get("items", [])]}


def build():
    d = extract()
    normal = [{"id": k, "t": CODE.sub("", v["title"]).strip(), "i": [line(it) for it in v.get("items", [])],
               "d": int(k) in d["deferredNormal"]} for k, v in sorted(d["normal"].items(), key=lambda kv: int(kv[0]))]
    abnormal = {k: procedure(k, v) for k, v in d["abnormal"].items()}
    fws = (FBW / FWS).read_text(encoding="utf-8")
    marks = list(ENTRY.finditer(fws))
    texts = {**{k: CODE.sub("", v).strip() for k, v in d["inop"].items()}, **{k: CODE.sub("", v).strip() for k, v in d["limitations"].items()}, **{k: CODE.sub("", v).strip() for k, v in d["infos"].items()}}
    for n, m in enumerate(marks):
        pid = m.group(1)
        if pid not in abnormal:
            continue
        block = fws[m.end():marks[n + 1].start() if n + 1 < len(marks) else len(fws)]
        p = abnormal[pid]
        hit = INHIB.search(block)
        if hit:
            p["ph"] = [int(x) for x in hit.group(1).replace(" ", "").split(",") if x]
        hit = FAILURE.search(block)
        if hit:
            p["f"] = int(hit.group(1))
        hit = SYSPAGE.search(block)
        if hit and hit.group(1) != "None":
            p["sd"] = hit.group(1)
        for key, body in LISTS.findall(block):
            items = []
            for i in ID9.findall(body):
                if i in texts and texts[i] not in items:
                    items.append(texts[i])
            if items:
                p.setdefault("x", {})[key] = items
    non_sensed = [dict(abnormal[str(n["id"])], c=n["category"] or "") for n in d["nonSensed"] if str(n["id"]) in abnormal]
    ns_ids = {str(n["id"]) for n in d["nonSensed"]}
    sensed = sorted((p for k, p in abnormal.items() if k not in ns_ids), key=lambda p: (p["s"], p["t"]))
    deferred = [{"id": k, "t": CODE.sub("", v.get("title", "")).strip(), "i": [line(it) for it in v.get("items", [])]}
                for k, v in d["deferred"].items()]
    perf = {k: v for k, v in d["perf"].items() if isinstance(v, (int, float))}
    docs = {"normal": normal, "sensed": sensed, "nonSensed": non_sensed, "deferred": deferred,
            "limitations": sorted({CODE.sub("", v).strip() for v in d["limitations"].values()}),
            "inop": sorted({CODE.sub("", v).strip() for v in d["inop"].values()}), "perf": perf}
    blob = START + "window.__DOCS__=" + json.dumps(docs, separators=(",", ":")) + ";" + END
    html = UI.read_text(encoding="utf-8")
    if START in html:
        a = html.index(START)
        b = html.index(END, a) + len(END)
        html = html[:a] + blob + html[b:]
    else:
        anchor = "\n<script>\n"
        i = html.index(anchor, html.index("<body>"))
        html = html[:i] + "\n" + blob + html[i:]
    UI.write_text(html, encoding="utf-8")
    print(len(normal), "checklists,", len(sensed), "ECAM procedures,", len(non_sensed), "non-sensed,", len(blob) // 1024, "KB")


if __name__ == "__main__":
    build()
