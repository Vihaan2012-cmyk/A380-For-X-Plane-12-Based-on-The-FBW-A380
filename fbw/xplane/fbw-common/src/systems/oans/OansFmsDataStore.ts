// Copyright (c) 2023-2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { BusSignal, Bus } from 'xplane-js/ui';
import { FmsData } from '../shared/src/publishers/FmsPublisher';

export class OansFmsDataStore {
  constructor(private bus: Bus) {
    const sub = this.bus.inbox<FmsData>();
    this.origin.rebind(sub.topic('fmsOrigin'));
    this.destination.rebind(sub.topic('fmsDestination'));
    this.alternate.rebind(sub.topic('fmsAlternate'));
    this.departureRunway.rebind(sub.topic('fmsDepartureRunway'));
    this.landingRunway.rebind(sub.topic('fmsLandingRunway'));
  }

  public readonly origin = BusSignal.from<string | null>(null, null);

  public readonly destination = BusSignal.from<string | null>(null, null);

  public readonly alternate = BusSignal.from<string | null>(null, null);

  public readonly departureRunway = BusSignal.from<string | null>(null, null);

  public readonly landingRunway = BusSignal.from<string | null>(null, null);
}
