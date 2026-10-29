import shutil
import sys
from pathlib import Path

FORK = Path("E:/fbw-int/fbw-aircraft-ts")
OUT = FORK / "fbw-a380x/out/flybywire-aircraft-a380-842/html_ui"
SRC = Path("E:/fbw-int/fbw-aircraft/fbw-a380x/src/base/flybywire-aircraft-a380-842")
PANEL = FORK / "fbw-a380x/src/base/flybywire-aircraft-a380-842/SimObjects/AirPlanes/FlyByWire_A380X/attachments/flybywire/Part_Interior_Cockpit/panel"
DEFAULT_DEST = Path("E:/fbw-build/rmp-oracle-pkg")
KEEP = {"[VCockpit16]", "[VCockpit17]", "[VCockpit18]", "[VCockpit22]", "[VCockpit23]"}


def rmp_views_only(cfg: str) -> str:
    out, keep = [], True
    for line in cfg.splitlines():
        s = line.strip()
        if s.startswith("[VCockpit"):
            keep = s in KEEP
        if keep:
            out.append(line)
    return "\n".join(out) + "\n"


def main(dest: Path, replace: bool):
    if dest.exists():
        if not replace:
            sys.exit(f"{dest} exists; pass --replace to rebuild it")
        shutil.rmtree(dest)
    shutil.copytree(OUT, dest / "html_ui")
    for rel in ("SystemsHost/systems-host.html", "ExtrasHost/extras-host.html"):
        page = SRC / "html_ui/Pages/VCockpit/Instruments/A380X" / rel
        if page.exists():
            shutil.copyfile(page, dest / "html_ui/Pages/VCockpit/Instruments/A380X" / rel)
    (dest / "panel.cfg").write_text(rmp_views_only((PANEL / "panel.cfg").read_text(encoding="utf-8")), encoding="utf-8")
    shutil.copyfile(PANEL / "panel.xml", dest / "panel.xml")
    print(f"{dest}: html_ui + RMP 1-3, systems-host and extras-host views panel.cfg")


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    main(Path(args[0]) if args else DEFAULT_DEST, "--replace" in sys.argv)
