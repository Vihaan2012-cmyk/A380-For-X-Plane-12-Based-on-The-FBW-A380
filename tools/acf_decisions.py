"""A380 values for the .acf properties acf_build.py cannot settle by rule, each with its source.

DECIDED: key -> (value, source). OWNED: key patterns for X-Plane systems the plugin replaces;
their values are never used, so they take the value most of Laminar's aircraft share for that
property (any index), which says nothing about the A330.
"""

FCOM = 'A380 FCOM'
FCOM_HYD = 'A380 FCOM DSC-29-10 (GREEN is X-Plane hydraulic system 1, YELLOW is system 2)'
ACAP = 'Airbus A380 Aircraft Characteristics, Airport and Maintenance Planning, Dec 2023'
TCDS = 'EASA TCDS E.012 issue 12, Trent 900'
EEDB = 'ICAO Aircraft Engine Emissions Databank v32, Trent 972-84'
FBW_FM = 'FlyByWire A380X flight_model.cfg'
FBW_FC = 'FlyByWire A380 flight-control actuator ranges, as the plugin writes them (src/flight_controls.rs)'
OBJ = 'our engine objects (a380_exterior_020/022.obj): core nozzle exit centre minus the engine reference point'
ELEC = 'A380 FCOM DSC-24 sources (4 x 150 kVA, 4 x 50 Ah); the plugin runs the A380 network, X-Plane buses only light X-Plane\'s named lights'

FT = 3.28084
LB_PER_SLUG = 32.174
EMPTY_LB = 661403.0
M2_PER_FT2 = 0.09290304


def _unitmass(moi_slug_ft2):
    return moi_slug_ft2 / (EMPTY_LB / LB_PER_SLUG) * M2_PER_FT2


def _nozzle(engine):
    centre = [(-25.80, -0.68, 2.15), (-14.87, -1.09, -5.90)]
    ref = [(-84.0, -1.5, 10.0), (-47.5, -4.0, -15.0)]
    outboard = engine in (0, 3)
    c, r = centre[0 if outboard else 1], ref[0 if outboard else 1]
    side = -1.0 if engine < 2 else 1.0
    return (side * abs(c[0] * FT) - side * abs(r[0]), c[1] * FT - r[1], c[2] * FT - r[2])


DECIDED = {
    'acf/_manufacturer': ('Airbus', 'type'),
    'acf/_tailnum': ('F-WWOW', 'first A380 registration'),
    '_gear/0/_cyc_time': ('17.000000000', FBW_FM + ' [CONTACT_POINTS] point.0 extension time'),
    '_gear/1/_cyc_time': ('21.100000000', FBW_FM + ' [CONTACT_POINTS] point.1 extension time'),
    '_gear/2/_cyc_time': ('21.100000000', FBW_FM + ' [CONTACT_POINTS] point.2 extension time'),
    'acf/_Gpos': ('2.500000000', 'CS 25.337 limit load factor'),
    'acf/_Gneg': ('1.000000000', 'CS 25.337 limit load factor'),

    # Hydraulics
    'acf/_hyd_engn_pump_0123/0': ('1', FCOM_HYD + ': engines 1 and 2 pressurise GREEN'),
    'acf/_hyd_engn_pump_0123/1': ('1', FCOM_HYD + ': engines 1 and 2 pressurise GREEN'),
    'acf/_hyd_engn_pump_0123/2': ('2', FCOM_HYD + ': engines 3 and 4 pressurise YELLOW'),
    'acf/_hyd_engn_pump_0123/3': ('2', FCOM_HYD + ': engines 3 and 4 pressurise YELLOW'),
    'acf/_hyd_ele_pump_0123/0': ('1', FCOM_HYD + ': electric pumps on each system'),
    'acf/_hyd_ele_pump_0123/1': ('2', FCOM_HYD + ': electric pumps on each system'),
    'acf/_hyd_ele_pump_0123/2': ('0', FCOM_HYD + ': one electric pump per system here, third slot unused'),
    'acf/_hyd_rat_pump_0123': ('0', FCOM + ' DSC-24: the RAT drives a generator, not a pump'),
    'acf/_hyd_brakes': ('3', FCOM + ' DSC-32: normal braking GREEN for the wing gear, YELLOW for the body gear'),
    'acf/_hyd_flaps': ('3', FCOM_HYD + ': flaps GREEN and YELLOW'),
    'acf/_hyd_slats': ('1', FCOM_HYD + ': slats GREEN'),
    'acf/_hyd_gear': ('1', FCOM_HYD + ': wing and nose gear actuation GREEN'),
    'acf/_hyd_nw_steer': ('1', FCOM_HYD + ': nosewheel steering GREEN'),
    'acf/_hyd_thrust_rev_L': ('0', FCOM + ' DSC-70: reversers are electrically actuated (ETRAS)'),
    'acf/_hyd_thrust_rev_R': ('0', FCOM + ' DSC-70: reversers are electrically actuated (ETRAS)'),
    'acf/_hyd_max_p': ('5200.000000000', FCOM + ' DSC-29: 5 000 to 5 200 psi in normal operation'),
    'acf/_hyd_max_p_eng': ('5000.000000000', FCOM + ' DSC-29: normal operating pressure 5 000 psi'),
    'acf/_hyd_max_p_ele': ('5000.000000000', FCOM + ' DSC-29: normal operating pressure 5 000 psi'),
    'acf/_hyd_max_p_RAT': ('0.000000000', FCOM + ' DSC-24: no hydraulic RAT'),
    'acf/_hyd_gear_fail_speed': ('250.000000000', FCOM + ' LIM-32: VLO 250 kt'),
    '_wing/2/_spoi1_hyd0123': ('2', FCOM_HYD + ': odd spoilers YELLOW'),
    '_wing/3/_spoi1_hyd0123': ('2', FCOM_HYD + ': odd spoilers YELLOW'),
    '_wing/2/_spoi2_hyd0123': ('1', FCOM_HYD + ': even spoilers GREEN'),
    '_wing/3/_spoi2_hyd0123': ('1', FCOM_HYD + ': even spoilers GREEN'),
    '_wing/4/_spoi2_hyd0123': ('1', FCOM_HYD + ': even spoilers GREEN'),
    '_wing/5/_spoi2_hyd0123': ('1', FCOM_HYD + ': even spoilers GREEN'),
    '_wing/4/_ailn1_hyd0123': ('1', FCOM_HYD + ': inboard ailerons GREEN'),
    '_wing/5/_ailn1_hyd0123': ('1', FCOM_HYD + ': inboard ailerons GREEN'),
    '_wing/6/_ailn2_hyd0123': ('3', FCOM_HYD + ': outboard ailerons GREEN and YELLOW'),
    '_wing/7/_ailn2_hyd0123': ('3', FCOM_HYD + ': outboard ailerons GREEN and YELLOW'),
    '_wing/8/_elev1_hyd0123': ('3', FCOM_HYD + ': elevators GREEN and YELLOW'),
    '_wing/9/_elev1_hyd0123': ('3', FCOM_HYD + ': elevators GREEN and YELLOW'),
    '_wing/10/_rudd1_hyd0123': ('3', FCOM_HYD + ': rudder GREEN and YELLOW'),

    # Surface travel: the plugin writes the angles; X-Plane's limits must not clip them
    'acf/_ailn2_up': ('30.000000000', FBW_FC + ': ailerons 30 up'),
    'acf/_ailn2_dn': ('20.000000000', FBW_FC + ': ailerons 20 down'),
    'acf/_elev1_up': ('30.000000000', FBW_FC + ': elevators 30 up'),
    'acf/_spoi1_up': ('50.000000000', FBW_FC + ': spoilers 0 to 50'),
    'acf/_spoi2_up': ('50.000000000', FBW_FC + ': spoilers 0 to 50'),
    'acf/_slat1_inc': ('23.000000000', FCOM + ' LIM-13: slats 23 degrees in CONF 3 and FULL'),
    'acf/_slat2_inc': ('23.000000000', FCOM + ' LIM-13: slats 23 degrees in CONF 3 and FULL'),

    # Limits and airframe
    'acf/_Vle_kts': ('250.000000000', FCOM + ' LIM-32: VLE 250 kt'),
    'acf/_max_press_diff': ('9.000000000', FCOM + ' LIM-21-30: maximum positive differential 9 psi'),
    'acf/_average_mac_acf': ('40.350000000', 'FlyByWire a380_systems airframe: MAC 40.35 ft'),
    'acf/_RAT_KEAS': ('140.000000000', FCOM + ' DSC-24: below 140 kt the static inverter takes over from the RAT'),
    'acf/_Jxx_unitmass': (f'{_unitmass(60518042):.9f}', FBW_FM + ': empty pitch MOI over empty weight, m2'),
    'acf/_Jyy_unitmass': (f'{_unitmass(88229309):.9f}', FBW_FM + ': empty yaw MOI over empty weight, m2'),
    'acf/_Jzz_unitmass': (f'{_unitmass(53559900):.9f}', FBW_FM + ': empty roll MOI over empty weight, m2'),

    # Gear
    '_gear/0/_tire_psi': ('205.000000000', ACAP + ' 7-2-0: nose gear 14.1 bar'),
    '_gear/1/_tire_psi': ('218.000000000', ACAP + ' 7-2-0: body gear 15 bar'),
    '_gear/2/_tire_psi': ('218.000000000', ACAP + ' 7-2-0: body gear 15 bar'),
    '_gear/3/_tire_psi': ('218.000000000', ACAP + ' 7-2-0: wing gear 15 bar'),
    '_gear/4/_tire_psi': ('218.000000000', ACAP + ' 7-2-0: wing gear 15 bar'),
    '_gear/0/_gear_type': ('3', ACAP + ': two-wheel nose gear'),
    '_gear/1/_gear_type': ('6', ACAP + ': six-wheel body gear (X-Plane type 6, as Laminar\'s 777)'),
    '_gear/2/_gear_type': ('6', ACAP + ': six-wheel body gear (X-Plane type 6, as Laminar\'s 777)'),
    '_gear/0/_steerdeg_hispeed': ('6.000000000', FBW_FM + ': 6 degrees of steering left at speed'),
    'acf/_truck_lat_sep': (f'{1.530 * FT:.9f}', ACAP + ' 7-2-0: 1.530 m between wheels'),
    'acf/_truck_lon_sep': (f'{1.700 / 2 * FT:.9f}', ACAP + ' 7-2-0: 1.700 m between axles, half as Laminar\'s 777 gives it'),

    # Engines: X-Plane's turbine no longer makes the thrust (engine_forces.rs); these set its gauges and fan
    '_engn/0/_type': ('JET_2SPOOL', 'X-Plane has no three-spool type; the closest high-bypass type'),
    '_engn/1/_type': ('JET_2SPOOL', 'X-Plane has no three-spool type; the closest high-bypass type'),
    'acf/_bypass_ratio': ('8.410000000', EEDB),
    'acf/_fan_rpm_for_n1_100': ('2900.000000000', TCDS + ': LP 100 % = 2 900 rpm'),
    'acf/_face_jet': (f'{3.14159265 * (2.95 / 2) ** 2 * FT * FT:.9f}', TCDS + ': fan diameter 2.95 m'),
    'acf/_ff_rat_idle_JET': (f'{0.258 / 2.672:.9f}', EEDB + ': idle over take-off fuel flow'),
    'acf/_jet_SFC_takeoff': (f'{2.672 / 345.95 * 3600 / 0.45359 * 4.44822 / 1000:.9f}', EEDB + ': take-off fuel flow over thrust, lb/lbf/h'),
    'acf/_max_EGT': ('900.000000000', TCDS + ': TGT 900 C take-off'),
    'acf/_red_hi_EGT': ('900.000000000', TCDS + ': TGT 900 C take-off'),
    'acf/_green_hi_EGT': ('850.000000000', TCDS + ': TGT 850 C maximum continuous'),
    'acf/_red_hi_N1': ('97.199996948', TCDS + ': LP 97.2 % take-off'),
    'acf/_green_hi_N1': ('97.199996948', TCDS + ': LP 97.2 % take-off'),
    'acf/_red_hi_N2': ('97.800003052', TCDS + ': HP 97.8 % take-off'),
    'acf/_red_lo_oilP': ('25.000000000', TCDS + ': minimum oil pressure 172 kPa'),
    'acf/_green_lo_oilP': ('25.000000000', TCDS + ': minimum oil pressure 172 kPa'),
    'acf/_max_oilT': ('196.000000000', TCDS + ': oil scavenge 196 C unrestricted'),
    'acf/_red_hi_oilT': ('196.000000000', TCDS + ': oil scavenge 196 C unrestricted'),
    'acf/_green_hi_oilT': ('196.000000000', TCDS + ': oil scavenge 196 C unrestricted'),

    # X-Plane's electrical: only so its named lights shine; four AC buses on four generators, two DC
    # buses on the four batteries, every relay on any live bus
    'acf/_num_buses': ('6', ELEC),
    'acf/_num_batteries': ('4', ELEC),
    'acf/_num_generators': ('4', ELEC),
    'acf/_bus_essential': ('4', ELEC),
    'acf/_nom_gen_volt': ('115.000000000', ELEC),
    'acf/_battery_watt_hr_max': (f'{50 * 24:.9f}', ELEC + ': 50 Ah at 24 V'),
    'acf/_max_gen_amp0': (f'{150000 / (3 * 115):.9f}', ELEC + ': 150 kVA per phase at 115 V'),
    'acf/_max_inverter': (f'{2500 / 115:.9f}', FCOM + ' DSC-24: 2.5 kVA static inverter'),
}
for g in range(4):
    DECIDED[f'acf/_bus_gen_is_on/{g}'] = (str(g), ELEC)
    DECIDED[f'acf/_bus_bat_is_on/{g}'] = (str(4 + g % 2), ELEC)
    DECIDED[f'acf/_max_gen_amp_per_gen/{g}'] = (f'{150000 / (3 * 115):.9f}', ELEC)
    for axis, v in zip('012', _nozzle(g)):
        DECIDED[f'_engn/{g}/_exhaust_os_xyz/{axis}'] = (f'{v:.9f}', OBJ)
for w in (32, 33):
    DECIDED[f'_wing/{w}/_dihed_design'] = ('90.000000000', 'outboard engine pylon, a vertical surface at the engine (x 85 ft)')
for w in (10, 12, 13, 32, 33):
    for n in (1, 2, 3):
        DECIDED[f'_wing/{w}/_afl_file_{n}'] = ('NACA 0009 (symmetrical).afl', 'X-Plane library symmetrical section for the fin, tip fences and pylons')
for b in range(4):
    DECIDED[f'_sbrk/{b}/_area'] = ('0.000000000', FCOM + ' DSC-27: the spoilers are the speed brakes; no separate panels')
DECIDED.update({
    'acf/_rad_alt_offset': ('-17.022998681', 'radio height reads zero at rest: minus acf/_h_eqlbm, as Laminar sets its XP12 aircraft'),
    '_body/1/_descrip': ('wing to body fairing', 'part name'),
    '_body/21/_descrip': ('nacelle 1', 'part name: at engine 1'),
    '_body/22/_descrip': ('nacelle 2', 'part name: at engine 2'),
    '_lite/1/_lite_type': ('beacon_strobe', 'hidden slot (_hide 1); our objects draw the beacons'),
    '_obja/17/_obj_flags': ('13', 'render flags of our object a380_cockpit_017.obj'),
    '_obja/19/_obj_flags': ('13', 'render flags of our object a380_cockpit_019.obj'),
    '_obja/27/_obj_flags': ('8195', 'render flags of our object a380_cockpit_027.obj'),
    'acf/_roll_co': ('0.025000000', "tyre rolling coefficient most of Laminar's aircraft use; no A380 figure is published"),
})
for b in range(6):
    DECIDED[f'acf/_esys_bus_bit_add/rel_bus{b}'] = (str(1 << b), ELEC + ': each bus on itself')

RELAYS_ON_ANY_BUS = ('acf/_esys_bus_bit_add/', str((1 << 6) - 1), ELEC)

OWNED = [
    ('acf/_esys_amperage/', 'the plugin runs the electrical loads'),
    ('acf/_batt_power_rat/', 'the plugin runs the batteries'),
    ('acf/_green_hi_bat_volt', 'X-Plane gauge; the A380 displays are the plugin\'s'),
    ('acf/_green_lo_bat_volt', 'X-Plane gauge; the A380 displays are the plugin\'s'),
    ('acf/_red_hi_bat_volt', 'X-Plane gauge; the A380 displays are the plugin\'s'),
    ('acf/_red_lo_bat_volt', 'X-Plane gauge; the A380 displays are the plugin\'s'),
    ('acf/_nom_bat_volt', 'the plugin runs the batteries'),
    ('acf/_max_bat_temp', 'the plugin runs the batteries'),
    ('acf/_ott_', 'the plugin is the autopilot'),
    ('acf/_FBW', 'the plugin is the fly-by-wire'),
    ('acf/_AShi_', 'the plugin is the fly-by-wire'),
    ('acf/_click_step_otto_alt', 'the plugin is the autopilot'),
    ('acf/_auto_psi_for_dir_change', 'the plugin is the autopilot'),
    ('acf/_hdng_max_full_def_KIAS', 'the plugin moves the surfaces'),
    ('acf/_con_smooth_', 'the plugin moves the surfaces'),
    ('acf/_ailn', 'the plugin moves the surfaces'),
    ('acf/_elev', 'the plugin moves the surfaces'),
    ('acf/_rudd', 'the plugin moves the surfaces'),
    ('acf/_spoi', 'the plugin moves the surfaces'),
    ('acf/_sbrk', 'the plugin moves the surfaces'),
    ('acf/_speedbrake_', 'the plugin moves the surfaces'),
    ('acf/_roll_to_eng_spo', 'the plugin moves the surfaces'),
    ('acf/_roll_acft_', 'the plugin moves the surfaces'),
    ('acf/_deg_psi_per_deg_def', 'the plugin moves the surfaces'),
    ('acf/_sster_def_time', 'the plugin steers the wheels'),
    ('acf/_takeoff_trim', 'the plugin sets the trim'),
    ('acf/_flap1_crat', 'the plugin moves the flaps'),
    ('acf/_flap2_crat', 'the plugin moves the flaps'),
    ('acf/_slat_ext', 'the plugin moves the slats'),
    ('acf/_slat_ret', 'the plugin moves the slats'),
    ('acf/_flap_pumps', 'the plugin runs the hydraulics'),
    ('acf/_gear_pumps', 'the plugin runs the hydraulics'),
    ('acf/_APU_', 'the plugin runs the APU'),
    ('acf/_o2_bottle_liter', 'the plugin runs the oxygen'),
    ('acf/_bleed_', 'the plugin runs the bleed'),
    ('acf/_tank_psi/', 'the plugin runs the fuel pumps'),
    ('acf/_dump_altitude', 'the plugin runs the jettison'),
    ('acf/_tat_probes/', 'the plugin runs the air data'),
    ('acf/_fadec_', 'the plugin is the FADEC'),
    ('acf/_throt_', 'the plugin is the FADEC'),
    ('acf/_thrust_max_limit', 'X-Plane\'s turbine does not make the thrust'),
    ('acf/_thrust_rev_dep_time', 'the plugin runs the reversers'),
    ('acf/_reverser_area', 'the plugin runs the reversers'),
    ('acf/_jet_', 'X-Plane\'s turbine does not make the thrust'),
    ('acf/_SFC_', 'X-Plane\'s turbine does not make the thrust'),
    ('acf/_n1_power_curve', 'X-Plane\'s turbine does not make the thrust'),
    ('acf/_n2_power_curve', 'X-Plane\'s turbine does not make the thrust'),
    ('acf/_idle_rat/', 'X-Plane\'s turbine does not make the thrust'),
    ('acf/_ff_rat_idle_PRP', 'no propellers'),
    ('acf/_ff_rat_idle_turbo', 'no turboprops'),
    ('acf/_spool_time_', 'the plugin spools the engines'),
    ('acf/_starter_', 'the plugin starts the engines'),
    ('acf/_fuel_intro_time_', 'the plugin starts the engines'),
    ('acf/_lock_n2', 'the plugin runs the generators'),
    ('acf/_generator_design_rpm_rat', 'the plugin runs the generators'),
    ('acf/_exhaust_rat', 'X-Plane\'s turbine does not make the thrust'),
    ('acf/_inlet_', 'X-Plane\'s turbine does not make the thrust'),
    ('acf/_min_n2_', 'the plugin starts the engines'),
    ('acf/_max_ITT', 'no turboprops'),
    ('acf/_green_hi_ITT', 'no turboprops'),
    ('acf/_green_lo_ITT', 'no turboprops'),
    ('acf/_red_hi_ITT', 'no turboprops'),
    ('acf/_max_CHT', 'no pistons'),
    ('acf/_max_MP_limit', 'no pistons'),
    ('acf/_crit_alt_mtr', 'no pistons'),
    ('acf/_max_stator', 'no turboprops'),
    ('acf/_max_mach_eff', 'X-Plane\'s turbine does not make the thrust'),
    ('acf/_green_hi_oilP', 'X-Plane gauge; the A380 displays are the plugin\'s'),
    ('acf/_red_hi_oilP', 'X-Plane gauge; the A380 displays are the plugin\'s'),
    ('acf/_nom_oilP', 'X-Plane gauge; the A380 displays are the plugin\'s'),
    ('acf/_autofeather_', 'no propellers'),
    ('acf/_prop_pitch_', 'no propellers'),
    ('acf/_num_xmsn', 'no propellers'),
    ('acf/_trans_loss_bearing', 'no propellers'),
    ('acf/_jato_', 'no JATO'),
    ('acf/_water_', 'no water bombing'),
    ('acf/_Vbg_kts', 'X-Plane AI speeds; the plugin flies the aircraft'),
    ('acf/_Vx_kts', 'X-Plane AI speeds; the plugin flies the aircraft'),
    ('acf/_Vy_kts', 'X-Plane AI speeds; the plugin flies the aircraft'),
    ('acf/_min_rwy_len', 'X-Plane AI'),
    ('acf/_fbrk_rto_vind', 'the plugin is the autobrake'),
    ('acf/_ann_wav_file/', 'the plugin makes the warnings'),
    ('acf/_snd_', 'the sound pack makes the sounds'),
    ('acf/_HUD_', 'the plugin draws the HUD'),
    ('acf/_cockpit_planned_vanish_y/', 'X-Plane 2D panel'),
    ('acf/_cus_rnd_hi_ang/', 'X-Plane 2D panel'),
    ('acf/_yawstring_', 'X-Plane 2D panel'),
    ('acf/_spot_', 'our objects draw the lights'),
    ('acf/_rgb_lite_', 'our objects draw the lights'),
    ('acf/_wiper_', 'our objects draw the wipers'),
    ('acf/_ang_offset/', 'X-Plane 2D panel'),
    ('_blad/', 'no propellers'),
    ('_prop/', 'no propellers'),
    ('_gear/', 'our objects draw the gear; X-Plane\'s retraction geometry is not shown'),
    ('acf/_tire_', 'our objects draw the gear'),
    ('acf/_wheel_', 'our objects draw the gear'),
    ('acf/_eagle_shock_to_tire_def_rat', 'our objects draw the gear'),
    ('_engn/', 'X-Plane\'s turbine does not make the thrust'),
] + [(f'_body/1/{h}', 'Plane Maker body editor handle; the shape is _geo_xyz') for h in ('_bot_s2', '_bot_t2', '_top_s2', '_top_t2')]
