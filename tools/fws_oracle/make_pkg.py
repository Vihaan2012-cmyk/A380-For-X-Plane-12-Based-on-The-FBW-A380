import shutil
import sys
from pathlib import Path

FORK = Path("E:/fbw-int/fbw-aircraft-ts")
OUT = FORK / "fbw-a380x/out/flybywire-aircraft-a380-842/html_ui"
SRC = Path("E:/fbw-int/fbw-aircraft/fbw-a380x/src/base/flybywire-aircraft-a380-842")
PANEL = FORK / "fbw-a380x/src/base/flybywire-aircraft-a380-842/SimObjects/AirPlanes/FlyByWire_A380X/attachments/flybywire/Part_Interior_Cockpit/panel"
DEFAULT_DEST = Path("E:/fbw-build/fws-oracle-pkg")


def systems_host_only(cfg: str) -> str:
    out, keep = [], True
    for line in cfg.splitlines():
        s = line.strip()
        if s.startswith("[VCockpit"):
            keep = s == "[VCockpit22]"
        if keep:
            out.append(line)
    return "\n".join(out) + "\n"


def main(dest: Path, replace: bool):
    if dest.exists():
        if not replace:
            sys.exit(f"{dest} exists; pass --replace to rebuild it")
        shutil.rmtree(dest)
    shutil.copytree(OUT, dest / "html_ui")
    page = SRC / "html_ui/Pages/VCockpit/Instruments/A380X/SystemsHost/systems-host.html"
    shutil.copyfile(page, dest / "html_ui/Pages/VCockpit/Instruments/A380X/SystemsHost/systems-host.html")
    (dest / "panel.cfg").write_text(systems_host_only((PANEL / "panel.cfg").read_text(encoding="utf-8")), encoding="utf-8")
    shutil.copyfile(PANEL / "panel.xml", dest / "panel.xml")
    print(f"{dest}: html_ui + systems-host.html + VCockpit22-only panel.cfg")


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    main(Path(args[0]) if args else DEFAULT_DEST, "--replace" in sys.argv)
