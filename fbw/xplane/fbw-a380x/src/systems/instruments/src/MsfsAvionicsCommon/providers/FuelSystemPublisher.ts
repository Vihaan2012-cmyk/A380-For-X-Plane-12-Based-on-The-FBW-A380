// Copyright (c) 2021-2025 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

/* TODO: remove this file after proper FQMS is implemented in Rust */

import {
  Bus,
  IndexedTopic,
  Pacer,
  VarPublisher,
  VarTopic,
  VarUnit,
} from 'xplane-js/ui';

/* eslint-disable camelcase */
export interface BaseFuelSystemEvents {
  /** The valve's switch: */
  fuel_valve_switch: boolean;
  /** The valve's actual continous position, in percent, 0 ... 1 */
  fuel_valve_open: number;
  /** The pump's switch state */
  fuel_pump_switch: boolean;
  /** The pump's active state (ex. false when pump is on but no fuel in tank) */
  fuel_pump_active: boolean;
  /** The engines's fuel pressure in psi. */
  fuel_engine_pressure: number;
  /** The pressure of a fuel line in psi. */
  fuel_line_pressure: number;
  /** The fuel flow of a fuel line in gallons per hour. */
  fuel_line_flow: number;
  /** The quantity of fuel in the selected tank (by tank index), in gallons. */
  fuel_tank_quantity: number;
  /** The state of the trigger (by index) as boolean */
  fuel_trigger_status: boolean;
  /** The setting of the junction (by index) as boolean */
  fuel_junction_setting: number;
  /** Whether refuel has been started as boolean */
  fuel_refuel_started_by_user: boolean;
  /** Desired fuel target for fuelling in kilograms */
  fuel_desired_by_user: number;
}

type IndexedTopics =
  | 'fuel_valve_switch'
  | 'fuel_valve_open'
  | 'fuel_pump_switch'
  | 'fuel_pump_active'
  | 'fuel_engine_pressure'
  | 'fuel_line_pressure'
  | 'fuel_line_flow'
  | 'fuel_tank_quantity'
  | 'fuel_trigger_status'
  | 'fuel_junction_setting';

type FuelSystemIndexedEvents = {
  [P in keyof Pick<BaseFuelSystemEvents, IndexedTopics> as IndexedTopic<P>]: BaseFuelSystemEvents[P];
};

/**
 * Fuel System events.
 */
export interface FuelSystemEvents extends BaseFuelSystemEvents, FuelSystemIndexedEvents {}

export class FuelSystemPublisher extends VarPublisher<FuelSystemEvents> {
  constructor(bus: Bus, pacer?: Pacer<FuelSystemEvents>) {
    const simvars: [keyof FuelSystemEvents, VarTopic<any>][] = [
      [
        'fuel_valve_switch',
        { name: 'fbw/FUELSYSTEM_VALVE_SWITCH_#index#', type: VarUnit.Bool, indexed: true, transform: (v) => !!v },
      ],
      ['fuel_valve_open', { name: 'fbw/FUELSYSTEM_VALVE_OPEN_#index#', type: VarUnit.Number, indexed: true }],
      [
        'fuel_pump_switch',
        { name: 'fbw/FUELSYSTEM_PUMP_SWITCH_#index#', type: VarUnit.Bool, indexed: true, transform: (v) => !!v },
      ],
      [
        'fuel_pump_active',
        { name: 'fbw/FUELSYSTEM_PUMP_ACTIVE_#index#', type: VarUnit.Bool, indexed: true, transform: (v) => !!v },
      ],
      [
        'fuel_engine_pressure',
        { name: 'fbw/FUELSYSTEM_ENGINE_PRESSURE_#index#', type: VarUnit.PSI, indexed: true },
      ],
      [
        'fuel_line_pressure',
        { name: 'fbw/FUELSYSTEM_LINE_FUEL_PRESSURE_#index#', type: VarUnit.PSI, indexed: true },
      ],
      ['fuel_line_flow', { name: 'fbw/FUELSYSTEM_LINE_FUEL_FLOW_#index#', type: VarUnit.GPH, indexed: true }],
      ['fuel_tank_quantity', { name: 'fbw/FUELSYSTEM_TANK_QUANTITY_#index#', type: VarUnit.GAL, indexed: true }],
      [
        'fuel_trigger_status',
        { name: 'fbw/FUELSYSTEM_TRIGGER_STATUS_#index#', type: VarUnit.Bool, indexed: true, transform: (v) => !!v },
      ],
      [
        'fuel_junction_setting',
        { name: 'fbw/FUELSYSTEM_JUNCTION_SETTING_#index#', type: VarUnit.Number, indexed: true },
      ],
      [
        'fuel_refuel_started_by_user',
        { name: 'L:A32NX_REFUEL_STARTED_BY_USR', type: VarUnit.Bool, indexed: false, transform: (v) => !!v },
      ],
      [
        'fuel_desired_by_user',
        {
          name: 'L:A32NX_FUEL_DESIRED',
          type: VarUnit.Number,
        },
      ],
    ];

    super(new Map(simvars), bus, pacer);
  }
}
