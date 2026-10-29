import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import acf_patch

ACF = '\n'.join([
    'I',
    '1200 Version',
    'P acf/_flap1_dn/4 26.000000000',
    'P acf/_flap1_dn/5 32.000000000',
    'P acf/_flap2_dn/5 32.000000000',
    'P _gear/0/_steerdeg_lospeed 70.000000000',
    'P _gear/1/_steerdeg_lospeed 8.000000000',
    'P _gear/2/_steerdeg_lospeed 8.000000000',
    'P acf/_Vso_kts 115.000000000',
    'P acf/_Vs_kts 171.000000000',
    'P acf/_m_max 1124355.000000000',
    'P acf/_m_empty 661403.000000000',
    '',
])

TEMPLATE = 'I\n1200 Version\nP acf/_flap1_dn/4 26.000000000\nP acf/_flap1_dn/5 33.000000000\nP acf/_m_max 1124355.000000000\n'


def built() -> dict[str, str]:
    with tempfile.TemporaryDirectory() as d:
        acf, template = Path(d) / 'A380.acf', Path(d) / 'A330.acf'
        acf.write_bytes(ACF.encode('latin-1'))
        template.write_bytes(TEMPLATE.encode('latin-1'))
        patch = acf_patch.make(acf, template)
        text, mismatched = acf_patch.apply(patch, template)
        assert mismatched == 0
        return acf_patch.properties(text)


class Tuning(unittest.TestCase):
    def setUp(self):
        self.p = built()

    def test_conf_full_flap1_is_33_deg(self):
        self.assertEqual(self.p['acf/_flap1_dn/5'], '33.000000000')

    def test_conf_full_flap2_is_33_deg(self):
        self.assertEqual(self.p['acf/_flap2_dn/5'], '33.000000000')

    def test_left_body_gear_steers_15_deg(self):
        self.assertEqual(self.p['_gear/1/_steerdeg_lospeed'], '15.000000000')

    def test_right_body_gear_steers_15_deg(self):
        self.assertEqual(self.p['_gear/2/_steerdeg_lospeed'], '15.000000000')

    def test_landing_stall_arc(self):
        self.assertEqual(self.p['acf/_Vso_kts'], '110.000000000')

    def test_clean_stall_arc(self):
        self.assertEqual(self.p['acf/_Vs_kts'], '150.000000000')

    def test_untuned_values_pass_through(self):
        self.assertEqual(self.p['acf/_flap1_dn/4'], '26.000000000')
        self.assertEqual(self.p['_gear/0/_steerdeg_lospeed'], '70.000000000')
        self.assertEqual(self.p['acf/_m_max'], '1124355.000000000')
        self.assertEqual(self.p['acf/_m_empty'], '661403.000000000')

    def test_a_missing_tuned_key_is_an_error(self):
        with self.assertRaises(ValueError):
            acf_patch.tune('P acf/_flap1_dn/5 32.000000000\n')


if __name__ == '__main__':
    unittest.main()
