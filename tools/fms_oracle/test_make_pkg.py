import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from make_pkg import mfd_only

CFG = "[Window Titles]\nWindow00=X\n\n[VCockpit01]\ntexture=SCREEN_DU_MFD\nhtmlgauge00=A380X/MFD/mfd.html?duID=2, 0,0,1646,1024\n\n[VCockpit02]\ntexture=SCREEN_DU_PFDL\nhtmlgauge00=A380X/PFD/pfd.html\n"

out = mfd_only(CFG)
assert "[Window Titles]" in out
assert "SCREEN_DU_MFD" in out
assert "SCREEN_DU_PFDL" not in out
assert "[VCockpit02]" not in out
print("ok")
