// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { Bus, VarSpec, VarUnit } from 'xplane-js/ui';
import {
  AdirsSimVarDefinitions,
  AdirsSimVars,
  SwitchingPanelSimVarsDefinitions,
  SwitchingPanelVSimVars,
} from '../MsfsAvionicsCommon/SimVarTypes';
import { UpdatableSimVarPublisher } from '../MsfsAvionicsCommon/UpdatableSimVarPublisher';

export type NDSimvars = AdirsSimVars &
  SwitchingPanelVSimVars & {
    elec: boolean;
    elecFo: boolean;
    potentiometerCaptain: number;
    potentiometerFo: number;
    ilsCourse: number;
    selectedHeading: Degrees;
    showSelectedHeading: boolean;
    absoluteTime: Seconds;
    kccuOnL: boolean;
    kccuOnR: boolean;
    wxr1Failed: boolean;
    wxr2Failed: boolean;
    wxrTawsSysSelected: number;
    wxrOff: boolean;
    wxrOverlayL: number;
    wxrElevnTiltL: number;
    wxrElevnFtL: number;
    wxrTiltDegL: number;
    wxrGainManL: boolean;
    wxrGainPctL: number;
    wxrModeMapL: boolean;
    wxrBaroStdL: boolean;
    wxrOverlayR: number;
    wxrElevnTiltR: number;
    wxrElevnFtR: number;
    wxrTiltDegR: number;
    wxrGainManR: boolean;
    wxrGainPctR: number;
    wxrModeMapR: boolean;
    wxrBaroStdR: boolean;
  };

export enum NDVars {
  elec = 'L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED',
  elecFo = 'L:A32NX_ELEC_AC_2_BUS_IS_POWERED',
  potentiometerCaptain = 'fbw/LIGHT_POTENTIOMETER_89',
  potentiometerFo = 'fbw/LIGHT_POTENTIOMETER_91',
  ilsCourse = 'L:A32NX_FM_LS_COURSE',
  selectedHeading = 'L:A32NX_FCU_HEADING_SELECTED',
  showSelectedHeading = 'L:A320_FCU_SHOW_SELECTED_HEADING',
  absoluteTime = 'E:fbw/ABSOLUTE_TIME',
  kccuOnL = 'L:A32NX_KCCU_L_KBD_ON_OFF',
  kccuOnR = 'L:A32NX_KCCU_R_KBD_ON_OFF',
  wxr1Failed = 'L:A32NX_WXR_1_FAILED',
  wxr2Failed = 'L:A32NX_WXR_2_FAILED',
  wxrTawsSysSelected = 'L:A32NX_WXR_TAWS_SYS_SELECTED',
  wxrOff = 'L:A380X_WXR_OFF',
  wxrOverlayL = 'L:A380X_EFIS_L_ACTIVE_OVERLAY',
  wxrElevnTiltL = 'L:A380X_WXR_L_ELEVN_TILT',
  wxrElevnFtL = 'L:A380X_WXR_L_ELEVN_FT',
  wxrTiltDegL = 'L:A380X_WXR_L_TILT_DEG',
  wxrGainManL = 'L:A380X_WXR_L_GAIN_MAN',
  wxrGainPctL = 'L:A380X_WXR_L_GAIN_PCT',
  wxrModeMapL = 'L:A380X_WXR_L_MODE_MAP',
  wxrBaroStdL = 'L:A32NX_FCU_EFIS_L_DISPLAY_BARO_IS_STD',
  wxrOverlayR = 'L:A380X_EFIS_R_ACTIVE_OVERLAY',
  wxrElevnTiltR = 'L:A380X_WXR_R_ELEVN_TILT',
  wxrElevnFtR = 'L:A380X_WXR_R_ELEVN_FT',
  wxrTiltDegR = 'L:A380X_WXR_R_TILT_DEG',
  wxrGainManR = 'L:A380X_WXR_R_GAIN_MAN',
  wxrGainPctR = 'L:A380X_WXR_R_GAIN_PCT',
  wxrModeMapR = 'L:A380X_WXR_R_MODE_MAP',
  wxrBaroStdR = 'L:A32NX_FCU_EFIS_R_DISPLAY_BARO_IS_STD',
}

/** A publisher to poll and publish nav/com simvars. */
export class NDSimvarPublisher extends UpdatableSimVarPublisher<NDSimvars> {
  private static simvars = new Map<keyof NDSimvars, VarSpec>([
    ...AdirsSimVarDefinitions,
    ...SwitchingPanelSimVarsDefinitions,
    ['elec', { name: NDVars.elec, type: VarUnit.Bool }],
    ['elecFo', { name: NDVars.elecFo, type: VarUnit.Bool }],
    ['potentiometerCaptain', { name: NDVars.potentiometerCaptain, type: VarUnit.Number }],
    ['potentiometerFo', { name: NDVars.potentiometerFo, type: VarUnit.Number }],
    ['ilsCourse', { name: NDVars.ilsCourse, type: VarUnit.Number }],
    ['selectedHeading', { name: NDVars.selectedHeading, type: VarUnit.Degree }],
    ['showSelectedHeading', { name: NDVars.showSelectedHeading, type: VarUnit.Bool }],
    ['absoluteTime', { name: NDVars.absoluteTime, type: VarUnit.Seconds }],
    ['kccuOnL', { name: NDVars.kccuOnL, type: VarUnit.Bool }],
    ['kccuOnR', { name: NDVars.kccuOnR, type: VarUnit.Bool }],
    ['wxr1Failed', { name: NDVars.wxr1Failed, type: VarUnit.Bool }],
    ['wxr2Failed', { name: NDVars.wxr2Failed, type: VarUnit.Bool }],
    ['wxrTawsSysSelected', { name: NDVars.wxrTawsSysSelected, type: VarUnit.Number }],
    ['wxrOff', { name: NDVars.wxrOff, type: VarUnit.Bool }],
    ['wxrOverlayL', { name: NDVars.wxrOverlayL, type: VarUnit.Number }],
    ['wxrElevnTiltL', { name: NDVars.wxrElevnTiltL, type: VarUnit.Number }],
    ['wxrElevnFtL', { name: NDVars.wxrElevnFtL, type: VarUnit.Number }],
    ['wxrTiltDegL', { name: NDVars.wxrTiltDegL, type: VarUnit.Number }],
    ['wxrGainManL', { name: NDVars.wxrGainManL, type: VarUnit.Bool }],
    ['wxrGainPctL', { name: NDVars.wxrGainPctL, type: VarUnit.Number }],
    ['wxrModeMapL', { name: NDVars.wxrModeMapL, type: VarUnit.Bool }],
    ['wxrBaroStdL', { name: NDVars.wxrBaroStdL, type: VarUnit.Bool }],
    ['wxrOverlayR', { name: NDVars.wxrOverlayR, type: VarUnit.Number }],
    ['wxrElevnTiltR', { name: NDVars.wxrElevnTiltR, type: VarUnit.Number }],
    ['wxrElevnFtR', { name: NDVars.wxrElevnFtR, type: VarUnit.Number }],
    ['wxrTiltDegR', { name: NDVars.wxrTiltDegR, type: VarUnit.Number }],
    ['wxrGainManR', { name: NDVars.wxrGainManR, type: VarUnit.Bool }],
    ['wxrGainPctR', { name: NDVars.wxrGainPctR, type: VarUnit.Number }],
    ['wxrModeMapR', { name: NDVars.wxrModeMapR, type: VarUnit.Bool }],
    ['wxrBaroStdR', { name: NDVars.wxrBaroStdR, type: VarUnit.Bool }],
  ]);

  public constructor(bus: Bus) {
    super(NDSimvarPublisher.simvars, bus);
  }
}
