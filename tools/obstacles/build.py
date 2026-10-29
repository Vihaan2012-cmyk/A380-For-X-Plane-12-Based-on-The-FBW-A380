"""usage: build.py <out dir> [--faa] [--osm NAME=S,W,N,E ...] [--airports APT_DAT] [--offline]

Downloads and builds the obstacle database the TAWS obstacle check reads
(crates/gpws/src/obstacles.rs):
  --faa       FAA Digital Obstacle File, DAILY_DOF_DAT.ZIP (US, public domain)
  --osm       OpenStreetMap towers, masts, chimneys and power towers in a box,
              through the Overpass API (ODbL; credit "OpenStreetMap
              contributors")
  --airports  the same OSM obstacles within 30 NM of every airport in an
              X-Plane apt.dat with a hard runway of at least 3000 x 45 m,
              fetched in rate-limited batches; one cache file per airport,
              so an interrupted run resumes where it stopped
Raw downloads are kept in <out>/raw so a rebuild with --offline needs no
network; run again without --offline to update. Writes <out>/obstacles.bin
and <out>/manifest.json."""

import argparse
import datetime as dt
import json
import math
import re
import struct
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import zipfile
from pathlib import Path

FAA_URL = "https://aeronav.faa.gov/Obst_Data/DAILY_DOF_DAT.ZIP"
OVERPASS_URLS = ["https://overpass-api.de/api/interpreter", "https://overpass.kumi.systems/api/interpreter", "https://overpass.private.coffee/api/interpreter"]
USER_AGENT = "A380 for X-Plane 12 obstacle builder"
MIN_OSM_HEIGHT_M = 60.0
DEDUPE_M = 150.0
FT = 0.3048
MAGIC = b"OBS1"
VERSION = 1
UNKNOWN_AMSL = -32768
SOURCE_FAA, SOURCE_OSM = 0, 1

FAA_LINE = re.compile(r"(\d{2}) (\d{2}) (\d{2}\.\d{2})([NS]) (\d{3}) (\d{2}) (\d{2}\.\d{2})([EW]) (.{18}) (.) (\d{5}) (\d{5})")
FAA_ACTION = re.compile(r"\s([ACD])\s+\d{7}\s*$")
OSM_MAN_MADE = "mast|tower|chimney|communications_tower"
OSM_HEIGHT_60M_UP = "^([6-9][0-9]|[1-9][0-9]{2,})"
AIRPORT_MIN_LENGTH_M = 3000.0
AIRPORT_MIN_WIDTH_M = 45.0
AIRPORT_RADIUS_NM = 30.0
HARD_SURFACES = {1, 2, *range(20, 39), *range(50, 58)}
AIRPORT_BATCH = 6
OVERPASS_PAUSE_S = 8.0
OVERPASS_BACKOFF_S = 60.0
overpass_mirror = 0


def fetch(url: str, dest: Path, data: bytes | None = None) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    req = urllib.request.Request(url, data=data, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(req, timeout=300) as r:
        tmp = dest.with_suffix(dest.suffix + ".part")
        tmp.write_bytes(r.read())
    tmp.replace(dest)


def dms(d: str, m: str, s: str, hemi: str) -> float:
    v = int(d) + int(m) / 60 + float(s) / 3600
    return -v if hemi in "SW" else v


def parse_faa(zip_path: Path) -> tuple[list[dict], str]:
    with zipfile.ZipFile(zip_path) as z:
        name = next(n for n in z.namelist() if n.upper().endswith(".DAT"))
        text = z.read(name).decode("latin-1")
    currency = ""
    out = []
    for line in text.splitlines():
        if "CURRENCY DATE" in line:
            currency = line.split("=", 1)[1].strip()
            continue
        m = FAA_LINE.search(line)
        if not m:
            continue
        action = FAA_ACTION.search(line)
        if action and action.group(1) == "D":
            continue
        agl, amsl = int(m.group(11)), int(m.group(12))
        out.append({
            "lat": dms(*m.group(1, 2, 3, 4)),
            "lon": dms(*m.group(5, 6, 7, 8)),
            "agl_ft": agl,
            "amsl_ft": amsl,
            "source": SOURCE_FAA,
        })
    return out, currency


def overpass_query(s: float, w: float, n: float, e: float) -> str:
    return overpass_areas([f"{s},{w},{n},{e}"])


def overpass_areas(areas: list[str]) -> str:
    parts = []
    for a in areas:
        parts.append(f'nwr["man_made"~"^({OSM_MAN_MADE})$"]["height"~"{OSM_HEIGHT_60M_UP}"]({a});')
        parts.append(f'nwr["power"="tower"]["height"~"{OSM_HEIGHT_60M_UP}"]({a});')
    return "[out:json][timeout:300];(" + "".join(parts) + ");out center tags;"


def overpass(query: str, dest: Path) -> None:
    global overpass_mirror
    body = urllib.parse.urlencode({"data": query}).encode()
    for attempt in range(3 * len(OVERPASS_URLS)):
        url = OVERPASS_URLS[(overpass_mirror + attempt) % len(OVERPASS_URLS)]
        try:
            fetch(url, dest, body)
            overpass_mirror = OVERPASS_URLS.index(url)
            return
        except OSError as err:
            print(f"overpass {url}: {err}", file=sys.stderr)
            busy = isinstance(err, urllib.error.HTTPError) and err.code in (429, 502, 503, 504)
            time.sleep(OVERPASS_BACKOFF_S if busy else OVERPASS_PAUSE_S)
    raise OSError("every Overpass mirror failed")


def metres(lat1: float, lon1: float, lat2: float, lon2: float) -> float:
    p1, p2 = math.radians(lat1), math.radians(lat2)
    a = math.sin((p2 - p1) / 2) ** 2 + math.cos(p1) * math.cos(p2) * math.sin(math.radians(lon2 - lon1) / 2) ** 2
    return 2 * 6_371_000 * math.asin(math.sqrt(min(1.0, a)))


def a380_airports(apt_dat: Path) -> list[dict]:
    out = []
    cur = None

    def close():
        if cur and cur["runways"]:
            lats = [p for r in cur["runways"] for p in (r[0], r[2])]
            lon0 = cur["runways"][0][1]
            lons = [lon0 + ((p - lon0 + 180) % 360 - 180) for r in cur["runways"] for p in (r[1], r[3])]
            out.append({
                "id": cur["id"],
                "name": cur["name"],
                "lat": round(sum(lats) / len(lats), 6),
                "lon": round((sum(lons) / len(lons) + 180) % 360 - 180, 6),
                "longest_m": round(max(r[4] for r in cur["runways"])),
            })

    with open(apt_dat, encoding="utf-8", errors="replace") as f:
        for line in f:
            t = line.split()
            if not t:
                continue
            code = t[0]
            if code in ("1", "16", "17"):
                close()
                cur = {"id": t[4], "name": " ".join(t[5:]), "runways": []} if code == "1" and len(t) > 4 else None
            elif code == "1302" and cur and len(t) > 2 and t[1] == "icao_code":
                cur["id"] = t[2]
            elif code == "100" and cur and len(t) > 19:
                width, surface = float(t[1]), int(t[2])
                lat1, lon1, lat2, lon2 = float(t[9]), float(t[10]), float(t[18]), float(t[19])
                length = metres(lat1, lon1, lat2, lon2)
                if surface in HARD_SURFACES and width >= AIRPORT_MIN_WIDTH_M and length >= AIRPORT_MIN_LENGTH_M:
                    cur["runways"].append((lat1, lon1, lat2, lon2, length))
            elif code == "99":
                break
    close()
    seen = {}
    for a in out:
        seen.setdefault(a["id"], a)
    return sorted(seen.values(), key=lambda a: a["id"])


def element_pos(el: dict) -> tuple[float | None, float | None]:
    return el.get("lat", el.get("center", {}).get("lat")), el.get("lon", el.get("center", {}).get("lon"))


def airport_box(a: dict, radius_m: float) -> tuple[float, float, float, float]:
    dlat = radius_m / 111_320
    dlon = min(180.0, dlat / max(0.05, math.cos(math.radians(a["lat"]))))
    wrap = lambda x: (x + 180) % 360 - 180
    return max(-90.0, a["lat"] - dlat), wrap(a["lon"] - dlon), min(90.0, a["lat"] + dlat), wrap(a["lon"] + dlon)


def fetch_airports(airports: list[dict], cache: Path, offline: bool) -> list[dict]:
    radius = AIRPORT_RADIUS_NM * 1852
    todo = [] if offline else [a for a in airports if not (cache / f"{a['id']}.json").exists()]
    if todo:
        print(f"overpass: {len(todo)} of {len(airports)} airports to fetch in batches of {AIRPORT_BATCH}", file=sys.stderr, flush=True)
    for i in range(0, len(todo), AIRPORT_BATCH):
        batch = todo[i : i + AIRPORT_BATCH]
        tmp = cache / "batch.part.json"
        overpass(overpass_areas([",".join(f"{v:.4f}" for v in airport_box(a, radius)) for a in batch]), tmp)
        elements = json.loads(tmp.read_text(encoding="utf-8")).get("elements", [])
        for a in batch:
            near = [el for el in elements if None not in element_pos(el) and metres(*element_pos(el), a["lat"], a["lon"]) <= radius + 1]
            (cache / f"{a['id']}.json").write_text(json.dumps({"airport": a, "elements": near}), encoding="utf-8")
        tmp.unlink()
        print(f"overpass: {min(i + AIRPORT_BATCH, len(todo))}/{len(todo)} airports, {len(elements)} elements in the last batch", file=sys.stderr, flush=True)
        time.sleep(OVERPASS_PAUSE_S)
    rows = []
    for a in airports:
        p = cache / f"{a['id']}.json"
        if p.exists():
            rows += parse_osm(p)
    return rows


def height_m(tag: str | None) -> float | None:
    if not tag:
        return None
    t = tag.strip().lower().replace(",", ".")
    m = re.match(r"^([0-9]+(?:\.[0-9]+)?)\s*(m|ft|')?", t)
    if not m:
        return None
    v = float(m.group(1))
    return v * FT if m.group(2) in ("ft", "'") else v


def parse_osm(path: Path) -> list[dict]:
    data = json.loads(path.read_text(encoding="utf-8"))
    out = []
    for el in data.get("elements", []):
        tags = el.get("tags", {})
        lat = el.get("lat", el.get("center", {}).get("lat"))
        lon = el.get("lon", el.get("center", {}).get("lon"))
        h = height_m(tags.get("height"))
        if lat is None or lon is None or h is None or h < MIN_OSM_HEIGHT_M:
            continue
        ele = height_m(tags.get("ele"))
        out.append({
            "lat": float(lat),
            "lon": float(lon),
            "agl_ft": round(h / FT),
            "amsl_ft": round((ele + h) / FT) if ele is not None else UNKNOWN_AMSL,
            "source": SOURCE_OSM,
        })
    return out


def dedupe(faa: list[dict], osm: list[dict]) -> list[dict]:
    cell = DEDUPE_M / 111_320
    grid = {}
    for o in faa:
        grid.setdefault((int(o["lat"] // cell), int(o["lon"] // cell)), []).append(o)
    kept = []
    for o in osm:
        ci, cj = int(o["lat"] // cell), int(o["lon"] // cell)
        near = False
        for di in (-1, 0, 1):
            for dj in (-1, 0, 1):
                for f in grid.get((ci + di, cj + dj), ()):
                    dn = (f["lat"] - o["lat"]) * 111_320
                    de = (f["lon"] - o["lon"]) * 111_320 * math.cos(math.radians(o["lat"]))
                    near |= dn * dn + de * de < DEDUPE_M * DEDUPE_M
        if not near:
            kept.append(o)
    return kept


def tile_of(lat: float, lon: float) -> tuple[int, int]:
    return min(89, math.floor(lat)), (math.floor(lon) + 180) % 360 - 180


def write_bin(path: Path, obstacles: list[dict]) -> dict[int, int]:
    tiles: dict[int, list[dict]] = {}
    for o in obstacles:
        ty, tx = tile_of(o["lat"], o["lon"])
        tiles.setdefault((ty + 90) * 360 + (tx + 180), []).append(o)
    keys = sorted(tiles)
    header = struct.pack("<4sHHII", MAGIC, VERSION, 0, len(keys), len(obstacles))
    index, records, first = [], [], 0
    for k in keys:
        ty, tx = k // 360 - 90, k % 360 - 180
        rows = sorted(tiles[k], key=lambda o: (o["lat"], o["lon"]))
        index.append(struct.pack("<III", k, first, len(rows)))
        for o in rows:
            fy = min(65535, max(0, round((o["lat"] - ty) * 65536)))
            fx = min(65535, max(0, round(((o["lon"] - tx) % 360) * 65536)))
            agl = min(65535, max(0, int(o["agl_ft"])))
            amsl = max(-32767, min(32767, int(o["amsl_ft"]))) if o["amsl_ft"] != UNKNOWN_AMSL else UNKNOWN_AMSL
            records.append(struct.pack("<HHHh", fy, fx, agl, amsl))
        first += len(rows)
    path.write_bytes(header + b"".join(index) + b"".join(records))
    return {k: len(tiles[k]) for k in keys}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("out", type=Path)
    ap.add_argument("--faa", action="store_true")
    ap.add_argument("--osm", action="append", default=[], metavar="NAME=S,W,N,E")
    ap.add_argument("--airports", type=Path, metavar="APT_DAT")
    ap.add_argument("--offline", action="store_true")
    args = ap.parse_args()
    raw = args.out / "raw"
    manifest = {"built": dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds"), "format": "OBS1", "sources": {}}

    faa = []
    if args.faa:
        z = raw / "faa" / "DAILY_DOF_DAT.ZIP"
        if not args.offline:
            fetch(FAA_URL, z)
        faa, currency = parse_faa(z)
        manifest["sources"]["faa_dof"] = {"url": FAA_URL, "currency": currency, "count": len(faa), "licence": "US Government work, public domain"}

    osm = []
    for spec in args.osm:
        name, box = spec.split("=", 1)
        s, w, n, e = (float(x) for x in box.split(","))
        p = raw / "osm" / f"{name}.json"
        if not args.offline:
            overpass(overpass_query(s, w, n, e), p)
        rows = parse_osm(p)
        osm += rows
        manifest["sources"][f"osm_{name}"] = {"bbox": [s, w, n, e], "count": len(rows), "licence": "ODbL 1.0, (c) OpenStreetMap contributors"}

    if args.airports:
        cache = raw / "osm_airports"
        listing = cache / "airports.json"
        if args.offline and listing.exists():
            airports = json.loads(listing.read_text(encoding="utf-8"))
        else:
            airports = a380_airports(args.airports)
            cache.mkdir(parents=True, exist_ok=True)
            listing.write_text(json.dumps(airports, indent=1) + "\n", encoding="utf-8")
        print(f"airports with a hard runway of at least {AIRPORT_MIN_LENGTH_M:.0f} x {AIRPORT_MIN_WIDTH_M:.0f} m: {len(airports)}", file=sys.stderr, flush=True)
        rows = fetch_airports(airports, cache, args.offline)
        osm += rows
        manifest["sources"]["osm_airports"] = {
            "airports": len(airports),
            "airports_fetched": sum(1 for a in airports if (cache / f"{a['id']}.json").exists()),
            "radius_nm": AIRPORT_RADIUS_NM,
            "runway_min_m": [AIRPORT_MIN_LENGTH_M, AIRPORT_MIN_WIDTH_M],
            "count": len(rows),
            "licence": "ODbL 1.0, (c) OpenStreetMap contributors",
        }

    osm = list({(round(o["lat"], 6), round(o["lon"], 6)): o for o in osm}.values())
    osm_kept = dedupe(faa, osm)
    all_obs = faa + osm_kept
    args.out.mkdir(parents=True, exist_ok=True)
    tiles = write_bin(args.out / "obstacles.bin", all_obs)
    manifest.update({
        "obstacles": len(all_obs),
        "osm_dropped_as_faa_duplicates": len(osm) - len(osm_kept),
        "osm_without_base_elevation": sum(1 for o in osm_kept if o["amsl_ft"] == UNKNOWN_AMSL),
        "tiles": len(tiles),
        "bytes": (args.out / "obstacles.bin").stat().st_size,
    })
    (args.out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({k: v for k, v in manifest.items() if k != "sources"}))
    for k, v in manifest["sources"].items():
        print(f"  {k}: {v['count']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
