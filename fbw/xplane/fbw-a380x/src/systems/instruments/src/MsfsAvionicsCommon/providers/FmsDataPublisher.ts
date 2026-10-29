import { Bus, VarUnit, Signal } from 'xplane-js/ui';
import { SwitchableSimVarProvider } from './SwitchableProvider';

export interface FmsVars {
  ndMessageFlags: number;
  crossTrackError: number;
  linearDeviationActive: boolean;
  targetAltitude: number;
  verticalProfileLatched: boolean;
  showSpeedMargins: boolean;
  upperSpeedMargin: number;
  lowerSpeedMargin: number;
  rnp: number;
  toWptIdent0: number;
  toWptIdent1: number;
  toWptBearing: number;
  toWptTrueBearing: number;
  toWptDistance: number;
  toWptEta: number;
  apprMessage0: number;
  apprMessage1: number;
  mrpLat: number;
  mrpLong: number;
  fmsFailed: boolean;
}

export class FmsDataPublisher extends SwitchableSimVarProvider<FmsVars, 'L' | 'R'> {
  constructor(bus: Bus, stateSubject: Signal<'L' | 'R'>) {
    super(
      new Map([
        [
          'ndMessageFlags',
          { name: (side) => `L:A32NX_EFIS_${side}_ND_FM_MESSAGE_FLAGS`, type: VarUnit.Number },
        ],
        ['crossTrackError', { name: (_side) => 'L:A32NX_FG_CROSS_TRACK_ERROR', type: VarUnit.NM }],
        [
          'linearDeviationActive',
          { name: (_side) => 'L:A32NX_PFD_LINEAR_DEVIATION_ACTIVE', type: VarUnit.Bool },
        ],
        ['targetAltitude', { name: (_side) => 'L:A32NX_PFD_TARGET_ALTITUDE', type: VarUnit.Feet }],
        [
          'verticalProfileLatched',
          { name: (_side) => 'L:A32NX_PFD_VERTICAL_PROFILE_LATCHED', type: VarUnit.Bool },
        ],
        ['showSpeedMargins', { name: (_side) => 'L:A32NX_PFD_SHOW_SPEED_MARGINS', type: VarUnit.Bool }],
        ['upperSpeedMargin', { name: (_side) => 'L:A32NX_PFD_UPPER_SPEED_MARGIN', type: VarUnit.Knots }],
        ['lowerSpeedMargin', { name: (_side) => 'L:A32NX_PFD_LOWER_SPEED_MARGIN', type: VarUnit.Knots }],
        ['rnp', { name: (side) => `L:A32NX_FMGC_${side}_RNP`, type: VarUnit.Number }],
        ['toWptIdent0', { name: (side) => `L:A32NX_EFIS_${side}_TO_WPT_IDENT_0`, type: VarUnit.Number }],
        ['toWptIdent1', { name: (side) => `L:A32NX_EFIS_${side}_TO_WPT_IDENT_1`, type: VarUnit.Number }],
        ['toWptBearing', { name: (side) => `L:A32NX_EFIS_${side}_TO_WPT_BEARING`, type: VarUnit.Degree }],
        [
          'toWptTrueBearing',
          { name: (side) => `L:A32NX_EFIS_${side}_TO_WPT_TRUE_BEARING`, type: VarUnit.Degree },
        ],
        ['toWptDistance', { name: (side) => `L:A32NX_EFIS_${side}_TO_WPT_DISTANCE`, type: VarUnit.Number }],
        ['toWptEta', { name: (side) => `L:A32NX_EFIS_${side}_TO_WPT_ETA`, type: VarUnit.Seconds }],
        ['apprMessage0', { name: (side) => `L:A32NX_EFIS_${side}_APPR_MSG_0`, type: VarUnit.Number }],
        ['apprMessage1', { name: (side) => `L:A32NX_EFIS_${side}_APPR_MSG_1`, type: VarUnit.Number }],
        ['mrpLat', { name: (side) => `L:A32NX_EFIS_${side}_MRP_LAT`, type: VarUnit.Degree }],
        ['mrpLong', { name: (side) => `L:A32NX_EFIS_${side}_MRP_LONG`, type: VarUnit.Degree }],
        ['fmsFailed', { name: (side) => `L:A32NX_FMS_${side}_FAILED`, type: VarUnit.Bool }],
      ]),
      stateSubject,
      bus,
    );
  }
}
