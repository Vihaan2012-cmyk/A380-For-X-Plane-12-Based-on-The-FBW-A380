import shutil
import sys
from pathlib import Path

FORK = Path("E:/fbw-int/fbw-aircraft-ts")
HTML_UI = FORK / "fbw-a380x/out/flybywire-aircraft-a380-842/html_ui"
PANEL = FORK / "fbw-a380x/src/base/flybywire-aircraft-a380-842/SimObjects/AirPlanes/FlyByWire_A380X/attachments/flybywire/Part_Interior_Cockpit/panel"
DEFAULT_DEST = Path("E:/fbw-build/fms-oracle-pkg")


def mfd_only(cfg: str) -> str:
    out, keep = [], True
    for line in cfg.splitlines():
        s = line.strip()
        if s.startswith("[VCockpit"):
            keep = s == "[VCockpit01]"
        if keep:
            out.append(line)
    return "\n".join(out) + "\n"


def main(dest: Path, replace: bool):
    if dest.exists():
        if not replace:
            sys.exit(f"{dest} exists; pass --replace to rebuild it")
        shutil.rmtree(dest)
    shutil.copytree(HTML_UI, dest / "html_ui")
    (dest / "panel.cfg").write_text(mfd_only((PANEL / "panel.cfg").read_text(encoding="utf-8")), encoding="utf-8")
    shutil.copyfile(PANEL / "panel.xml", dest / "panel.xml")
    print(f"{dest}: html_ui + MFD-only panel.cfg")


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    main(Path(args[0]) if args else DEFAULT_DEST, "--replace" in sys.argv)
