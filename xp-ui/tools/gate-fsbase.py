"""python gate-fsbase.py <report.json> --ref <name>=<reference dir> [--ref ...] <candidate file or dir | @list file>...

Every candidate is checked against each reference separately; the report names references only by <name>.
"""
import json
import sys
from pathlib import Path

SIMCHECK = Path("E:/fbw-tools/session-tools/simcheck_tool")
sys.path.insert(0, str(SIMCHECK))
from sim import norm, tokens  # noqa: E402
from gate import EXACT_MAX_RUN, K, declaration_lines  # noqa: E402


def files(root, suffixes):
    root = Path(root)
    if root.is_file():
        yield root
        return
    for p in sorted(root.rglob("*")):
        if p.is_file() and p.suffix in suffixes and not p.name.endswith(".d.ts") and "node_modules" not in p.parts:
            yield p


def index(root, mode):
    seen = set()
    count = 0
    for p in files(root, (".js", ".mjs", ".ts")):
        toks = [norm(t, mode) for t in tokens(p.read_text(encoding="utf-8", errors="replace"))]
        for i in range(len(toks) - K + 1):
            seen.add(hash(tuple(toks[i:i + K])))
        count += 1
    return seen, count


def measure(toks, decl, seq, idx):
    covered = [False] * len(seq)
    for i in range(len(seq) - K + 1):
        if hash(tuple(seq[i:i + K])) in idx:
            for j in range(i, i + K):
                covered[j] = True
    longest = run = impl = hit = 0
    for i, t in enumerate(toks):
        inside = t[2] not in decl
        impl += inside
        hit += inside and covered[i]
        run = run + 1 if covered[i] and inside else 0
        longest = max(longest, run)
    return longest, round(100 * hit / max(1, impl), 2)


def main():
    args = sys.argv[1:]
    out = Path(args.pop(0))
    refs, candidates = [], []
    while args:
        a = args.pop(0)
        if a == "--ref":
            name, _, path = args.pop(0).partition("=")
            refs.append((name, Path(path)))
        elif a.startswith("@"):
            candidates += [Path(line) for line in Path(a[1:]).read_text(encoding="utf-8").splitlines() if line]
        else:
            candidates.append(Path(a))
    missing = [name for name, path in refs if not path.is_dir()]
    if not refs or missing:
        why = f"reference not found: {', '.join(missing)}" if missing else "no reference given"
        out.write_text(json.dumps({"gate": "fs-base-ui", "pass": False, "error": why}, indent=1), encoding="utf-8")
        print(f"FAIL fs-base-ui gate: {why}")
        sys.exit(1)
    sources = []
    for c in candidates:
        for p in files(c, (".ts", ".tsx", ".js", ".mjs")):
            src = p.read_text(encoding="utf-8", errors="replace")
            sources.append((p, tokens(src), declaration_lines(src)))
    rows = {str(p): {"file": str(p), "tokens": len(toks), "pass": True} for p, toks, _ in sources}
    counts = {}
    for name, path in refs:
        for mode in ("raw", "norm"):
            idx, counts[name] = index(path, mode)
            for p, toks, decl in sources:
                longest, pct = measure(toks, decl, [norm(t, mode) for t in toks], idx)
                row = rows[str(p)]
                row[f"{name}_{mode}_longest"] = longest
                row[f"{name}_{mode}_pct"] = pct
                if mode == "raw" and longest > EXACT_MAX_RUN:
                    row["pass"] = False
            del idx
    rows = list(rows.values())
    ok = bool(rows) and all(r["pass"] for r in rows)
    out.write_text(json.dumps({"gate": "fs-base-ui", "references": counts, "max_run": EXACT_MAX_RUN, "pass": ok, "files": rows}, indent=1), encoding="utf-8")
    for r in rows:
        cols = "  ".join(f"{name} exact {r[name + '_raw_pct']:6}% run {r[name + '_raw_longest']:4} norm {r[name + '_norm_pct']:6}% run {r[name + '_norm_longest']:4}" for name, _ in refs)
        print(f"{'PASS' if r['pass'] else 'FAIL'} {cols} {r['file']}")
    against = ", ".join(f"{name} ({n} files)" for name, n in counts.items())
    print(f"{sum(r['pass'] for r in rows)}/{len(rows)} files pass against {against} (longest verbatim run <= {EXACT_MAX_RUN} tokens)")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
