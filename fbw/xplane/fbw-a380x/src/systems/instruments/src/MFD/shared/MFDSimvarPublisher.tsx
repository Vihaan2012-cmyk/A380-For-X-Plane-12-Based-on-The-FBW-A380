import { EfisSide } from '@flybywiresim/fbw-sdk';
import { Bus, VarSpec, VarPublisher, VarUnit } from 'xplane-js/ui';

export type MfdSimvars = {
  coldDark: number;
  elec: boolean;
  elecFo: boolean;
  potentiometerCaptain: number;
  potentiometerFo: number;
  fmsDataKnob: number;
  flightPhase: number;
  flexTemp: number;
  adirs1MaintWord: number;
  adirs2MaintWord: number;
  adirs3MaintWord: number;
  leftMfdInView: boolean;
  rightMfdInView: boolean;
  kccuOnL: boolean;
  kccuOnR: boolean;
  xpdrAvail: boolean;
  xpdrCode: number;
  xpdrState: number;
  tcasFail: boolean;
  gpwsTerrOff: boolean;
  gpwsSysOff: boolean;
  gpwsGsInhibit: boolean;
  gpwsFlapsInhibit: boolean;
  dcBusEss: boolean;
  dcBus1: boolean;
  dcBus2: boolean;
  fmcAIsHealthy: boolean;
  fmcBIsHealthy: boolean;
  fmcCIsHealthy: boolean;
  fmsCaptFailed: boolean;
  fmsFoFailed: boolean;
  wxrTawsSysSelected: number;
  terr1Failed: boolean;
  terr2Failed: boolean;
  gpws1Failed: boolean;
  gpws2Failed: boolean;
  wxr1Failed: boolean;
  wxr2Failed: boolean;
  wxrOff: boolean;
  wxrPwsOff: boolean;
  wxrTurbOff: boolean;
  baroStdL: boolean;
  baroStdR: boolean;
  wxrElevnTiltL: number;
  wxrElevnFtL: number;
  wxrTiltDegL: number;
  wxrGainManL: boolean;
  wxrGainPctL: number;
  wxrModeMapL: boolean;
  wxrVdOffL: boolean;
  wxrElevnTiltR: number;
  wxrElevnFtR: number;
  wxrTiltDegR: number;
  wxrGainManR: boolean;
  wxrGainPctR: number;
  wxrModeMapR: boolean;
  wxrVdOffR: boolean;
};

export type InternalKccuKeyEvent = {
  /** First element: Side on which KCCU was pressed, Second element: key string */
  kccuKeyEvent: [EfisSide, string];
};

export enum MfdVars {
  coldDark = 'L:A32NX_COLD_AND_DARK_SPAWN',
  elec = 'L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED',
  elecFo = 'L:A32NX_ELEC_AC_2_BUS_IS_POWERED',
  potentiometerCaptain = 'fbw/LIGHT_POTENTIOMETER_88',
  potentiometerFo = 'fbw/LIGHT_POTENTIOMETER_90',
  fmsDataKnob = 'L:A32NX_FMS_SWITCHING_KNOB',
  flightPhase = 'L:A32NX_FMGC_FLIGHT_PHASE',
  flexTemp = 'L:A32NX_AIRLINER_TO_FLEX_TEMP',
  adirs1MaintWord = 'L:A32NX_ADIRS_IR_1_MAINT_WORD',
  adirs2MaintWord = 'L:A32NX_ADIRS_IR_2_MAINT_WORD',
  adirs3MaintWord = 'L:A32NX_ADIRS_IR_3_MAINT_WORD',
  leftMfdInView = 'fbw/IS_CAMERA_RAY_INTERSECT_WITH_NODE_1',
  rightMfdInView = 'fbw/IS_CAMERA_RAY_INTERSECT_WITH_NODE_2',
  kccuOnL = 'L:A32NX_KCCU_L_KBD_ON_OFF',
  kccuOnR = 'L:A32NX_KCCU_R_KBD_ON_OFF',
  xpdrAvail = 'fbw/TRANSPONDER_AVAILABLE',
  xpdrCode = 'fbw/TRANSPONDER_CODE_1',
  xpdrState = 'fbw/TRANSPONDER_STATE_1',
  tcasFail = 'L:A32NX_TCAS_FAULT',
  gpwsTerrOff = 'L:A32NX_GPWS_TERR_OFF',
  gpwsSysOff = 'L:A32NX_GPWS_SYS_OFF',
  gpwsGsInhibit = 'L:A32NX_GPWS_GS_OFF',
  gpwsFlapsInhibit = 'L:A32NX_GPWS_FLAPS_OFF',
  dcBusEss = 'L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED',
  dcBus1 = 'L:A32NX_ELEC_DC_1_BUS_IS_POWERED',
  dcBus2 = 'L:A32NX_ELEC_DC_2_BUS_IS_POWERED',
  fmcAIsHealthy = 'L:A32NX_FMC_A_IS_HEALTHY',
  fmcBIsHealthy = 'L:A32NX_FMC_B_IS_HEALTHY',
  fmcCIsHealthy = 'L:A32NX_FMC_C_IS_HEALTHY',
  fmsCaptFailed = 'L:A32NX_FMS_L_FAILED',
  fmsFoFailed = 'L:A32NX_FMS_R_FAILED',
  wxrTawsSysSelected = 'L:A32NX_WXR_TAWS_SYS_SELECTED',
  terr1Failed = 'L:A32NX_TERR_1_FAILED',
  terr2Failed = 'L:A32NX_TERR_2_FAILED',
  gpws1Failed = 'L:A32NX_GPWS_1_FAILED',
  gpws2Failed = 'L:A32NX_GPWS_2_FAILED',
  wxr1Failed = 'L:A32NX_WXR_1_FAILED',
  wxr2Failed = 'L:A32NX_WXR_2_FAILED',
  wxrOff = 'L:A380X_WXR_OFF',
  wxrPwsOff = 'L:A380X_WXR_PWS_OFF',
  wxrTurbOff = 'L:A380X_WXR_TURB_OFF',
  baroStdL = 'L:A32NX_FCU_EFIS_L_DISPLAY_BARO_IS_STD',
  baroStdR = 'L:A32NX_FCU_EFIS_R_DISPLAY_BARO_IS_STD',
  wxrElevnTiltL = 'L:A380X_WXR_L_ELEVN_TILT',
  wxrElevnFtL = 'L:A380X_WXR_L_ELEVN_FT',
  wxrTiltDegL = 'L:A380X_WXR_L_TILT_DEG',
  wxrGainManL = 'L:A380X_WXR_L_GAIN_MAN',
  wxrGainPctL = 'L:A380X_WXR_L_GAIN_PCT',
  wxrModeMapL = 'L:A380X_WXR_L_MODE_MAP',
  wxrVdOffL = 'L:A380X_WXR_L_VD_OFF',
  wxrElevnTiltR = 'L:A380X_WXR_R_ELEVN_TILT',
  wxrElevnFtR = 'L:A380X_WXR_R_ELEVN_FT',
  wxrTiltDegR = 'L:A380X_WXR_R_TILT_DEG',
  wxrGainManR = 'L:A380X_WXR_R_GAIN_MAN',
  wxrGainPctR = 'L:A380X_WXR_R_GAIN_PCT',
  wxrModeMapR = 'L:A380X_WXR_R_MODE_MAP',
  wxrVdOffR = 'L:A380X_WXR_R_VD_OFF',
}

/** A publisher to poll and publish nav/com simvars. */
export class MfdSimvarPublisher extends VarPublisher<MfdSimvars> {
  private static simvars = new Map<keyof MfdSimvars, VarSpec>([
    ['elec', { name: MfdVars.elec, type: VarUnit.Bool }],
    ['elecFo', { name: MfdVars.elecFo, type: VarUnit.Bool }],
    ['potentiometerCaptain', { name: MfdVars.potentiometerCaptain, type: VarUnit.Number }],
    ['potentiometerFo', { name: MfdVars.potentiometerFo, type: VarUnit.Number }],
    ['fmsDataKnob', { name: MfdVars.fmsDataKnob, type: VarUnit.Enum }],
    ['flightPhase', { name: MfdVars.flightPhase, type: VarUnit.Enum }],
    ['flexTemp', { name: MfdVars.flexTemp, type: VarUnit.Number }],
    ['adirs1MaintWord', { name: MfdVars.adirs1MaintWord, type: VarUnit.Number }],
    ['adirs2MaintWord', { name: MfdVars.adirs2MaintWord, type: VarUnit.Number }],
    ['adirs3MaintWord', { name: MfdVars.adirs3MaintWord, type: VarUnit.Number }],
    ['leftMfdInView', { name: MfdVars.leftMfdInView, type: VarUnit.Bool }],
    ['rightMfdInView', { name: MfdVars.rightMfdInView, type: VarUnit.Bool }],
    ['kccuOnL', { name: MfdVars.kccuOnL, type: VarUnit.Bool }],
    ['kccuOnR', { name: MfdVars.kccuOnR, type: VarUnit.Bool }],
    ['xpdrAvail', { name: MfdVars.xpdrAvail, type: VarUnit.Bool }],
    ['xpdrCode', { name: MfdVars.xpdrCode, type: VarUnit.Number }],
    ['xpdrState', { name: MfdVars.xpdrState, type: VarUnit.Enum }],
    ['tcasFail', { name: MfdVars.tcasFail, type: VarUnit.Bool }],
    ['gpwsTerrOff', { name: MfdVars.gpwsTerrOff, type: VarUnit.Bool }],
    ['gpwsSysOff', { name: MfdVars.gpwsSysOff, type: VarUnit.Bool }],
    ['gpwsGsInhibit', { name: MfdVars.gpwsGsInhibit, type: VarUnit.Bool }],
    ['gpwsFlapsInhibit', { name: MfdVars.gpwsFlapsInhibit, type: VarUnit.Bool }],
    ['dcBusEss', { name: MfdVars.dcBusEss, type: VarUnit.Bool }],
    ['dcBus1', { name: MfdVars.dcBus1, type: VarUnit.Bool }],
    ['dcBus2', { name: MfdVars.dcBus2, type: VarUnit.Bool }],
    ['fmcAIsHealthy', { name: MfdVars.fmcAIsHealthy, type: VarUnit.Bool }],
    ['fmcBIsHealthy', { name: MfdVars.fmcBIsHealthy, type: VarUnit.Bool }],
    ['fmcCIsHealthy', { name: MfdVars.fmcCIsHealthy, type: VarUnit.Bool }],
    ['fmsCaptFailed', { name: MfdVars.fmsCaptFailed, type: VarUnit.Bool }],
    ['fmsFoFailed', { name: MfdVars.fmsFoFailed, type: VarUnit.Bool }],
    ['wxrTawsSysSelected', { name: MfdVars.wxrTawsSysSelected, type: VarUnit.Number }],
    ['terr1Failed', { name: MfdVars.terr1Failed, type: VarUnit.Bool }],
    ['terr2Failed', { name: MfdVars.terr2Failed, type: VarUnit.Bool }],
    ['gpws1Failed', { name: MfdVars.gpws1Failed, type: VarUnit.Bool }],
    ['gpws2Failed', { name: MfdVars.gpws2Failed, type: VarUnit.Bool }],
    ['wxr1Failed', { name: MfdVars.wxr1Failed, type: VarUnit.Bool }],
    ['wxr2Failed', { name: MfdVars.wxr2Failed, type: VarUnit.Bool }],
    ['wxrOff', { name: MfdVars.wxrOff, type: VarUnit.Bool }],
    ['wxrPwsOff', { name: MfdVars.wxrPwsOff, type: VarUnit.Bool }],
    ['wxrTurbOff', { name: MfdVars.wxrTurbOff, type: VarUnit.Bool }],
    ['baroStdL', { name: MfdVars.baroStdL, type: VarUnit.Bool }],
    ['baroStdR', { name: MfdVars.baroStdR, type: VarUnit.Bool }],
    ['wxrElevnTiltL', { name: MfdVars.wxrElevnTiltL, type: VarUnit.Number }],
    ['wxrElevnFtL', { name: MfdVars.wxrElevnFtL, type: VarUnit.Number }],
    ['wxrTiltDegL', { name: MfdVars.wxrTiltDegL, type: VarUnit.Number }],
    ['wxrGainManL', { name: MfdVars.wxrGainManL, type: VarUnit.Bool }],
    ['wxrGainPctL', { name: MfdVars.wxrGainPctL, type: VarUnit.Number }],
    ['wxrModeMapL', { name: MfdVars.wxrModeMapL, type: VarUnit.Bool }],
    ['wxrVdOffL', { name: MfdVars.wxrVdOffL, type: VarUnit.Bool }],
    ['wxrElevnTiltR', { name: MfdVars.wxrElevnTiltR, type: VarUnit.Number }],
    ['wxrElevnFtR', { name: MfdVars.wxrElevnFtR, type: VarUnit.Number }],
    ['wxrTiltDegR', { name: MfdVars.wxrTiltDegR, type: VarUnit.Number }],
    ['wxrGainManR', { name: MfdVars.wxrGainManR, type: VarUnit.Bool }],
    ['wxrGainPctR', { name: MfdVars.wxrGainPctR, type: VarUnit.Number }],
    ['wxrModeMapR', { name: MfdVars.wxrModeMapR, type: VarUnit.Bool }],
    ['wxrVdOffR', { name: MfdVars.wxrVdOffR, type: VarUnit.Bool }],
  ]);

  public constructor(bus: Bus) {
    super(MfdSimvarPublisher.simvars, bus);
  }
}
