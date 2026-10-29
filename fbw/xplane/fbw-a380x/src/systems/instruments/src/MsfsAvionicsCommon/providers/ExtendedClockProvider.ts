// Copyright (c) 2021-2025 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { RegisteredSimVar } from '@flybywiresim/fbw-sdk';
import { ClockTopics, Bus, BoardDevice, VarUnit, Signal, Readable } from 'xplane-js/ui';

export interface ExtendedClockEvents {
  /** Time in milliseconds since the start of the sim session, accumulating at simrate. */
  ext_clock_monotonic_time: number;
  /** Time in milliseconds since the previous monotonic clock update. */
  ext_clock_delta_time: number;
  /** Flashes on/off at 1 Hz rate when the system is powered. */
  ext_clock_one_hertz: boolean;
}

export class ExtendedClockEventProvider implements BoardDevice {
  private prevSimTime?: number;
  private powerOnTime?: number;

  private readonly oneHertzClock = Signal.of(false);
  private readonly monotonicSimTimeVar = RegisteredSimVar.create<number>('E:fbw/SIMULATION_TIME', VarUnit.Number);

  constructor(
    private readonly bus: Bus,
    private readonly isPowered: Readable<boolean>,
  ) {}

  /** @inheritdoc */
  public setup(): void {
    const publisher = this.bus.outbox<ExtendedClockEvents>();
    const subscriber = this.bus.inbox<ClockTopics>();

    this.isPowered.watch((isPowered) => {
      if (isPowered) {
        this.powerOnTime = this.monotonicSimTimeVar.get() * 1000;
      } else {
        this.powerOnTime = undefined;
      }
    }, true);

    subscriber.topic('simTime').listen(() => {
      const monotonicTime = this.monotonicSimTimeVar.get() * 1000;
      const deltaTime = this.prevSimTime !== undefined ? monotonicTime - this.prevSimTime : 0;

      if (this.powerOnTime !== undefined) {
        const poweredOnTime = monotonicTime - this.powerOnTime;
        this.oneHertzClock.set(poweredOnTime % 1000 >= 500);
      } else {
        this.oneHertzClock.set(false);
      }

      publisher.post('ext_clock_delta_time', deltaTime);
      publisher.post('ext_clock_monotonic_time', monotonicTime);

      this.prevSimTime = monotonicTime;
    });

    this.oneHertzClock.watch((oneHertzClock) => {
      publisher.post('ext_clock_one_hertz', oneHertzClock);
    });
  }

  /** @inheritdoc */
  public tick(): void {
    // noop
  }
}
