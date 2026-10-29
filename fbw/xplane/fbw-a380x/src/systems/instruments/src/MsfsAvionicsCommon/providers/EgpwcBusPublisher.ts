// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import { Bus, VarPublisher, VarUnit } from 'xplane-js/ui';
import { EfisSide } from '@flybywiresim/fbw-sdk';

export enum TerrainLevelMode {
  PeaksMode = 0,
  Warning = 1,
  Caution = 2,
}

export interface EgpwcSimVars {
  'egpwc.minElevation': number;
  'egpwc.minElevationMode': TerrainLevelMode;
  'egpwc.maxElevation': number;
  'egpwc.maxElevationMode': TerrainLevelMode;
  /** Raw ARINC 429 word */
  'egpwc.presentLatitude': number;
  /** Raw ARINC 429 word */
  'egpwc.presentLongitude': number;
  /** Raw ARINC 429 word */
  'egpwc.presentAltitude': number;
  /** Raw ARINC 429 word */
  'egpwc.presentHeading': number;
  /** Raw ARINC 429 word */
  'egpwc.presentVerticalSpeed': number;
  /** Raw ARINC 429 word */
  'egpwc.destinationLatitude': number;
  /** Raw ARINC 429 word */
  'egpwc.destinationLongitude': number;
  'egpwc.gearIsDown': number;
  'egpwc.terrainActive': boolean;
  'egpwc.terrOnNdRenderingMode': number;
}

export class EgpwcBusPublisher extends VarPublisher<EgpwcSimVars> {
  constructor(bus: Bus, side: EfisSide) {
    super(
      new Map([
        [
          'egpwc.minElevation',
          { name: `L:A32NX_EGPWC_ND_${side}_TERRAIN_MIN_ELEVATION`, type: VarUnit.Number },
        ],
        [
          'egpwc.minElevationMode',
          { name: `L:A32NX_EGPWC_ND_${side}_TERRAIN_MIN_ELEVATION_MODE`, type: VarUnit.Number },
        ],
        [
          'egpwc.maxElevation',
          { name: `L:A32NX_EGPWC_ND_${side}_TERRAIN_MAX_ELEVATION`, type: VarUnit.Number },
        ],
        [
          'egpwc.maxElevationMode',
          { name: `L:A32NX_EGPWC_ND_${side}_TERRAIN_MAX_ELEVATION_MODE`, type: VarUnit.Number },
        ],
        ['egpwc.terrainActive', { name: `L:A32NX_EGPWC_ND_${side}_TERRAIN_ACTIVE`, type: VarUnit.Number }],
        ['egpwc.presentLatitude', { name: `L:A32NX_EGPWC_PRESENT_LAT`, type: VarUnit.Number }],
        ['egpwc.presentLongitude', { name: `L:A32NX_EGPWC_PRESENT_LONG`, type: VarUnit.Number }],
        ['egpwc.presentAltitude', { name: `L:A32NX_EGPWC_PRESENT_ALTITUDE`, type: VarUnit.Number }],
        ['egpwc.presentHeading', { name: `L:A32NX_EGPWC_PRESENT_HEADING`, type: VarUnit.Number }],
        ['egpwc.presentVerticalSpeed', { name: `L:A32NX_EGPWC_PRESENT_VERTICAL_SPEED`, type: VarUnit.Number }],
        ['egpwc.destinationLatitude', { name: `L:A32NX_EGPWC_DEST_LAT`, type: VarUnit.Number }],
        ['egpwc.destinationLongitude', { name: `L:A32NX_EGPWC_DEST_LONG`, type: VarUnit.Number }],
        ['egpwc.gearIsDown', { name: `L:A32NX_EGPWC_GEAR_IS_DOWN`, type: VarUnit.Number }],
        [
          'egpwc.terrOnNdRenderingMode',
          { name: `L:A32NX_EGPWC_TERRONND_RENDERING_MODE`, type: VarUnit.Number },
        ],
      ]),
      bus,
    );
  }
}
