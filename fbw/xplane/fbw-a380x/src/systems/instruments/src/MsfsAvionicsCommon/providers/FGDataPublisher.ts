import { Bus, VarPublisher, VarUnit } from 'xplane-js/ui';
import { AutoThrustMode, AutoThrustModeMessage, LateralMode, VerticalMode } from '@shared/autopilot';

export enum FgLateralArmedFlags {
  None = 0,
  Nav = 1 << 0,
  Loc = 1 << 1,
}

export enum FgVerticalArmedFlags {
  None = 0,
  Alt = 1 << 0,
  AltCst = 1 << 1,
  Clb = 1 << 2,
  Des = 1 << 3,
  Gs = 1 << 4,
  Final = 1 << 5,
  Tcas = 1 << 6,
}

export interface FGVars {
  'fg.fma.lateralMode': LateralMode;
  'fg.fma.lateralArmedBitmask': FgLateralArmedFlags;
  'fg.fma.verticalMode': VerticalMode;
  'fg.fma.verticalArmedBitmask': FgVerticalArmedFlags;
  'fg.speeds.managed': number;
  /** Altitude constraint in ft. 0 if not set. */
  'fg.altitudeConstraint': number;

  'fg.athr.message': AutoThrustModeMessage;
  'fg.athr.mode': AutoThrustMode;

  /** Which MMR deviations are the FLS's: 0 none, 1 vertical (F-G/S), 2 also lateral (F-LOC). */
  'fg.fls': number;
  /** The FLS anchor ident, packed by SimVarString. */
  'fg.fls.ident0': number;
  'fg.fls.ident1': number;
  /** Degrees. */
  'fg.fls.slope': number;
  /** 0 none, 1 RAW ONLY, 2 F-APP + RAW, 3 F-APP. */
  'fg.fls.capability': number;
  /** The FMS's beam for the ND: 0 none, 1 vertical only, 2 lateral and vertical. */
  'fg.fls.beam': number;
  'fg.fls.anchorLat': number;
  'fg.fls.anchorLong': number;
  /** Degrees true, inbound. */
  'fg.fls.course': number;
  'fg.fls.anchorPseudo': number;
}

export class FGDataPublisher extends VarPublisher<FGVars> {
  constructor(bus: Bus) {
    super(
      new Map([
        ['fg.fma.lateralMode', { name: 'L:A32NX_FMA_LATERAL_MODE', type: VarUnit.Number }],
        ['fg.fma.lateralArmedBitmask', { name: 'L:A32NX_FMA_LATERAL_ARMED', type: VarUnit.Number }],
        ['fg.fma.verticalMode', { name: 'L:A32NX_FMA_VERTICAL_MODE', type: VarUnit.Number }],
        ['fg.fma.verticalArmedBitmask', { name: 'L:A32NX_FMA_VERTICAL_ARMED', type: VarUnit.Number }],
        ['fg.speeds.managed', { name: 'L:A32NX_SPEEDS_MANAGED_PFD', type: VarUnit.Knots }],
        ['fg.altitudeConstraint', { name: 'L:A32NX_FG_ALTITUDE_CONSTRAINT', type: VarUnit.Feet }],
        ['fg.athr.message', { name: 'L:A32NX_AUTOTHRUST_MODE_MESSAGE', type: VarUnit.Enum }],
        ['fg.athr.mode', { name: 'L:A32NX_AUTOTHRUST_MODE', type: VarUnit.Enum }],
        ['fg.fls', { name: 'L:A32NX_FLS_1_ACTIVE', type: VarUnit.Number }],
        ['fg.fls.ident0', { name: 'L:A32NX_FM_FLS_IDENT_0', type: VarUnit.Number }],
        ['fg.fls.ident1', { name: 'L:A32NX_FM_FLS_IDENT_1', type: VarUnit.Number }],
        ['fg.fls.slope', { name: 'L:A32NX_FM_FLS_SLOPE', type: VarUnit.Number }],
        ['fg.fls.capability', { name: 'L:A32NX_FM_FLS_CAPABILITY', type: VarUnit.Number }],
        ['fg.fls.beam', { name: 'L:A32NX_FM_FLS_STATE', type: VarUnit.Number }],
        ['fg.fls.anchorLat', { name: 'L:A32NX_FM_FLS_ANCHOR_LAT', type: VarUnit.Number }],
        ['fg.fls.anchorLong', { name: 'L:A32NX_FM_FLS_ANCHOR_LONG', type: VarUnit.Number }],
        ['fg.fls.course', { name: 'L:A32NX_FM_FLS_COURSE', type: VarUnit.Number }],
        ['fg.fls.anchorPseudo', { name: 'L:A32NX_FM_FLS_ANCHOR_PSEUDO', type: VarUnit.Number }],
      ]),
      bus,
    );
  }
}
