import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).parent
UI = HERE.parent.parent / "ui" / "index.html"


def bundle():
    data = {"sil": json.loads((HERE / "silhouettes.json").read_text()), "parts": json.loads((HERE / "parts.json").read_text()), "model": json.loads((HERE / "model.json").read_text()), "shots": json.loads((HERE / "ecam_shots.json").read_text()) if (HERE / "ecam_shots.json").exists() else {}}
    js = "const DX_DATA = " + json.dumps(data, separators=(",", ":")) + ";\n"
    for f in sorted(HERE.glob("*.js")):
        js += f.read_text(encoding="utf-8") + "\n"
    return js


def clashes(raw, js):
    decl = re.compile(r"^(?:const|let|class|function)\s+([A-Za-z_$][\w$]*)", re.M)
    mine = set(decl.findall(js))
    others = re.findall(r'<script(?![^>]*src)([^>]*)>(.*?)</script>', raw, re.S)
    found = set()
    for attr, body in others:
        if "efb-diagrams" not in attr:
            found |= mine & set(decl.findall(body))
    return sorted(found)


def main(target=UI):
    raw = target.read_bytes().decode("utf-8")
    clash = clashes(raw, bundle())
    if clash:
        sys.exit("name clash with the EFB page: " + ", ".join(clash))
    nl = "\r\n" if "\r\n" in raw else "\n"
    block = '<script id="efb-diagrams">' + nl + bundle().replace("\n", nl) + "</script>"
    if '<script id="efb-diagrams">' in raw:
        raw = re.sub(r'<script id="efb-diagrams">.*?</script>', lambda m: block, raw, flags=re.S)
    else:
        i = raw.rindex("</body>")
        raw = raw[:i] + block + nl + raw[i:]
    target.write_bytes(raw.encode("utf-8"))
    print("diagrams inlined into", target)


if __name__ == "__main__":
    main(Path(sys.argv[1]) if len(sys.argv) > 1 else UI)
