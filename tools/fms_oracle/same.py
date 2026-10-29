import sys
from pathlib import Path

a, b = Path(sys.argv[1]), Path(sys.argv[2])
names = sorted({p.name for p in a.glob("*.json")} | {p.name for p in b.glob("*.json")})
bad = [n for n in names if not (a / n).exists() or not (b / n).exists() or (a / n).read_bytes() != (b / n).read_bytes()]
for n in bad:
    print("differs:", n)
print(f"{len(names) - len(bad)}/{len(names)} identical")
sys.exit(1 if bad else 0)
