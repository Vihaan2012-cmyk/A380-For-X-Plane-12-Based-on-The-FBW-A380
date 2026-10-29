// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { Bus, IndexedTopic, VarTopic, VarUnit } from 'xplane-js/ui';
import {
  AdirsSimVarDefinitions,
  AdirsSimVars,
  SwitchingPanelSimVarsDefinitions,
  SwitchingPanelVSimVars,
} from '../MsfsAvionicsCommon/SimVarTypes';
import { UpdatableSimVarPublisher } from '../MsfsAvionicsCommon/UpdatableSimVarPublisher';

// FIXME ideally migrate to singular publishers (split by source)
export type BaseSDSimvars = AdirsSimVars &
  SwitchingPanelVSimVars & {
    moreActive: boolean;
    sdPageToShow: number;
    sdStsPageToShow: number;
    zuluTime: number;
    /** in gallons */
    grossWeightCg: number;
    /** in pounds */
    fuelWeightPerGallon: number;
    cockpitCabinTemp: number;
    fwdCargoTemp: number;
    aftCargoTemp: number;
    condMainDeckTemp: number;
    condUpperDeckTemp: number;
    cabinAltitudeIsAuto: boolean;
    cabinVerticalSpeedIsAuto: boolean;
    pressDeltaPressure: boolean;
    manCabinAltitude: number;
    manCabinDeltaPressure: number;
    manCabinVerticalSpeed: number;
    pressCabinAltitude: number;
    pressCabinDeltaPressure: number;
    pressCabinVerticalSpeed: number;
    cpcsBxDiscreteWord: number;
    engineFuelUsed: number;
    engineFuelFlow: number;
    fws1_is_healthy: boolean;
    fws2_is_healthy: boolean;
    afdx_4_4_reachable: boolean;
    afdx_14_14_reachable: boolean;
    afdx_4_3_reachable: boolean;
    afdx_14_13_reachable: boolean;
    prim1Healthy: boolean;
    prim2Healthy: boolean;
    prim3Healthy: boolean;
    sec1Healthy: boolean;
    sec2Healthy: boolean;
    sec3Healthy: boolean;
    leftInnerAileronDeflection: number;
    rightInnerAileronDeflection: number;
    leftMiddleAileronDeflection: number;
    rightMiddleAileronDeflection: number;
    leftOuterAileronDeflection: number;
    rightOuterAileronDeflection: number;
    leftSpoiler1Deflection: number;
    rightSpoiler1Deflection: number;
    leftSpoiler2Deflection: number;
    rightSpoiler2Deflection: number;
    leftSpoiler3Deflection: number;
    rightSpoiler3Deflection: number;
    leftSpoiler4Deflection: number;
    rightSpoiler4Deflection: number;
    leftSpoiler5Deflection: number;
    rightSpoiler5Deflection: number;
    leftSpoiler6Deflection: number;
    rightSpoiler6Deflection: number;
    leftSpoiler7Deflection: number;
    rightSpoiler7Deflection: number;
    leftSpoiler8Deflection: number;
    rightSpoiler8Deflection: number;
    leftOuterElevatorDeflection: number;
    rightOuterElevatorDeflection: number;
    leftInnerElevatorDeflection: number;
    rightInnerElevatorDeflection: number;
    thsDeflection: number;
    upperRudderDeflection: number;
    lowerRudderDeflection: number;
    greenPressureSwitch: boolean;
    yellowPressureSwitch: boolean;
    acEssPowered: boolean;
    ac1Powered: boolean;
    acEhaPowered: boolean;
    sec1RudderStatusWord: number;
    sec3RudderStatusWord: number;
    sec1RudderTrimPosition: number;
    sec3RudderTrimPosition: number;
    dc1Powered: boolean;
    dcEssPowered: boolean;
    lgciu1LeftGearCompressed: boolean;
  };

type IndexedTopics =
  | 'condMainDeckTemp'
  | 'condUpperDeckTemp'
  | 'cpcsBxDiscreteWord'
  | 'pressCabinAltitude'
  | 'pressCabinDeltaPressure'
  | 'pressCabinVerticalSpeed'
  | 'engineFuelUsed'
  | 'engineFuelFlow';
type SDIndexedEvents = {
  [P in keyof Pick<BaseSDSimvars, IndexedTopics> as IndexedTopic<P>]: BaseSDSimvars[P];
};

export interface SDSimvars extends BaseSDSimvars, SDIndexedEvents {}

/** A publisher to poll and publish nav/com simvars. */
export class SDSimvarPublisher extends UpdatableSimVarPublisher<SDSimvars> {
  private static simvars = new Map<keyof SDSimvars, VarTopic<any>>([
    ...AdirsSimVarDefinitions,
    ...SwitchingPanelSimVarsDefinitions,
    ['moreActive', { name: 'L:A32NX_ECAM_SD_MORE_SHOWN', type: VarUnit.Bool }],
    ['sdPageToShow', { name: 'L:A32NX_ECAM_SD_PAGE_TO_SHOW', type: VarUnit.Enum }],
    ['sdStsPageToShow', { name: 'L:A32NX_ECAM_SD_STS_PAGE_TO_SHOW', type: VarUnit.Enum }],
    ['zuluTime', { name: 'E:fbw/ZULU_TIME', type: VarUnit.Seconds }],
    ['cockpitCabinTemp', { name: 'L:A32NX_COND_CKPT_TEMP', type: VarUnit.Number }],
    ['fwdCargoTemp', { name: 'L:A32NX_COND_CARGO_FWD_TEMP', type: VarUnit.Number }],
    ['aftCargoTemp', { name: 'L:A32NX_COND_CARGO_BULK_TEMP', type: VarUnit.Number }],
    ['condMainDeckTemp', { name: 'L:A32NX_COND_MAIN_DECK_#index#_TEMP', type: VarUnit.Number, indexed: true }],
    [
      'condUpperDeckTemp',
      { name: 'L:A32NX_COND_UPPER_DECK_#index#_TEMP', type: VarUnit.Number, indexed: true },
    ],
    ['cabinAltitudeIsAuto', { name: 'L:A32NX_OVHD_PRESS_MAN_ALTITUDE_PB_IS_AUTO', type: VarUnit.Bool }],
    ['cabinVerticalSpeedIsAuto', { name: 'L:A32NX_OVHD_PRESS_MAN_VS_CTL_PB_IS_AUTO', type: VarUnit.Bool }],
    ['manCabinAltitude', { name: 'L:A32NX_PRESS_MAN_CABIN_ALTITUDE', type: VarUnit.Number }],
    ['manCabinDeltaPressure', { name: 'L:A32NX_PRESS_MAN_CABIN_DELTA_PRESSURE', type: VarUnit.Number }],
    ['manCabinVerticalSpeed', { name: 'L:A32NX_PRESS_MAN_CABIN_VS', type: VarUnit.Number }],
    [
      'pressCabinAltitude',
      { name: 'L:A32NX_PRESS_CABIN_ALTITUDE_B#index#', type: VarUnit.Number, indexed: true },
    ],
    [
      'pressCabinDeltaPressure',
      { name: 'L:A32NX_PRESS_CABIN_DELTA_PRESSURE_B#index#', type: VarUnit.Number, indexed: true },
    ],
    [
      'pressCabinVerticalSpeed',
      { name: 'L:A32NX_PRESS_CABIN_VS_B#index#', type: VarUnit.Number, indexed: true },
    ],
    [
      'cpcsBxDiscreteWord',
      { name: 'L:A32NX_COND_CPIOM_B#index#_CPCS_DISCRETE_WORD', type: VarUnit.Number, indexed: true },
    ],
    ['engineFuelUsed', { name: 'L:A32NX_FUEL_USED:#index#', type: VarUnit.Number, indexed: true }],
    ['engineFuelFlow', { name: 'L:A32NX_ENGINE_FF:#index#', type: VarUnit.Number, indexed: true }],
    ['fws1_is_healthy', { name: 'L:A32NX_FWS1_IS_HEALTHY', type: VarUnit.Bool }],
    ['fws2_is_healthy', { name: 'L:A32NX_FWS2_IS_HEALTHY', type: VarUnit.Bool }],
    ['afdx_4_4_reachable', { name: 'L:A32NX_AFDX_4_4_REACHABLE', type: VarUnit.Bool }],
    ['afdx_14_14_reachable', { name: 'L:A32NX_AFDX_14_14_REACHABLE', type: VarUnit.Bool }],
    ['afdx_4_3_reachable', { name: 'L:A32NX_AFDX_4_3_REACHABLE', type: VarUnit.Bool }],
    ['afdx_14_13_reachable', { name: 'L:A32NX_AFDX_14_13_REACHABLE', type: VarUnit.Bool }],
    ['prim1Healthy', { name: 'L:A32NX_PRIM_1_HEALTHY', type: VarUnit.Bool }],
    ['prim2Healthy', { name: 'L:A32NX_PRIM_2_HEALTHY', type: VarUnit.Bool }],
    ['prim3Healthy', { name: 'L:A32NX_PRIM_3_HEALTHY', type: VarUnit.Bool }],
    ['sec1Healthy', { name: 'L:A32NX_SEC_1_HEALTHY', type: VarUnit.Bool }],
    ['sec2Healthy', { name: 'L:A32NX_SEC_2_HEALTHY', type: VarUnit.Bool }],
    ['sec3Healthy', { name: 'L:A32NX_SEC_3_HEALTHY', type: VarUnit.Bool }],
    [
      'leftInnerAileronDeflection',
      { name: 'L:A32NX_HYD_AILERON_LEFT_INWARD_DEFLECTION', type: VarUnit.Number },
    ],
    [
      'rightInnerAileronDeflection',
      { name: 'L:A32NX_HYD_AILERON_RIGHT_INWARD_DEFLECTION', type: VarUnit.Number },
    ],
    [
      'leftMiddleAileronDeflection',
      { name: 'L:A32NX_HYD_AILERON_LEFT_MIDDLE_DEFLECTION', type: VarUnit.Number },
    ],
    [
      'rightMiddleAileronDeflection',
      { name: 'L:A32NX_HYD_AILERON_RIGHT_MIDDLE_DEFLECTION', type: VarUnit.Number },
    ],
    [
      'leftOuterAileronDeflection',
      { name: 'L:A32NX_HYD_AILERON_LEFT_OUTWARD_DEFLECTION', type: VarUnit.Number },
    ],
    [
      'rightOuterAileronDeflection',
      { name: 'L:A32NX_HYD_AILERON_RIGHT_OUTWARD_DEFLECTION', type: VarUnit.Number },
    ],
    ['leftSpoiler1Deflection', { name: 'L:A32NX_HYD_SPOILER_1_LEFT_DEFLECTION', type: VarUnit.Number }],
    ['rightSpoiler1Deflection', { name: 'L:A32NX_HYD_SPOILER_1_RIGHT_DEFLECTION', type: VarUnit.Number }],
    ['leftSpoiler2Deflection', { name: 'L:A32NX_HYD_SPOILER_2_LEFT_DEFLECTION', type: VarUnit.Number }],
    ['rightSpoiler2Deflection', { name: 'L:A32NX_HYD_SPOILER_2_RIGHT_DEFLECTION', type: VarUnit.Number }],
    ['leftSpoiler3Deflection', { name: 'L:A32NX_HYD_SPOILER_3_LEFT_DEFLECTION', type: VarUnit.Number }],
    ['rightSpoiler3Deflection', { name: 'L:A32NX_HYD_SPOILER_3_RIGHT_DEFLECTION', type: VarUnit.Number }],
    ['leftSpoiler4Deflection', { name: 'L:A32NX_HYD_SPOILER_4_LEFT_DEFLECTION', type: VarUnit.Number }],
    ['rightSpoiler4Deflection', { name: 'L:A32NX_HYD_SPOILER_4_RIGHT_DEFLECTION', type: VarUnit.Number }],
    ['leftSpoiler5Deflection', { name: 'L:A32NX_HYD_SPOILER_5_LEFT_DEFLECTION', type: VarUnit.Number }],
    ['rightSpoiler5Deflection', { name: 'L:A32NX_HYD_SPOILER_5_RIGHT_DEFLECTION', type: VarUnit.Number }],
    ['leftSpoiler6Deflection', { name: 'L:A32NX_HYD_SPOILER_6_LEFT_DEFLECTION', type: VarUnit.Number }],
    ['rightSpoiler6Deflection', { name: 'L:A32NX_HYD_SPOILER_6_RIGHT_DEFLECTION', type: VarUnit.Number }],
    ['leftSpoiler7Deflection', { name: 'L:A32NX_HYD_SPOILER_7_LEFT_DEFLECTION', type: VarUnit.Number }],
    ['rightSpoiler7Deflection', { name: 'L:A32NX_HYD_SPOILER_7_RIGHT_DEFLECTION', type: VarUnit.Number }],
    ['leftSpoiler8Deflection', { name: 'L:A32NX_HYD_SPOILER_8_LEFT_DEFLECTION', type: VarUnit.Number }],
    ['rightSpoiler8Deflection', { name: 'L:A32NX_HYD_SPOILER_8_RIGHT_DEFLECTION', type: VarUnit.Number }],
    [
      'leftOuterElevatorDeflection',
      { name: 'L:A32NX_HYD_ELEVATOR_LEFT_OUTWARD_DEFLECTION', type: VarUnit.Number },
    ],
    [
      'rightOuterElevatorDeflection',
      { name: 'L:A32NX_HYD_ELEVATOR_RIGHT_OUTWARD_DEFLECTION', type: VarUnit.Number },
    ],
    [
      'leftInnerElevatorDeflection',
      { name: 'L:A32NX_HYD_ELEVATOR_LEFT_INWARD_DEFLECTION', type: VarUnit.Number },
    ],
    [
      'rightInnerElevatorDeflection',
      { name: 'L:A32NX_HYD_ELEVATOR_RIGHT_INWARD_DEFLECTION', type: VarUnit.Number },
    ],
    ['thsDeflection', { name: 'A:fbw/ELEVATOR_TRIM_POSITION', type: VarUnit.Number }],
    ['upperRudderDeflection', { name: 'L:A32NX_HYD_UPPER_RUDDER_DEFLECTION', type: VarUnit.Number }],
    ['lowerRudderDeflection', { name: 'L:A32NX_HYD_LOWER_RUDDER_DEFLECTION', type: VarUnit.Number }],
    ['lowerRudderDeflection', { name: 'L:A32NX_HYD_LOWER_RUDDER_DEFLECTION', type: VarUnit.Number }],
    ['greenPressureSwitch', { name: 'L:A32NX_HYD_GREEN_SYSTEM_1_SECTION_PRESSURE_SWITCH', type: VarUnit.Bool }],
    [
      'yellowPressureSwitch',
      { name: 'L:A32NX_HYD_YELLOW_SYSTEM_1_SECTION_PRESSURE_SWITCH', type: VarUnit.Bool },
    ],
    ['acEssPowered', { name: 'L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED', type: VarUnit.Bool }],
    ['ac1Powered', { name: 'L:A32NX_ELEC_AC_1_BUS_IS_POWERED', type: VarUnit.Bool }],
    ['acEhaPowered', { name: 'L:A32NX_ELEC_247XP_BUS_IS_POWERED', type: VarUnit.Bool }],
    ['sec1RudderStatusWord', { name: 'L:A32NX_SEC_1_RUDDER_STATUS_WORD', type: VarUnit.Enum }],
    ['sec3RudderStatusWord', { name: 'L:A32NX_SEC_3_RUDDER_STATUS_WORD', type: VarUnit.Enum }],
    ['sec1RudderTrimPosition', { name: 'L:A32NX_SEC_1_RUDDER_ACTUAL_POSITION', type: VarUnit.Enum }],
    ['sec3RudderTrimPosition', { name: 'L:A32NX_SEC_3_RUDDER_ACTUAL_POSITION', type: VarUnit.Enum }],
    ['dc1Powered', { name: 'L:A32NX_ELEC_DC_1_BUS_IS_POWERED', type: VarUnit.Bool }],
    ['dcEssPowered', { name: 'L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED', type: VarUnit.Bool }],
    ['lgciu1LeftGearCompressed', { name: 'L:A32NX_LGCIU_1_LEFT_GEAR_COMPRESSED', type: VarUnit.Bool }],
  ]);

  public constructor(bus: Bus) {
    super(SDSimvarPublisher.simvars, bus);
  }
}
