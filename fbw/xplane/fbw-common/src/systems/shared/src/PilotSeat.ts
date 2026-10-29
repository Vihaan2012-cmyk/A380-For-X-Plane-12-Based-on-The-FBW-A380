// @ts-strict-ignore
// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  Bus,
  BoardDevice,
  Pacer,
  VarPublisher,
  VarTopic,
  VarUnit,
  Signal,
} from 'xplane-js/ui';
import { NXDataStore } from '@flybywiresim/fbw-sdk';
import { Vars, Vec3Data } from 'xplane-js/data';

export enum PilotSeatConfig {
  Auto = 'Auto',
  Left = 'Left',
  Right = 'Right',
}

export const DefaultPilotSeatConfig = PilotSeatConfig.Left;

export enum PilotSeat {
  Left,
  Right,
}

export const DefaultPilotSeat = PilotSeat.Left;

export interface FlightDeckBounds {
  minX: number;
  maxX: number;
  minY: number;
  maxY: number;
  minZ: number;
  maxZ: number;
}

export class PilotSeatManager implements BoardDevice {
  private configSeat = PilotSeatConfig.Left;
  private readonly actualSeat = Signal.of(DefaultPilotSeat);
  private readonly inFlightDeck = Signal.of(false);

  constructor(private readonly flightDeckBounds: FlightDeckBounds) {}

  public setup(): void {
    NXDataStore.getAndSubscribeLegacy(
      'CONFIG_PILOT_SEAT',
      (_, config: PilotSeatConfig) => (this.configSeat = config),
      DefaultPilotSeatConfig,
    );

    this.actualSeat.watch((v) => Vars.write('L:FBW_PILOT_SEAT', VarUnit.Enum, v), true);
    this.inFlightDeck.watch((v) => Vars.write('L:FBW_IN_FLIGHT_DECK', VarUnit.Bool, v), true);
  }

  public tick(): void {
    const cameraPos: Vec3Data = Vars.readHost('fbw/CAMERA_POS_IN_PLANE', 'xyz');
    const cameraState: number = Vars.read('sim/graphics/view/view_is_external', VarUnit.Enum);
    this.inFlightDeck.set(this.isInFlightDeckBounds(cameraPos, cameraState));
    if (this.configSeat === PilotSeatConfig.Auto) {
      // if we are not inside the flight deck, do not update the side
      if (this.inFlightDeck.get()) {
        const inRightSide = cameraPos.x < 0;
        this.actualSeat.set(inRightSide ? PilotSeat.Right : PilotSeat.Left);
      }
    } else {
      this.actualSeat.set(this.configSeat === PilotSeatConfig.Right ? PilotSeat.Right : PilotSeat.Left);
    }
  }

  private isInFlightDeckBounds(pos: Vec3Data, cameraState: number): boolean {
    return (
      pos.x >= this.flightDeckBounds.minX &&
      pos.x <= this.flightDeckBounds.maxX &&
      pos.y >= this.flightDeckBounds.minY &&
      pos.y <= this.flightDeckBounds.maxY &&
      pos.z >= this.flightDeckBounds.minZ &&
      pos.z <= this.flightDeckBounds.maxZ &&
      (cameraState === 2 || cameraState === 3)
    );
  }
}

export interface PilotSeatEvents {
  /** Which seat the pilot/user is currently operating from. */
  pilot_seat: PilotSeat;
}

export class PilotSeatPublisher extends VarPublisher<PilotSeatEvents> {
  constructor(bus: Bus, pacer?: Pacer<PilotSeatEvents>) {
    const vars = new Map<keyof PilotSeatEvents, VarTopic<PilotSeatEvents>>([
      ['pilot_seat', { name: 'L:FBW_PILOT_SEAT', type: VarUnit.Enum }],
    ]);
    super(vars, bus, pacer);
  }
}
