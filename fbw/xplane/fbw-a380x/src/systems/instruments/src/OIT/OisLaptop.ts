// Copyright (c) 2025 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  BusSignal,
  Bus,
  BoardDevice,
  VarUnit,
  Signal,
  Readable,
  Bond,
} from 'xplane-js/ui';
import { FailuresConsumer, FmsData } from '@flybywiresim/fbw-sdk';
import { A380Failure } from '@failures';
import { OisInternalData } from './OisInternalPublisher';
import { OitSimvars } from './OitSimvarPublisher';
import { Vars } from 'xplane-js/data';

type LaptopIndex = 1 | 2;

export class OisLaptop implements BoardDevice {
  private readonly subscriptions: Bond[] = [];

  public readonly data = new OisLaptopData();

  private readonly failureKey = this.index === 1 ? A380Failure.CaptainLaptop : A380Failure.FirstOfficerLaptop;

  private readonly powered = Signal.of(false);

  private readonly _isHealthy = Signal.of(false);
  public readonly isHealthy = this._isHealthy as Readable<boolean>;
  private readonly isHealthySimVar = `L:A32NX_FLTOPS_LAPTOP_${this.index.toFixed(0)}_IS_HEALTHY`;

  private readonly sub = this.bus.inbox<FmsData & OisInternalData & OitSimvars>();

  private readonly laptopSwitchedOn = BusSignal.from(
    this.sub.topic(this.index === 1 ? 'laptopCaptPowered' : 'laptopFoPowered'),
    false,
  );

  private readonly nssMasterOff = BusSignal.from(this.sub.topic('nssMasterOff'), false);

  private readonly fltNumberBus = BusSignal.from(this.sub.topic('fmsFlightNumber'), null);
  private readonly fromAirportBus = BusSignal.from(this.sub.topic('fmsOrigin'), null);
  private readonly toAirportBus = BusSignal.from(this.sub.topic('fmsDestination'), null);

  public synchroAvionics() {
    this.data.fltNumber.set(this.fltNumberBus.get());
    this.data.fromAirport.set(this.fromAirportBus.get());
    this.data.toAirport.set(this.toAirportBus.get());

    // Workaround since synced messages in the same VCockpit don't work
    Vars.write('L:A32NX_OIS_SYNCHRO_AVIONICS', VarUnit.Number, Math.random());
  }

  constructor(
    private readonly bus: Bus,
    private readonly index: LaptopIndex,
    private readonly failuresConsumer: FailuresConsumer,
  ) {}

  /** @inheritdoc */
  setup(): void {
    this.failuresConsumer.register(this.failureKey);

    this.subscriptions.push(
      this.sub.topic('synchroAvncs').listen(() => this.synchroAvionics()),
      this._isHealthy.watch((v) => Vars.write(this.isHealthySimVar, VarUnit.Bool, v)),
    );

    this.subscriptions.push(this.fltNumberBus, this.fromAirportBus, this.toAirportBus);
  }

  /** @inheritdoc */
  tick(): void {
    const failed = this.failuresConsumer.isActive(this.failureKey);

    if (this.index === 1) {
      this.powered.set(
        Vars.read('L:A32NX_ELEC_DC_1_BUS_IS_POWERED', VarUnit.Bool) && this.laptopSwitchedOn.get(),
      );
    } else {
      this.powered.set(
        (Vars.read('L:A32NX_ELEC_DC_2_BUS_IS_POWERED', VarUnit.Bool) ||
          Vars.read('L:A32NX_ELEC_DC_HOT_2_BUS_IS_POWERED', VarUnit.Bool)) &&
          this.laptopSwitchedOn.get(),
      );
    }

    this._isHealthy.set(!failed && this.powered.get() && !this.nssMasterOff.get());
  }

  destroy() {
    for (const s of this.subscriptions) {
      s.destroy();
    }
  }
}

export class OisLaptopData {
  public readonly fltNumber = Signal.of<string | null>(null);
  public readonly fromAirport = Signal.of<string | null>(null);
  public readonly toAirport = Signal.of<string | null>(null);
}
