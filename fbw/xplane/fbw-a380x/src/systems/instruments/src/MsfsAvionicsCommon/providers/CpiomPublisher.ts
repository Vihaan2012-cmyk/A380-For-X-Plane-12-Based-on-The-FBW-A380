// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  Bus,
  IndexedTopic,
  VarPublisher,
  VarTopic,
  VarUnit,
} from 'xplane-js/ui';

/**
 * Transmitted from CPIOMs
 */
export interface BaseCpiomData {
  cpiom_a_available: boolean;
  cpiom_b_available: boolean;
  cpiom_c_available: boolean;
  cpiom_d_available: boolean;
  cpiom_e_available: boolean;
  cpiom_f_available: boolean;
  cpiom_g_available: boolean;
}

type IndexedTopics =
  | 'cpiom_a_available'
  | 'cpiom_b_available'
  | 'cpiom_c_available'
  | 'cpiom_d_available'
  | 'cpiom_e_available'
  | 'cpiom_f_available'
  | 'cpiom_g_available';
type CpiomIndexedEvents = {
  [P in keyof Pick<BaseCpiomData, IndexedTopics> as IndexedTopic<P>]: BaseCpiomData[P];
};

export interface CpiomData extends BaseCpiomData, CpiomIndexedEvents {}

/** A publisher to poll and publish nav/com simvars. */
export class CpiomDataPublisher extends VarPublisher<CpiomData> {
  private static simvars = new Map<keyof CpiomData, VarTopic<any>>([
    ['cpiom_a_available', { name: 'L:A32NX_CPIOM_A#index#_AVAIL', type: VarUnit.Bool, indexed: true }],
    ['cpiom_b_available', { name: 'L:A32NX_CPIOM_B#index#_AVAIL', type: VarUnit.Bool, indexed: true }],
    ['cpiom_c_available', { name: 'L:A32NX_CPIOM_C#index#_AVAIL', type: VarUnit.Bool, indexed: true }],
    ['cpiom_d_available', { name: 'L:A32NX_CPIOM_D#index#_AVAIL', type: VarUnit.Bool, indexed: true }],
    ['cpiom_e_available', { name: 'L:A32NX_CPIOM_E#index#_AVAIL', type: VarUnit.Bool, indexed: true }],
    ['cpiom_f_available', { name: 'L:A32NX_CPIOM_F#index#_AVAIL', type: VarUnit.Bool, indexed: true }],
    ['cpiom_g_available', { name: 'L:A32NX_CPIOM_G#index#_AVAIL', type: VarUnit.Bool, indexed: true }],
  ]);

  public constructor(bus: Bus) {
    super(CpiomDataPublisher.simvars, bus);
  }
}
