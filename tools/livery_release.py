"""Package the liveries as separate downloads for the installer's Liveries page.

    python tools/livery_release.py <aircraft liveries folder> <out dir>

Every livery folder becomes livery-<id>-2k.zip / livery-<id>-4k.zip (the folder named " 4K" is the
4K edition) holding the folder itself, plus a livery-<id>.jpg preview from its X-Plane icon and one
liveries.json describing them all (airline, registration, country, engines, model, creator, files).
"""

import hashlib
import json
import re
import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import lzma_zip  # noqa: E402

CREATOR = "D_an_"
PREVIEW_WIDTH = 960
AIRLINES = [
    ("Air France", "France", "AF", "A380-861", "GP7270"),
    ("ANA", "Japan", "NH", "A380-841", "Trent 970"),
    ("Asiana Airlines", "South Korea", "OZ", "A380-841", "Trent 970"),
    ("British Airways", "United Kingdom", "BA", "A380-841", "Trent 970"),
    ("China Southern", "China", "CZ", "A380-841", "Trent 970"),
    ("Emirates", "United Arab Emirates", "EK", "A380-861", "GP7270"),
    ("Etihad", "United Arab Emirates", "EY", "A380-861", "GP7270"),
    ("Global Airlines", "United Kingdom", "", "A380-841", "Trent 970"),
    ("Korean Air", "South Korea", "KE", "A380-861", "GP7270"),
    ("Lufthansa", "Germany", "LH", "A380-841", "Trent 970"),
    ("Qantas", "Australia", "QF", "A380-842", "Trent 972"),
    ("Qatar Airways", "Qatar", "QR", "A380-861", "GP7270"),
    ("Singapore Airlines", "Singapore", "SQ", "A380-841", "Trent 970"),
    ("Thai Airways", "Thailand", "TG", "A380-841", "Trent 970"),
]
REGISTRATION = re.compile(r"^(?:[A-Z0-9]{1,2}-[A-Z0-9]{2,5}|JA\d{3}[A-Z]|N\d{1,5}[A-Z]{0,2})$")


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def slug(text: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")


def describe(base: str, folder: Path) -> dict:
    airline = next((a for a in AIRLINES if base == a[0] or base.startswith(a[0] + " ")), None)
    if airline is None:
        raise SystemExit(f"{base}: no airline entry; add it to AIRLINES")
    rest = base[len(airline[0]):].strip()
    registration = rest if REGISTRATION.match(rest) else ""
    validation = folder / "validation.json"
    if not registration and validation.is_file():
        registration = json.loads(validation.read_text(encoding="utf-8")).get("registration", "")
    return {
        "id": slug(base),
        "airline": airline[0],
        "name": rest if rest and rest != registration else registration or airline[0],
        "registration": registration,
        "country": airline[1],
        "iata": airline[2],
        "model": airline[3],
        "engines": airline[4],
        "creator": CREATOR,
    }


def zip_folder(folder: Path, out: Path) -> None:
    files = sorted(p for p in folder.rglob("*") if p.is_file())
    lzma_zip.write_zip(out, [(p, f"{folder.name}/{p.relative_to(folder).as_posix()}") for p in files])


def make(liveries: Path, out: Path) -> list[dict]:
    out.mkdir(parents=True, exist_ok=True)
    folders = sorted(p for p in liveries.iterdir() if p.is_dir())
    bases = sorted({re.sub(r" 4K$", "", p.name) for p in folders})
    index = []
    for base in bases:
        editions = {"2K": liveries / base, "4K": liveries / f"{base} 4K"}
        editions = {k: v for k, v in editions.items() if v.is_dir()}
        entry = describe(base, next(iter(editions.values())))
        icon = next((f / "A380_icon11.png" for f in editions.values() if (f / "A380_icon11.png").is_file()), None)
        if icon is not None:
            im = Image.open(icon).convert("RGB")
            im = im.resize((PREVIEW_WIDTH, round(im.height * PREVIEW_WIDTH / im.width)), Image.LANCZOS)
            preview = out / f"livery-{entry['id']}.jpg"
            im.save(preview, quality=86)
            entry["preview"] = preview.name
        entry["editions"] = {}
        for label, folder in editions.items():
            target = out / f"livery-{entry['id']}-{label.lower()}.zip"
            zip_folder(folder, target)
            entry["editions"][label] = {"file": target.name, "folder": folder.name, "size": target.stat().st_size, "sha256": sha256(target)}
            print(f"  {target.name} {target.stat().st_size / 1e6:.1f} MB")
        index.append(entry)
    (out / "liveries.json").write_text(json.dumps({"liveries": index}, indent=1), encoding="utf-8")
    return index


if __name__ == "__main__":
    print(len(make(Path(sys.argv[1]), Path(sys.argv[2]))), "liveries")
