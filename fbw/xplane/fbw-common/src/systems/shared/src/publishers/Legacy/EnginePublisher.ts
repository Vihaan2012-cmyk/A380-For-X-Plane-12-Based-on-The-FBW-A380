// Copyright (c) 2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  Bus,
  IndexedTopic,
  Pacer,
  VarPublisher,
  VarTopic,
  VarUnit,
} from 'xplane-js/ui';

export enum EngineState {
  Off = 0,
  On = 1,
  Starting = 2,
  Restarting = 3,
  Shutting = 4,
}

interface EngineBaseEvents {
  /** Whether the engine master switch is on. */
  fbw_engine_master: boolean;
  /** Whether the FADEC is powered. */
  fbw_engine_fadec_powered: boolean;
  /** The engine state. */
  fbw_engine_state: EngineState;
  /** The thrust lever angle in degrees. */
  fbw_engine_thrust_lever_angle: number;
}

type IndexedTopics = keyof EngineBaseEvents;

type EngineIndexedEvents = {
  [P in keyof Pick<EngineBaseEvents, IndexedTopics> as IndexedTopic<P>]: EngineBaseEvents[P];
};

interface EnginePublisherEvents extends EngineBaseEvents, EngineIndexedEvents {}

/**
 * Events for engine local vars from the legacy "FADEC".
 */
export interface EngineEvents extends Omit<EngineBaseEvents, IndexedTopics>, EngineIndexedEvents {}

/**
 * Publisher for engine local vars.
 */
export class EnginePublisher extends VarPublisher<EnginePublisherEvents> {
  /**
   * Create a publisher.
   * @param bus The EventBus to publish to
   * @param pacer An optional pacer to use to control the rate of publishing
   */
  public constructor(bus: Bus, pacer?: Pacer<EnginePublisherEvents>) {
    const simvars = new Map<keyof EnginePublisherEvents, VarTopic<any>>([
      [
        'fbw_engine_master',
        {
          name: 'A:fbw/FUELSYSTEM_VALVE_SWITCH_#index#',
          type: VarUnit.Bool,
          indexed: true,
        },
      ],
      [
        'fbw_engine_fadec_powered',
        {
          name: 'L:A32NX_FADEC_POWERED_ENG#index#',
          type: VarUnit.Bool,
          indexed: true,
        },
      ],
      [
        'fbw_engine_state',
        {
          name: 'L:A32NX_ENGINE_STATE:#index#',
          type: VarUnit.Enum,
          indexed: true,
        },
      ],
      [
        'fbw_engine_thrust_lever_angle',
        {
          name: 'L:A32NX_AUTOTHRUST_TLA:#index#',
          type: VarUnit.Number,
          indexed: true,
        },
      ],
    ]);

    super(simvars, bus, pacer);
  }
}
