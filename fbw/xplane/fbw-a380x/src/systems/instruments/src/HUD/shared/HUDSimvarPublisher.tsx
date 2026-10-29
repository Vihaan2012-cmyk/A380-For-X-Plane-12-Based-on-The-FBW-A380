import { TopicFeeder, Bus, VarSpec, VarUnit } from 'xplane-js/ui';
import { UpdatableSimVarPublisher } from '../../MsfsAvionicsCommon/UpdatableSimVarPublisher';
import { HUDSyntheticRunway, GenericDataListenerSync } from '@flybywiresim/fbw-sdk';

export interface HUDSimvars {
  hudLPos: number;
  hudRPos: number;
  rev2: number;
  rev3: number;
  rev2Pos: number;
  rev3Pos: number;
  eng2State: number;
  eng3State: number;
  brakePedalInputLeft: number;
  brakePedalInputRight: number;
  throttle2Position: number;
  throttle3Position: number;
  hudMode: number;
  windDirection: number;
  windSpeed: number;
  trueHeadingRaw: number;
  apVsSelected: number;
  crosswindModeL: boolean;
  declutterModeL: number;
  crosswindModeR: boolean;
  declutterModeR: number;
  slatsFlapsStatusRaw: number;
  slatsPositionRaw: number;
  flapsPositionRaw: number;
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
  cgPercent: number;
  spoilersCommanded: number;
  spoilersArmed: boolean;
  btvExitMissed: boolean;
  hydGreenSysPressurized: boolean;
  hydYellowSysPressurized: boolean;
}

export enum HUDVars {
  hudLPos = 'L:A320_Neo_HUD_L_POS',
  hudRPos = 'L:A320_Neo_HUD_R_POS',
  eng2State = 'L:A32NX_ENGINE_STATE:2',
  eng3State = 'L:A32NX_ENGINE_STATE:3',
  rev2 = 'L:A32NX_AUTOTHRUST_REVERSE:2',
  rev3 = 'L:A32NX_AUTOTHRUST_REVERSE:3',
  rev2Pos = 'L:A32NX_REVERSER_2_POSITION',
  rev3Pos = 'L:A32NX_REVERSER_3_POSITION',
  brakePedalInputLeft = 'L:A32NX_LEFT_BRAKE_PEDAL_INPUT',
  brakePedalInputRight = 'L:A32NX_RIGHT_BRAKE_PEDAL_INPUT',
  throttle2Position = 'L:XMLVAR_Throttle2Position',
  throttle3Position = 'L:XMLVAR_Throttle3Position',
  hudMode = 'L:A380X_HUDMODE',
  windDirection = 'L:A32NX_ADIRS_IR_1_WIND_DIRECTION',
  windSpeed = 'L:A32NX_ADIRS_IR_1_WIND_SPEED',
  trueHeadingRaw = 'L:A32NX_ADIRS_IR_1_TRUE_HEADING',
  apVsSelected = 'L:A32NX_AUTOPILOT_VS_SELECTED',
  crosswindModeL = 'L:A380X_HUD_L_CROSSWIND_MODE',
  declutterModeL = 'L:A380X_HUD_L_DECLUTTER_MODE',
  crosswindModeR = 'L:A380X_HUD_R_CROSSWIND_MODE',
  declutterModeR = 'L:A380X_HUD_R_DECLUTTER_MODE',
  slatsFlapsStatusRaw = 'L:A32NX_SFCC_SLAT_FLAP_SYSTEM_STATUS_WORD',
  slatsPositionRaw = 'L:A32NX_SFCC_SLAT_ACTUAL_POSITION_WORD',
  flapsPositionRaw = 'L:A32NX_SFCC_FLAP_ACTUAL_POSITION_WORD',
  coldDark = 'L:A32NX_COLD_AND_DARK_SPAWN',
  elec = 'L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED',
  elecFo = 'L:A32NX_ELEC_AC_2_BUS_IS_POWERED',
  potentiometerCaptain = 'fbw/LIGHT_POTENTIOMETER_71',
  potentiometerFo = 'fbw/LIGHT_POTENTIOMETER_72',
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
  fm2DhRaw = 'L:A32NX_FM2_DECISION_HEIGHT',
  fm1HealthyDiscrete = 'L:A32NX_FM1_HEALTHY_DISCRETE',
  fm2HealthyDiscrete = 'L:A32NX_FM2_HEALTHY_DISCRETE',
  fm1TransAltRaw = 'L:A32NX_FM1_TRANS_ALT',
  fm2TransAltRaw = 'L:A32NX_FM2_TRANS_ALT',
  fm1TransLvlRaw = 'L:A32NX_FM1_TRANS_LVL',
  fm2TransLvlRaw = 'L:A32NX_FM2_TRANS_LVL',
  lgciuDiscreteWord1Raw = 'L:A32NX_LGCIU_1_DISCRETE_WORD_1',
  slatPosLeft = 'L:A32NX_LEFT_SLATS_ANGLE',
  cgPercent = 'L:A32NX_AIRFRAME_GW_CG_PERCENT_MAC',
  spoilersCommanded = 'L:A32NX_LEFT_SPOILER_1_COMMANDED_POSITION',
  spoilersArmed = 'L:A32NX_SPOILERS_ARMED',
  btvExitMissed = 'L:A32NX_BTV_EXIT_MISSED',
  hydGreenSysPressurized = 'L:A32NX_HYD_GREEN_SYSTEM_1_SECTION_PRESSURE_SWITCH',
  hydYellowSysPressurized = 'L:A32NX_HYD_YELLOW_SYSTEM_1_SECTION_PRESSURE_SWITCH',
}

/** A publisher to poll and publish nav/com simvars. */
export class HUDSimvarPublisher extends UpdatableSimVarPublisher<HUDSimvars> {
  private static simvars = new Map<keyof HUDSimvars, VarSpec>([
    ['hudLPos', { name: HUDVars.hudLPos, type: VarUnit.Number }],
    ['hudRPos', { name: HUDVars.hudRPos, type: VarUnit.Number }],
    ['eng2State', { name: HUDVars.eng2State, type: VarUnit.Number }],
    ['eng3State', { name: HUDVars.eng3State, type: VarUnit.Number }],
    ['rev2', { name: HUDVars.rev2, type: VarUnit.Number }],
    ['rev3', { name: HUDVars.rev3, type: VarUnit.Number }],
    ['rev2Pos', { name: HUDVars.rev2Pos, type: VarUnit.Number }],
    ['rev3Pos', { name: HUDVars.rev3Pos, type: VarUnit.Number }],
    ['brakePedalInputLeft', { name: HUDVars.brakePedalInputLeft, type: VarUnit.Number }],
    ['brakePedalInputRight', { name: HUDVars.brakePedalInputRight, type: VarUnit.Number }],
    ['throttle2Position', { name: HUDVars.throttle2Position, type: VarUnit.Number }],
    ['throttle3Position', { name: HUDVars.throttle3Position, type: VarUnit.Number }],
    ['hudMode', { name: HUDVars.hudMode, type: VarUnit.Number }],
    ['windDirection', { name: HUDVars.windDirection, type: VarUnit.Number }],
    ['windSpeed', { name: HUDVars.windSpeed, type: VarUnit.Number }],
    ['trueHeadingRaw', { name: HUDVars.trueHeadingRaw, type: VarUnit.Number }],
    ['apVsSelected', { name: HUDVars.apVsSelected, type: VarUnit.FPM }],
    ['crosswindModeL', { name: HUDVars.crosswindModeL, type: VarUnit.Bool }],
    ['declutterModeL', { name: HUDVars.declutterModeL, type: VarUnit.Number }],
    ['crosswindModeR', { name: HUDVars.crosswindModeR, type: VarUnit.Bool }],
    ['declutterModeR', { name: HUDVars.declutterModeR, type: VarUnit.Number }],
    ['slatsFlapsStatusRaw', { name: HUDVars.slatsFlapsStatusRaw, type: VarUnit.Number }],
    ['slatsPositionRaw', { name: HUDVars.slatsPositionRaw, type: VarUnit.Number }],
    ['flapsPositionRaw', { name: HUDVars.flapsPositionRaw, type: VarUnit.Number }],
    ['elec', { name: HUDVars.elec, type: VarUnit.Bool }],
    ['elecFo', { name: HUDVars.elecFo, type: VarUnit.Bool }],
    ['potentiometerCaptain', { name: HUDVars.potentiometerCaptain, type: VarUnit.Number }],
    ['potentiometerFo', { name: HUDVars.potentiometerFo, type: VarUnit.Number }],
    ['pitch', { name: HUDVars.pitch, type: VarUnit.Number }],
    ['roll', { name: HUDVars.roll, type: VarUnit.Number }],
    ['heading', { name: HUDVars.heading, type: VarUnit.Number }],
    ['baroCorrectedAltitude1', { name: HUDVars.baroCorrectedAltitude1, type: VarUnit.Number }],
    ['baroCorrectedAltitude2', { name: HUDVars.baroCorrectedAltitude1, type: VarUnit.Number }],
    ['pressureAltitude', { name: HUDVars.pressureAltitude, type: VarUnit.Number }],
    ['speed', { name: HUDVars.speed, type: VarUnit.Number }],
    ['staticPressureRaw', { name: HUDVars.staticPressureRaw, type: VarUnit.Number }],
    ['noseGearCompressed', { name: HUDVars.noseGearCompressed, type: VarUnit.Bool }],
    ['leftMainGearCompressed', { name: HUDVars.leftMainGearCompressed, type: VarUnit.Bool }],
    ['rightMainGearCompressed', { name: HUDVars.rightMainGearCompressed, type: VarUnit.Bool }],
    ['mda', { name: HUDVars.mda, type: VarUnit.Feet }],
    ['dh', { name: HUDVars.dh, type: VarUnit.Feet }],
    ['attHdgKnob', { name: HUDVars.attHdgKnob, type: VarUnit.Enum }],
    ['airKnob', { name: HUDVars.airKnob, type: VarUnit.Enum }],
    ['vsBaro', { name: HUDVars.vsBaro, type: VarUnit.Number }],
    ['vsInert', { name: HUDVars.vsInert, type: VarUnit.Number }],
    ['sideStickX', { name: HUDVars.sideStickX, type: VarUnit.Number }],
    ['sideStickY', { name: HUDVars.sideStickY, type: VarUnit.Number }],
    ['v1', { name: HUDVars.v1, type: VarUnit.Knots }],
    ['vr', { name: HUDVars.vr, type: VarUnit.Knots }],
    ['fwcFlightPhase', { name: HUDVars.fwcFlightPhase, type: VarUnit.Number }],
    ['fmgcFlightPhase', { name: HUDVars.fmgcFlightPhase, type: VarUnit.Enum }],
    ['hasLoc', { name: HUDVars.hasLoc, type: VarUnit.Bool }],
    ['hasDme', { name: HUDVars.hasDme, type: VarUnit.Bool }],
    ['navIdent', { name: HUDVars.navIdent, type: VarUnit.String }],
    ['navFreq', { name: HUDVars.navFreq, type: VarUnit.MHz }],
    ['dme', { name: HUDVars.dme, type: VarUnit.NM }],
    ['navRadialError', { name: HUDVars.navRadialError, type: VarUnit.Degree }],
    ['hasGlideslope', { name: HUDVars.hasGlideslope, type: VarUnit.Bool }],
    ['glideSlopeError', { name: HUDVars.glideSlopeError, type: VarUnit.Degree }],
    ['markerBeacon', { name: HUDVars.markerBeacon, type: VarUnit.Enum }],
    ['mach', { name: HUDVars.mach, type: VarUnit.Number }],
    ['flapHandleIndex', { name: HUDVars.flapHandleIndex, type: VarUnit.Number }],
    ['groundTrack', { name: HUDVars.groundTrack, type: VarUnit.Number }],
    ['aoa', { name: HUDVars.aoa, type: VarUnit.Degree }],
    ['groundHeadingTrue', { name: HUDVars.groundHeadingTrue, type: VarUnit.Degree }],
    ['groundTrackTrue', { name: HUDVars.groundTrackTrue, type: VarUnit.Degree }],
    ['ilsCourse', { name: HUDVars.ilsCourse, type: VarUnit.Number }],
    ['tla1', { name: HUDVars.tla1, type: VarUnit.Number }],
    ['tla2', { name: HUDVars.tla2, type: VarUnit.Number }],
    ['tla3', { name: HUDVars.tla3, type: VarUnit.Number }],
    ['tla4', { name: HUDVars.tla4, type: VarUnit.Number }],
    ['tcasState', { name: HUDVars.tcasState, type: VarUnit.Enum }],
    ['tcasCorrective', { name: HUDVars.tcasCorrective, type: VarUnit.Bool }],
    ['tcasRedZoneL', { name: HUDVars.tcasRedZoneL, type: VarUnit.Number }],
    ['tcasRedZoneH', { name: HUDVars.tcasRedZoneH, type: VarUnit.Number }],
    ['tcasGreenZoneL', { name: HUDVars.tcasGreenZoneL, type: VarUnit.Number }],
    ['tcasGreenZoneH', { name: HUDVars.tcasGreenZoneH, type: VarUnit.Number }],
    ['tcasFail', { name: HUDVars.tcasFail, type: VarUnit.Bool }],
    ['engOneRunning', { name: HUDVars.engOneRunning, type: VarUnit.Bool }],
    ['engTwoRunning', { name: HUDVars.engTwoRunning, type: VarUnit.Bool }],
    ['engThreeRunning', { name: HUDVars.engThreeRunning, type: VarUnit.Bool }],
    ['engFourRunning', { name: HUDVars.engFourRunning, type: VarUnit.Bool }],
    ['setHoldSpeed', { name: HUDVars.setHoldSpeed, type: VarUnit.Bool }],
    ['tdReached', { name: HUDVars.tdReached, type: VarUnit.Bool }],
    ['groundSpeed', { name: HUDVars.groundSpeed, type: VarUnit.Number }],
    ['radioAltitude1', { name: HUDVars.radioAltitude1, type: VarUnit.Number }],
    ['radioAltitude2', { name: HUDVars.radioAltitude2, type: VarUnit.Number }],
    ['radioAltitude3', { name: HUDVars.radioAltitude3, type: VarUnit.Number }],
    ['beta', { name: HUDVars.beta, type: VarUnit.Degree }],
    ['betaTargetActive', { name: HUDVars.betaTargetActive, type: VarUnit.Number }],
    ['betaTarget', { name: HUDVars.betaTarget, type: VarUnit.Number }],
    ['latAcc', { name: HUDVars.latAcc, type: 'G Force' as VarUnit }],
    ['flexTemp', { name: HUDVars.flexTemp, type: VarUnit.Number }],
    ['autoBrakeMode', { name: HUDVars.autoBrakeMode, type: VarUnit.Number }],
    ['autoBrakeActive', { name: HUDVars.autoBrakeActive, type: VarUnit.Bool }],
    ['autoBrakeDecel', { name: HUDVars.autoBrakeDecel, type: VarUnit.Bool }],
    ['fpaRaw', { name: HUDVars.fpaRaw, type: VarUnit.Number }],
    ['daRaw', { name: HUDVars.daRaw, type: VarUnit.Number }],
    ['xtk', { name: HUDVars.xtk, type: VarUnit.NM }],
    ['ldevRequestLeft', { name: HUDVars.ldevLeft, type: VarUnit.Bool }],
    ['ldevRequestRight', { name: HUDVars.ldevRight, type: VarUnit.Bool }],
    ['landingElevation1Raw', { name: HUDVars.landingElevation1Raw, type: VarUnit.Number }],
    ['landingElevation2Raw', { name: HUDVars.landingElevation2Raw, type: VarUnit.Number }],
    ['fm1NavDiscrete', { name: HUDVars.fm1NavDiscrete, type: VarUnit.Number }],
    ['fm1EisDiscrete2Raw', { name: HUDVars.fm1EisDiscrete2, type: VarUnit.Number }],
    ['fm2EisDiscrete2Raw', { name: HUDVars.fm2EisDiscrete2, type: VarUnit.Number }],
    ['fm1MdaRaw', { name: HUDVars.fm1MdaRaw, type: VarUnit.Number }],
    ['fm2MdaRaw', { name: HUDVars.fm2MdaRaw, type: VarUnit.Number }],
    ['fm1DhRaw', { name: HUDVars.fm1DhRaw, type: VarUnit.Number }],
    ['fm2DhRaw', { name: HUDVars.fm2DhRaw, type: VarUnit.Number }],
    ['fm1HealthyDiscrete', { name: HUDVars.fm1HealthyDiscrete, type: VarUnit.Number }],
    ['fm2HealthyDiscrete', { name: HUDVars.fm2HealthyDiscrete, type: VarUnit.Number }],
    ['fm1TransAltRaw', { name: HUDVars.fm1TransAltRaw, type: VarUnit.Number }],
    ['fm2TransAltRaw', { name: HUDVars.fm2TransAltRaw, type: VarUnit.Number }],
    ['fm1TransLvlRaw', { name: HUDVars.fm1TransLvlRaw, type: VarUnit.Number }],
    ['fm2TransLvlRaw', { name: HUDVars.fm2TransLvlRaw, type: VarUnit.Number }],
    ['lgciuDiscreteWord1Raw', { name: HUDVars.lgciuDiscreteWord1Raw, type: VarUnit.Number }],
    ['slatPosLeft', { name: HUDVars.slatPosLeft, type: VarUnit.Number }],
    ['cgPercent', { name: HUDVars.cgPercent, type: VarUnit.Number }],
    ['spoilersCommanded', { name: HUDVars.spoilersCommanded, type: VarUnit.Number }],
    ['spoilersArmed', { name: HUDVars.spoilersArmed, type: VarUnit.Bool }],
    ['btvExitMissed', { name: HUDVars.btvExitMissed, type: VarUnit.Bool }],
    ['hydGreenSysPressurized', { name: HUDVars.hydGreenSysPressurized, type: VarUnit.Bool }],
    ['hydYellowSysPressurized', { name: HUDVars.hydYellowSysPressurized, type: VarUnit.Bool }],
  ]);

  public constructor(bus: Bus) {
    super(HUDSimvarPublisher.simvars, bus);
  }
}

export interface HUDSymbolData {
  symbol: HUDSyntheticRunway;
}

export class HUDSymbolsPublisher extends TopicFeeder<HUDSymbolData> {
  private readonly events: GenericDataListenerSync[] = [];

  constructor(bus: Bus) {
    super(bus);

    this.events.push(
      new GenericDataListenerSync((ev, data) => {
        this.postTopic('symbol', data);
      }, 'A380X_EFIS_HUD_SYMBOLS'),
    );
  }
}
