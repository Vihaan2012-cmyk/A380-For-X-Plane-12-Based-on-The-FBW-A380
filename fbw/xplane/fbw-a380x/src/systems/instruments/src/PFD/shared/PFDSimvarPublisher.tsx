import { Bus, VarSpec, VarUnit } from 'xplane-js/ui';
import { UpdatableSimVarPublisher } from '../../MsfsAvionicsCommon/UpdatableSimVarPublisher';

export interface PFDSimvars {
  coldDark: number;
  elec: number;
  elecFo: number;
  potentiometerCaptain: number;
  potentiometerFo: number;
  pitch: number;
  roll: number;
  heading: number;
  baroCorrectedAltitude1: number;
  baroCorrectedAltitude2: number;
  pressureAltitude: number;
  speed: number;
  staticPressureRaw: number;
  noseGearCompressed: boolean;
  leftMainGearCompressed: boolean;
  rightMainGearCompressed: boolean;
  mda: number;
  dh: number;
  attHdgKnob: number;
  airKnob: number;
  vsBaro: number;
  vsInert: number;
  sideStickX: number;
  sideStickY: number;
  v1: number;
  vr: number;
  fwcFlightPhase: number;
  fmgcFlightPhase: number;
  hasLoc: boolean;
  hasDme: boolean;
  navIdent: string;
  navFreq: number;
  dme: number;
  navRadialError: number;
  hasGlideslope: boolean;
  glideSlopeError: number;
  markerBeacon: number;
  mach: number;
  flapHandleIndex: number;
  groundTrack: number;
  aoa: number;
  groundHeadingTrue: number;
  groundTrackTrue: number;
  ilsCourse: number;
  tla1: number;
  tla2: number;
  tla3: number;
  tla4: number;
  landingElevation: number;
  tcasState: number;
  tcasCorrective: boolean;
  tcasRedZoneL: number;
  tcasRedZoneH: number;
  tcasGreenZoneL: number;
  tcasGreenZoneH: number;
  tcasFail: boolean;
  engOneRunning: boolean;
  engTwoRunning: boolean;
  engThreeRunning: boolean;
  engFourRunning: boolean;
  setHoldSpeed: boolean;
  tdReached: boolean;
  groundSpeed: number;
  radioAltitude1: number;
  radioAltitude2: number;
  radioAltitude3: number;
  beta: number;
  betaTargetActive: number;
  betaTarget: number;
  latAcc: number;
  flexTemp: number;
  autoBrakeMode: number;
  autoBrakeActive: boolean;
  autoBrakeDecel: boolean;
  fpaRaw: number;
  daRaw: number;
  xtk: number;
  ldevRequestLeft: boolean;
  ldevRequestRight: boolean;
  landingElevation1Raw: number;
  landingElevation2Raw: number;
  fm1NavDiscrete: number;
  fm1EisDiscrete2Raw: number;
  fm2EisDiscrete2Raw: number;
  fm1MdaRaw: number;
  fm2MdaRaw: number;
  fm1DhRaw: number;
  fm2DhRaw: number;
  fm1HealthyDiscrete: number;
  fm2HealthyDiscrete: number;
  fm1TransAltRaw: number;
  fm2TransAltRaw: number;
  fm1TransLvlRaw: number;
  fm2TransLvlRaw: number;
  lgciuDiscreteWord1Raw: number;
  slatPosLeft: number;
  trimPosition: number;
  cgPercent: number;
  spoilersCommanded: number;
  spoilersArmed: boolean;
  btvExitMissed: boolean;
  hydGreenSysPressurized: boolean;
  hydYellowSysPressurized: boolean;
}

export enum PFDVars {
  coldDark = 'L:A32NX_COLD_AND_DARK_SPAWN',
  elec = 'L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED',
  elecFo = 'L:A32NX_ELEC_AC_2_BUS_IS_POWERED',
  potentiometerCaptain = 'fbw/LIGHT_POTENTIOMETER_88',
  potentiometerFo = 'fbw/LIGHT_POTENTIOMETER_90',
  pitch = 'L:A32NX_ADIRS_IR_1_PITCH',
  roll = 'L:A32NX_ADIRS_IR_1_ROLL',
  heading = 'L:A32NX_ADIRS_IR_1_HEADING',
  baroCorrectedAltitude1 = 'L:A32NX_ADIRS_ADR_1_BARO_CORRECTED_ALTITUDE_1',
  baroCorrectedAltitude2 = 'L:A32NX_ADIRS_ADR_1_BARO_CORRECTED_ALTITUDE_2',
  pressureAltitude = 'L:A32NX_ADIRS_ADR_1_ALTITUDE',
  speed = 'L:A32NX_ADIRS_ADR_1_COMPUTED_AIRSPEED',
  staticPressureRaw = 'L:A32NX_ADIRS_ADR_1_CORRECTED_AVERAGE_STATIC_PRESSURE',
  noseGearCompressed = 'L:A32NX_LGCIU_1_NOSE_GEAR_COMPRESSED',
  leftMainGearCompressed = 'L:A32NX_LGCIU_1_LEFT_GEAR_COMPRESSED',
  rightMainGearCompressed = 'L:A32NX_LGCIU_1_RIGHT_GEAR_COMPRESSED',
  mda = 'L:AIRLINER_MINIMUM_DESCENT_ALTITUDE',
  dh = 'L:AIRLINER_DECISION_HEIGHT',
  attHdgKnob = 'L:A32NX_ATT_HDG_SWITCHING_KNOB',
  airKnob = 'L:A32NX_AIR_DATA_SWITCHING_KNOB',
  vsBaro = 'L:A32NX_ADIRS_ADR_1_BAROMETRIC_VERTICAL_SPEED',
  vsInert = 'L:A32NX_ADIRS_IR_1_VERTICAL_SPEED',
  sideStickX = 'L:A32NX_SIDESTICK_POSITION_X',
  sideStickY = 'L:A32NX_SIDESTICK_POSITION_Y',
  v1 = 'L:AIRLINER_V1_SPEED',
  vr = 'L:AIRLINER_VR_SPEED',
  fwcFlightPhase = 'L:A32NX_FWC_FLIGHT_PHASE',
  fmgcFlightPhase = 'L:A32NX_FMGC_FLIGHT_PHASE',
  hasLoc = 'L:A32NX_RADIO_RECEIVER_LOC_IS_VALID',
  hasDme = 'fbw/NAV_HAS_DME_3',
  navIdent = 'fbw/NAV_IDENT_3',
  navFreq = 'fbw/NAV_FREQUENCY_3',
  dme = 'fbw/NAV_DME_3',
  navRadialError = 'L:A32NX_RADIO_RECEIVER_LOC_DEVIATION',
  hasGlideslope = 'L:A32NX_RADIO_RECEIVER_GS_IS_VALID',
  glideSlopeError = 'L:A32NX_RADIO_RECEIVER_GS_DEVIATION',
  markerBeacon = 'fbw/MARKER_BEACON_STATE',
  mach = 'L:A32NX_ADIRS_ADR_1_MACH',
  flapHandleIndex = 'L:A32NX_FLAPS_HANDLE_INDEX',
  groundTrack = 'L:A32NX_ADIRS_IR_1_TRACK',
  aoa = 'fbw/INCIDENCE_ALPHA',
  groundHeadingTrue = 'fbw/GPS_GROUND_TRUE_HEADING',
  groundTrackTrue = 'fbw/GPS_GROUND_TRUE_TRACK',
  ilsCourse = 'L:A32NX_FM_LS_COURSE',
  metricAltToggle = 'L:A32NX_METRIC_ALT_TOGGLE',
  tla1 = 'L:A32NX_AUTOTHRUST_TLA:1',
  tla2 = 'L:A32NX_AUTOTHRUST_TLA:2',
  tla3 = 'L:A32NX_AUTOTHRUST_TLA:3',
  tla4 = 'L:A32NX_AUTOTHRUST_TLA:4',
  tcasState = 'L:A32NX_TCAS_STATE',
  tcasCorrective = 'L:A32NX_TCAS_RA_CORRECTIVE',
  tcasRedZoneL = 'L:A32NX_TCAS_VSPEED_RED:1',
  tcasRedZoneH = 'L:A32NX_TCAS_VSPEED_RED:2',
  tcasGreenZoneL = 'L:A32NX_TCAS_VSPEED_GREEN:1',
  tcasGreenZoneH = 'L:A32NX_TCAS_VSPEED_GREEN:2',
  tcasFail = 'L:A32NX_TCAS_FAULT',
  engOneRunning = 'fbw/GENERAL_ENG_COMBUSTION_1',
  engTwoRunning = 'fbw/GENERAL_ENG_COMBUSTION_2',
  engThreeRunning = 'fbw/GENERAL_ENG_COMBUSTION_3',
  engFourRunning = 'fbw/GENERAL_ENG_COMBUSTION_4',
  setHoldSpeed = 'L:A32NX_PFD_MSG_SET_HOLD_SPEED',
  tdReached = 'L:A32NX_PFD_MSG_TD_REACHED',
  groundSpeed = 'L:A32NX_ADIRS_IR_1_GROUND_SPEED',
  radioAltitude1 = 'L:A32NX_RA_1_RADIO_ALTITUDE',
  radioAltitude2 = 'L:A32NX_RA_2_RADIO_ALTITUDE',
  radioAltitude3 = 'L:A32NX_RA_3_RADIO_ALTITUDE',
  beta = 'sim/flightmodel/position/beta',
  betaTargetActive = 'L:A32NX_BETA_TARGET_ACTIVE',
  betaTarget = 'L:A32NX_BETA_TARGET',
  latAcc = 'sim/flightmodel/forces/g_side',
  flexTemp = 'L:A32NX_AIRLINER_TO_FLEX_TEMP',
  autoBrakeMode = 'L:A32NX_AUTOBRAKES_ARMED_MODE',
  autoBrakeActive = 'L:A32NX_AUTOBRAKES_ACTIVE',
  autoBrakeDecel = 'L:A32NX_AUTOBRAKES_DECEL_LIGHT',
  fpaRaw = 'L:A32NX_ADIRS_IR_1_FLIGHT_PATH_ANGLE',
  daRaw = 'L:A32NX_ADIRS_IR_1_DRIFT_ANGLE',
  xtk = 'L:A32NX_FG_CROSS_TRACK_ERROR',
  ldevLeft = 'L:A32NX_FMGC_L_LDEV_REQUEST',
  ldevRight = 'L:A32NX_FMGC_R_LDEV_REQUEST',
  landingElevation1Raw = 'L:A32NX_FM1_LANDING_ELEVATION',
  landingElevation2Raw = 'L:A32NX_FM2_LANDING_ELEVATION',
  fm1NavDiscrete = 'L:A32NX_FM1_NAV_DISCRETE',
  fm1EisDiscrete2 = 'L:A32NX_FM1_EIS_DISCRETE_WORD_2',
  fm2EisDiscrete2 = 'L:A32NX_FM2_EIS_DISCRETE_WORD_2',
  fm1MdaRaw = 'L:A32NX_FM1_MINIMUM_DESCENT_ALTITUDE',
  fm2MdaRaw = 'L:A32NX_FM2_MINIMUM_DESCENT_ALTITUDE',
  fm1DhRaw = 'L:A32NX_FM1_DECISION_HEIGHT',
  fm2DhRaw = 'L:A32NX_FM1_DECISION_HEIGHT',
  fm1HealthyDiscrete = 'L:A32NX_FM1_HEALTHY_DISCRETE',
  fm2HealthyDiscrete = 'L:A32NX_FM2_HEALTHY_DISCRETE',
  fm1TransAltRaw = 'L:A32NX_FM1_TRANS_ALT',
  fm2TransAltRaw = 'L:A32NX_FM2_TRANS_ALT',
  fm1TransLvlRaw = 'L:A32NX_FM1_TRANS_LVL',
  fm2TransLvlRaw = 'L:A32NX_FM2_TRANS_LVL',
  lgciuDiscreteWord1Raw = 'L:A32NX_LGCIU_1_DISCRETE_WORD_1',
  slatPosLeft = 'L:A32NX_LEFT_SLATS_ANGLE',
  trimPosition = 'fbw/ELEVATOR_TRIM_POSITION',
  spoilersCommanded = 'L:A32NX_LEFT_SPOILER_1_COMMANDED_POSITION',
  spoilersArmed = 'L:A32NX_SPOILERS_ARMED',
  btvExitMissed = 'L:A32NX_BTV_EXIT_MISSED',
  hydGreenSysPressurized = 'L:A32NX_HYD_GREEN_SYSTEM_1_SECTION_PRESSURE_SWITCH',
  hydYellowSysPressurized = 'L:A32NX_HYD_YELLOW_SYSTEM_1_SECTION_PRESSURE_SWITCH',
}

/** A publisher to poll and publish nav/com simvars. */
export class PFDSimvarPublisher extends UpdatableSimVarPublisher<PFDSimvars> {
  private static simvars = new Map<keyof PFDSimvars, VarSpec>([
    ['elec', { name: PFDVars.elec, type: VarUnit.Bool }],
    ['elecFo', { name: PFDVars.elecFo, type: VarUnit.Bool }],
    ['potentiometerCaptain', { name: PFDVars.potentiometerCaptain, type: VarUnit.Number }],
    ['potentiometerFo', { name: PFDVars.potentiometerFo, type: VarUnit.Number }],
    ['pitch', { name: PFDVars.pitch, type: VarUnit.Number }],
    ['roll', { name: PFDVars.roll, type: VarUnit.Number }],
    ['heading', { name: PFDVars.heading, type: VarUnit.Number }],
    ['baroCorrectedAltitude1', { name: PFDVars.baroCorrectedAltitude1, type: VarUnit.Number }],
    ['baroCorrectedAltitude2', { name: PFDVars.baroCorrectedAltitude1, type: VarUnit.Number }],
    ['pressureAltitude', { name: PFDVars.pressureAltitude, type: VarUnit.Number }],
    ['speed', { name: PFDVars.speed, type: VarUnit.Number }],
    ['staticPressureRaw', { name: PFDVars.staticPressureRaw, type: VarUnit.Number }],
    ['noseGearCompressed', { name: PFDVars.noseGearCompressed, type: VarUnit.Bool }],
    ['leftMainGearCompressed', { name: PFDVars.leftMainGearCompressed, type: VarUnit.Bool }],
    ['rightMainGearCompressed', { name: PFDVars.rightMainGearCompressed, type: VarUnit.Bool }],
    ['mda', { name: PFDVars.mda, type: VarUnit.Feet }],
    ['dh', { name: PFDVars.dh, type: VarUnit.Feet }],
    ['attHdgKnob', { name: PFDVars.attHdgKnob, type: VarUnit.Enum }],
    ['airKnob', { name: PFDVars.airKnob, type: VarUnit.Enum }],
    ['vsBaro', { name: PFDVars.vsBaro, type: VarUnit.Number }],
    ['vsInert', { name: PFDVars.vsInert, type: VarUnit.Number }],
    ['sideStickX', { name: PFDVars.sideStickX, type: VarUnit.Number }],
    ['sideStickY', { name: PFDVars.sideStickY, type: VarUnit.Number }],
    ['v1', { name: PFDVars.v1, type: VarUnit.Knots }],
    ['vr', { name: PFDVars.vr, type: VarUnit.Knots }],
    ['fwcFlightPhase', { name: PFDVars.fwcFlightPhase, type: VarUnit.Number }],
    ['fmgcFlightPhase', { name: PFDVars.fmgcFlightPhase, type: VarUnit.Enum }],
    ['hasLoc', { name: PFDVars.hasLoc, type: VarUnit.Bool }],
    ['hasDme', { name: PFDVars.hasDme, type: VarUnit.Bool }],
    ['navIdent', { name: PFDVars.navIdent, type: VarUnit.String }],
    ['navFreq', { name: PFDVars.navFreq, type: VarUnit.MHz }],
    ['dme', { name: PFDVars.dme, type: VarUnit.NM }],
    ['navRadialError', { name: PFDVars.navRadialError, type: VarUnit.Degree }],
    ['hasGlideslope', { name: PFDVars.hasGlideslope, type: VarUnit.Bool }],
    ['glideSlopeError', { name: PFDVars.glideSlopeError, type: VarUnit.Degree }],
    ['markerBeacon', { name: PFDVars.markerBeacon, type: VarUnit.Enum }],
    ['mach', { name: PFDVars.mach, type: VarUnit.Number }],
    ['flapHandleIndex', { name: PFDVars.flapHandleIndex, type: VarUnit.Number }],
    ['groundTrack', { name: PFDVars.groundTrack, type: VarUnit.Number }],
    ['aoa', { name: PFDVars.aoa, type: VarUnit.Degree }],
    ['groundHeadingTrue', { name: PFDVars.groundHeadingTrue, type: VarUnit.Degree }],
    ['groundTrackTrue', { name: PFDVars.groundTrackTrue, type: VarUnit.Degree }],
    ['ilsCourse', { name: PFDVars.ilsCourse, type: VarUnit.Number }],
    ['tla1', { name: PFDVars.tla1, type: VarUnit.Number }],
    ['tla2', { name: PFDVars.tla2, type: VarUnit.Number }],
    ['tla3', { name: PFDVars.tla3, type: VarUnit.Number }],
    ['tla4', { name: PFDVars.tla4, type: VarUnit.Number }],
    ['tcasState', { name: PFDVars.tcasState, type: VarUnit.Enum }],
    ['tcasCorrective', { name: PFDVars.tcasCorrective, type: VarUnit.Bool }],
    ['tcasRedZoneL', { name: PFDVars.tcasRedZoneL, type: VarUnit.Number }],
    ['tcasRedZoneH', { name: PFDVars.tcasRedZoneH, type: VarUnit.Number }],
    ['tcasGreenZoneL', { name: PFDVars.tcasGreenZoneL, type: VarUnit.Number }],
    ['tcasGreenZoneH', { name: PFDVars.tcasGreenZoneH, type: VarUnit.Number }],
    ['tcasFail', { name: PFDVars.tcasFail, type: VarUnit.Bool }],
    ['engOneRunning', { name: PFDVars.engOneRunning, type: VarUnit.Bool }],
    ['engTwoRunning', { name: PFDVars.engTwoRunning, type: VarUnit.Bool }],
    ['engThreeRunning', { name: PFDVars.engThreeRunning, type: VarUnit.Bool }],
    ['engFourRunning', { name: PFDVars.engFourRunning, type: VarUnit.Bool }],
    ['setHoldSpeed', { name: PFDVars.setHoldSpeed, type: VarUnit.Bool }],
    ['tdReached', { name: PFDVars.tdReached, type: VarUnit.Bool }],
    ['groundSpeed', { name: PFDVars.groundSpeed, type: VarUnit.Number }],
    ['radioAltitude1', { name: PFDVars.radioAltitude1, type: VarUnit.Number }],
    ['radioAltitude2', { name: PFDVars.radioAltitude2, type: VarUnit.Number }],
    ['radioAltitude3', { name: PFDVars.radioAltitude3, type: VarUnit.Number }],
    ['beta', { name: PFDVars.beta, type: VarUnit.Degree }],
    ['betaTargetActive', { name: PFDVars.betaTargetActive, type: VarUnit.Number }],
    ['betaTarget', { name: PFDVars.betaTarget, type: VarUnit.Number }],
    ['latAcc', { name: PFDVars.latAcc, type: 'G Force' as VarUnit }],
    ['flexTemp', { name: PFDVars.flexTemp, type: VarUnit.Number }],
    ['autoBrakeMode', { name: PFDVars.autoBrakeMode, type: VarUnit.Number }],
    ['autoBrakeActive', { name: PFDVars.autoBrakeActive, type: VarUnit.Bool }],
    ['autoBrakeDecel', { name: PFDVars.autoBrakeDecel, type: VarUnit.Bool }],
    ['fpaRaw', { name: PFDVars.fpaRaw, type: VarUnit.Number }],
    ['daRaw', { name: PFDVars.daRaw, type: VarUnit.Number }],
    ['xtk', { name: PFDVars.xtk, type: VarUnit.NM }],
    ['ldevRequestLeft', { name: PFDVars.ldevLeft, type: VarUnit.Bool }],
    ['ldevRequestRight', { name: PFDVars.ldevRight, type: VarUnit.Bool }],
    ['landingElevation1Raw', { name: PFDVars.landingElevation1Raw, type: VarUnit.Number }],
    ['landingElevation2Raw', { name: PFDVars.landingElevation2Raw, type: VarUnit.Number }],
    ['fm1NavDiscrete', { name: PFDVars.fm1NavDiscrete, type: VarUnit.Number }],
    ['fm1EisDiscrete2Raw', { name: PFDVars.fm1EisDiscrete2, type: VarUnit.Number }],
    ['fm2EisDiscrete2Raw', { name: PFDVars.fm2EisDiscrete2, type: VarUnit.Number }],
    ['fm1MdaRaw', { name: PFDVars.fm1MdaRaw, type: VarUnit.Number }],
    ['fm2MdaRaw', { name: PFDVars.fm2MdaRaw, type: VarUnit.Number }],
    ['fm1DhRaw', { name: PFDVars.fm1DhRaw, type: VarUnit.Number }],
    ['fm2DhRaw', { name: PFDVars.fm2DhRaw, type: VarUnit.Number }],
    ['fm1HealthyDiscrete', { name: PFDVars.fm1HealthyDiscrete, type: VarUnit.Number }],
    ['fm2HealthyDiscrete', { name: PFDVars.fm2HealthyDiscrete, type: VarUnit.Number }],
    ['fm1TransAltRaw', { name: PFDVars.fm1TransAltRaw, type: VarUnit.Number }],
    ['fm2TransAltRaw', { name: PFDVars.fm2TransAltRaw, type: VarUnit.Number }],
    ['fm1TransLvlRaw', { name: PFDVars.fm1TransLvlRaw, type: VarUnit.Number }],
    ['fm2TransLvlRaw', { name: PFDVars.fm2TransLvlRaw, type: VarUnit.Number }],
    ['lgciuDiscreteWord1Raw', { name: PFDVars.lgciuDiscreteWord1Raw, type: VarUnit.Number }],
    ['slatPosLeft', { name: PFDVars.slatPosLeft, type: VarUnit.Number }],
    ['trimPosition', { name: PFDVars.trimPosition, type: VarUnit.Number }],
    ['spoilersCommanded', { name: PFDVars.spoilersCommanded, type: VarUnit.Number }],
    ['spoilersArmed', { name: PFDVars.spoilersArmed, type: VarUnit.Bool }],
    ['btvExitMissed', { name: PFDVars.btvExitMissed, type: VarUnit.Bool }],
    ['hydGreenSysPressurized', { name: PFDVars.hydGreenSysPressurized, type: VarUnit.Bool }],
    ['hydYellowSysPressurized', { name: PFDVars.hydYellowSysPressurized, type: VarUnit.Bool }],
  ]);

  public constructor(bus: Bus) {
    super(PFDSimvarPublisher.simvars, bus);
  }
}
