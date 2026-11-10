import math
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import acf11
import make_xp11
import obj11


class Acf(unittest.TestCase):
    def test_rib_arms_match_plane_maker_11(self):
        out = {"_wing/16/_els": "6", "_wing/16/_semilen_SEG": "22.729999542", "_wing/16/_is_right_mult": "-1.0",
               "_wing/16/_dihed_design": "7.0", "_wing/16/_sweep_design": "26.0",
               "_part/16/_part_x": "-2.0", "_part/16/_part_y": "1.929999948", "_part/16/_part_z": "99.0"}
        acf11.crib_arms(out)
        self.assertAlmostEqual(float(out["_wing/16/_crib_x_arm/1"]), -5.379550934, places=5)
        self.assertAlmostEqual(float(out["_wing/16/_crib_y_arm/1"]), 2.344956636, places=5)
        self.assertAlmostEqual(float(out["_wing/16/_crib_z_arm/6"]), 108.964179993, places=4)
        self.assertEqual(float(out["_wing/16/_crib_x_arm/7"]), 0.0)

    def test_hydraulic_bitmasks_become_xp11_system_numbers(self):
        self.assertEqual([acf11.hyd_0123(v) for v in ("0", "1", "2", "4", "3", "5", "7")], ["0", "1", "2", "2", "3", "3", "3"])

    def test_part_slots_and_attachments(self):
        p12 = {"_body/0/_part_x": "1.0", "_wing/2/_part_z": "3.0", "_wing/2/_Croot": "9.0", "_engn/1/_part_x": "-5.0", "_engn/9/_part_x": "7.0",
               "_obja/0/_v10_att_body": "-1", "_obja/0/_v10_att_wing": "2", "_obja/0/_v10_att_gear": "-1", "_obja/0/_obj_flags": "32781",
               "acf/_hyd_gear": "3", "acf/_m_max": "1000.0", "_gear/count": "0"}
        out = acf11.convert(p12, {})
        self.assertEqual(out["_part/56/_part_x"], "1.0")
        self.assertEqual(out["_part/10/_part_z"], "3.0")
        self.assertEqual(out["_wing/10/_Croot"], "9.0")
        self.assertEqual(out["_part/1/_part_x"], "-5.0")
        self.assertNotIn("_part/9/_part_x", out)
        self.assertEqual(out["_obja/0/_v10_att_part"], "10")
        self.assertEqual(out["_obja/0/_obj_flags"], "13")
        self.assertEqual(out["acf/_hyd_gear"], "1")

    def test_gear_shares_add_up_and_the_nose_carries_least(self):
        p12 = {"_gear/count": "3", "acf/_m_max": "1000.0", "acf/_cgZ": "-8.0"}
        for g, z in enumerate((-99.0, 5.0, 5.0)):
            p12.update({f"_gear/{g}/_gear_type": "5", f"_gear/{g}/_gear_z": str(z), f"_gear/{g}/_leg_len": "8.0"})
        out = {}
        acf11.gear_springs(p12, out)
        shares = [float(out[f"_gear/{g}/_gear_load_fraction"]) for g in range(3)]
        self.assertAlmostEqual(sum(shares), 1.0, places=6)
        self.assertLess(shares[0], shares[1])
        self.assertLess(float(out["_gear/1/_leg_len"]), 8.0)


class Obj(unittest.TestCase):
    def test_a_forward_light_needs_no_turn_and_a_left_one_turns_ninety(self):
        self.assertEqual(tuple(round(a, 6) for a in obj11._aim(0, 0, -1)), (0.0, 0.0))
        yaw, pitch = obj11._aim(-1, 0, 0)
        self.assertAlmostEqual(yaw, 90.0)
        yaw, pitch = obj11._aim(0, 1, 0)
        self.assertAlmostEqual(pitch, 90.0)

    def test_the_turn_points_minus_z_along_the_light(self):
        d = (-0.642788, -0.1, -0.99498744)
        yaw, pitch = map(math.radians, obj11._aim(*d))
        v = (-math.cos(pitch) * math.sin(yaw), math.sin(pitch), -math.cos(pitch) * math.cos(yaw))
        n = math.sqrt(sum(c * c for c in d))
        for a, b in zip(v, d):
            self.assertAlmostEqual(a, b / n, places=5)

    def test_xp12_lights_become_xp11_lights(self):
        nav = obj11.light("LIGHT_PARAM airplane_nav_bb -38.7 2.2 11.1 1 0 0 0 3000cd -1 0 0 0.0436")
        self.assertEqual(nav, ["LIGHT_NAMED airplane_nav_left -38.7 2.2 11.1"])
        land = obj11.light("LIGHT_PARAM airplane_landing_pm 1 2 3 0.7 0.6 0.5 2 1530000cd 0 -0.087 -0.996 0.976")
        self.assertEqual(land[0], "ANIM_begin")
        self.assertTrue(land[4].startswith("LIGHT_PARAM airplane_landing_sp 0 0 0 0.7 0.6 0.5 2 "))
        self.assertIsNone(obj11.light("LIGHT_PARAM airplane_landing_sp 0 0 0 1 1 1 0 10 1"))

    def test_material_maps_become_one_normal_map(self):
        text = "I\n800\nOBJ\nTEXTURE a.dds\nTEXTURE_MAP normal a_NML.png\nTEXTURE_MAP material_gloss a_MAT.png\nGLOBAL_specular 1.0\nPOINT_COUNTS 0 0 0 0\nATTR_cockpit_device X 0 0 0\nATTR_manip_device hand X X\n"
        out, changed = obj11.generic(text)
        self.assertTrue(changed)
        self.assertIn("TEXTURE_NORMAL a_NML.png", out)
        self.assertNotIn("TEXTURE_MAP", out)
        self.assertNotIn("ATTR_cockpit_device", out)
        self.assertIn("ATTR_manip_none", out)
        self.assertIn("GLOBAL_specular 0.5", out)

    def test_screen_glass_moves_onto_the_panel(self):
        click = "I\n800\nOBJ\nTEXTURE t.png\nPOINT_COUNTS 1 0 0 1\nVT 0 0 0 0 1 0 0 0\nIDX 0\nATTR_manip_device hand S S\nATTR_cockpit_device S 0 0 0\nTRIS 0 1\nATTR_manip_none\n"
        screens = ("I\n800\nOBJ\nTEXTURE s.png\nPOINT_COUNTS 3 0 0 3\nVT 0 0 0 0 1 0 0 0\nVT 1 0 0 0 1 0 1 0\nVT 1 1 0 0 1 0 1 1\nIDX 0\nIDX 1\nIDX 2\n"
                   "ATTR_LOD 0 1000\nATTR_cockpit_device S 0 0 0 1000\nTRIS 0 3\nATTR_no_cockpit\n")
        out = obj11.cockpit_with_screens(click, screens, {"S": {"x": 1024, "y": 2048, "w": 1024, "h": 512}}, (4096, 4096))
        self.assertIn("POINT_COUNTS 4 0 0 4", out)
        self.assertIn("VT 1 1 0 0 1 0 0.500000 0.625000", out)
        self.assertIn("TRIS 1 3", out)
        self.assertNotIn("TRIS 0 1", out)
        self.assertIn("ATTR_cockpit\n", out)
        self.assertNotIn("ATTR_cockpit_device", out)
        self.assertNotIn("ATTR_LOD", out)


class Dds(unittest.TestCase):
    def test_a_texture_over_4096_loses_its_top_mip(self):
        import struct
        import tempfile
        w, h, mips = 4096, 8192, 3
        sizes = [max(1, (w >> i) // 4) * max(1, (h >> i) // 4) * 16 for i in range(mips)]
        header = bytearray(128)
        header[:4] = b"DDS "
        header[12:20] = struct.pack("<II", h, w)
        header[28:32] = struct.pack("<I", mips)
        header[84:88] = b"DXT5"
        body = b"".join(bytes([i + 1]) * n for i, n in enumerate(sizes))
        with tempfile.TemporaryDirectory() as d:
            src, dst = Path(d) / "a.dds", Path(d) / "b.dds"
            src.write_bytes(bytes(header) + body)
            self.assertEqual(make_xp11.shrink_dds(src, dst), (2048, 4096))
            out = dst.read_bytes()
            self.assertEqual(struct.unpack("<III", out[12:24]), (4096, 2048, sizes[1]))
            self.assertEqual(struct.unpack("<I", out[28:32])[0], 2)
            self.assertEqual(out[128], 2)
            self.assertEqual(len(out), 128 + sizes[1] + sizes[2])

    def test_the_3d_panel_gets_one_black_square_per_screen(self):
        atlas = {"A": {"x": 0, "y": 0, "w": 10, "h": 20}}
        tail = ["PROPERTIES_END", "PANEL_2D_BEGIN", "PANEL_2D_END", "PANEL_3D_BEGIN", "PANEL_3D_END"]
        out = make_xp11.with_panel_3d(tail, make_xp11.panel_3d(atlas, {(10, 20): "fbw_xp11/black_10x20"}))
        self.assertEqual(out[:3], ["PROPERTIES_END", "PANEL_2D_BEGIN", "PANEL_2D_END"])
        self.assertIn("  gen_rotary A", out)
        self.assertIn("    IMAGE fbw_xp11/black_10x20", out)
        self.assertEqual(out[-1], "PANEL_3D_END")
        self.assertEqual(sum(1 for l in out if l == "PANEL_3D_BEGIN"), 1)


class NormalMaps(unittest.TestCase):
    def test_an_object_gets_one_packed_normal_map_read_with_normal_metalness(self):
        text = "\n".join(["I", "800", "OBJ", "TEXTURE a.dds", "TEXTURE_MAP normal a_NML.png", "TEXTURE_MAP material_gloss a_MAT.png",
                          "GLOBAL_specular 1.0", "POINT_COUNTS 0 0 0 0", ""])
        seen = []
        out, changed = obj11.generic(text, lambda n, m: seen.append((n, m)) or "a_xp11.png")
        self.assertEqual(seen, [("a_NML.png", "a_MAT.png")])
        self.assertIn("\n".join(["TEXTURE_NORMAL a_xp11.png", "NORMAL_METALNESS", ""]), out)
        self.assertNotIn("TEXTURE_MAP", out)
        self.assertIn("GLOBAL_specular 1.0", out)

    def test_the_packed_map_holds_normal_xy_metalness_and_gloss(self):
        import tempfile
        from PIL import Image
        import numpy as np
        with tempfile.TemporaryDirectory() as d:
            n, m, o = Path(d) / "n.png", Path(d) / "m.png", Path(d) / "o.png"
            Image.fromarray(np.full((4, 4, 3), [100, 150, 0], np.uint8)).save(n)
            Image.fromarray(np.full((2, 2, 3), [30, 200, 0], np.uint8)).save(m)
            make_xp11.pack_normal(n, m, o)
            px = np.asarray(Image.open(o))
            self.assertEqual(px.shape, (4, 4, 4))
            self.assertEqual(list(px[1, 1]), [100, 150, 30, 200])


class Atlas(unittest.TestCase):
    def test_the_cockpit_screens_fit_with_the_fcu_and_rmps_halved(self):
        sizes = make_xp11.screen_sizes(Path(__file__).resolve().parents[2] / "src" / "display" / "screens.rs")
        ids = ["SCREEN_EFB", "RTPI", "FCU", "SCREEN_DU_EWD", "SCREEN_DU_MFD", "SCREEN_DU_NDL", "SCREEN_DU_NDR", "SCREEN_DU_PFDL", "SCREEN_DU_PFDR",
               "SCREEN_DU_SD", "SCREEN_OIT_LEFT", "SCREEN_OIT_RIGHT", "SCREEN_DU_RMP_1", "SCREEN_DU_RMP_2", "SCREEN_DU_RMP_3", "SCREEN_ISIS_2", "SCREEN_ISIS_1", "Clock", "BAT"]
        atlas = make_xp11.pack({k: sizes[k] for k in ids})
        self.assertEqual(atlas["SCREEN_DU_PFDL"]["w"], 768)
        self.assertEqual(atlas["SCREEN_DU_MFD"]["w"], sizes["SCREEN_DU_MFD"][0])
        rects = list(atlas.values())
        for i, a in enumerate(rects):
            self.assertLessEqual(a["x"] + a["w"], 4096)
            self.assertLessEqual(a["y"] + a["h"], 4096)
            for b in rects[i + 1:]:
                overlap = a["x"] < b["x"] + b["w"] and b["x"] < a["x"] + a["w"] and a["y"] < b["y"] + b["h"] and b["y"] < a["y"] + a["h"]
                self.assertFalse(overlap)


if __name__ == "__main__":
    unittest.main()
