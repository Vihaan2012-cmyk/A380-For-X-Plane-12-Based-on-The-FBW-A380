import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from acf_build import wing_mesh, pattern
from acf_decisions import DECIDED, OWNED

PLANFORM = {
    '_wing/0/_s_dim': '20', '_wing/0/_r_dim': '14', '_wing/0/_semilen_SEG': '40.0', '_wing/0/_Croot': '30.0',
    '_wing/0/_Ctip': '20.0', '_wing/0/_sweep_design': '30.0', '_wing/0/_dihed_design': '5.0',
    '_wing/0/_part_x': '0.0', '_wing/0/_part_y': '-2.0', '_wing/0/_part_z': '10.0',
}


def point(i, j):
    return [wing_mesh(PLANFORM, 0, f'_wing/0/_geo_xyz/{i},{j},{k}') for k in range(3)]


class WingMesh(unittest.TestCase):
    def test_the_root_section_spans_its_chord_about_the_quarter_chord(self):
        le, te = point(0, 0), point(0, 6)
        self.assertAlmostEqual(te[2] - le[2], 30.0, places=6)
        self.assertAlmostEqual(le[2] + 0.25 * 30.0, 10.0, places=6)

    def test_the_tip_lies_on_the_swept_quarter_chord_line(self):
        le = point(19, 0)
        self.assertAlmostEqual(le[2] + 0.25 * 20.0, 10.0 + 40.0 * 0.5, places=6)
        self.assertLess(le[0], 0.0)

    def test_the_section_closes_and_has_thickness(self):
        self.assertEqual(point(5, 0), point(5, 13))
        self.assertGreater(point(5, 3)[1], point(5, 10)[1])

    def test_slots_beyond_the_dimensions_are_zero(self):
        self.assertEqual(point(5, 15), [0.0, 0.0, 0.0])


class Decisions(unittest.TestCase):
    def test_every_decision_names_its_source(self):
        for key, (value, source) in DECIDED.items():
            self.assertTrue(source.strip(), key)
            self.assertNotEqual(value, '', key)

    def test_nozzle_offsets_mirror_left_and_right(self):
        for a, b in ((0, 3), (1, 2)):
            ax, bx = (float(DECIDED[f'_engn/{e}/_exhaust_os_xyz/0'][0]) for e in (a, b))
            self.assertAlmostEqual(ax, -bx, places=6)
            self.assertEqual(DECIDED[f'_engn/{a}/_exhaust_os_xyz/2'], DECIDED[f'_engn/{b}/_exhaust_os_xyz/2'])

    def test_green_and_yellow_engine_pumps(self):
        self.assertEqual([DECIDED[f'acf/_hyd_engn_pump_0123/{e}'][0] for e in range(4)], ['1', '1', '2', '2'])

    def test_owned_prefixes_are_unique(self):
        prefixes = [p for p, _ in OWNED]
        self.assertEqual(len(prefixes), len(set(prefixes)))

    def test_pattern_ignores_indices(self):
        self.assertEqual(pattern('_wing/12/_geo_xyz/1,2,0'), '_wing/*/_geo_xyz/*')


if __name__ == '__main__':
    unittest.main()
