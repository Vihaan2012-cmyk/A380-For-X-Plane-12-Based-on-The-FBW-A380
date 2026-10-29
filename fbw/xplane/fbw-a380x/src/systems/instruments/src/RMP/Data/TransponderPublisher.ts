// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  Bus,
  IndexedTopic,
  Pacer,
  VarPublisher,
  VarTopic,
  VarUnit,
} from 'xplane-js/ui';

export enum TransponderState {
  Off = 0,
  Standby = 1,
  Test = 2,
  ModeA = 3,
  ModeC = 4,
  ModeS = 5,
}

interface BaseTransponderEvents {
  /** The transponder code in octal. */
  transponder_code: number;
  /** Whether the ident function is active. */
  transponder_ident: boolean;
  /** The current state of the transponder. */
  transponder_auto: boolean;
}

type IndexedTopics = 'transponder_code' | 'transponder_ident' | 'transponder_auto';

type TransponderIndexedEvents = {
  [P in keyof Pick<BaseTransponderEvents, IndexedTopics> as IndexedTopic<P>]: BaseTransponderEvents[P];
};

export interface TransponderEvents extends BaseTransponderEvents, TransponderIndexedEvents {}

export class TransponderPublisher extends VarPublisher<TransponderEvents> {
  /**
   * Create a transponder publisher.
   * @param bus The EventBus to publish to
   * @param pacer An optional pacer to use to control the rate of publishing
   */
  public constructor(bus: Bus, pacer?: Pacer<TransponderEvents>) {
    const simvars = new Map<keyof TransponderEvents, VarTopic<any>>([
      ['transponder_code', { name: `fbw/TRANSPONDER_CODE_#index#`, type: 'bco16' as VarUnit, indexed: true }],
      ['transponder_ident', { name: `fbw/TRANSPONDER_IDENT_#index#`, type: VarUnit.Bool, indexed: true }],
      [
        'transponder_auto',
        {
          name: `fbw/TRANSPONDER_STATE_#index#`,
          type: VarUnit.Enum,
          indexed: true,
          transform: (v) => v === TransponderState.ModeA || v === TransponderState.ModeC || v === TransponderState.ModeS,
        },
      ],
    ]);

    super(simvars, bus, pacer);
  }
}
