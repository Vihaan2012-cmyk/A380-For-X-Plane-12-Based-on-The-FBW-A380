// Copyright (c) 2021-2022 FlyByWire Simulations
// Copyright (c) 2021-2022 Synaptic Simulations
//
// SPDX-License-Identifier: GPL-3.0

import { Bus, Signal, Bond } from 'xplane-js/ui';
import { FlightPlanEvents } from './FlightPlanEvents';

export class FlightPlanChangeNotifier {
  /** This subject is notified when any flight plan was changed, swapped, deleted. Don't listen to the value itself. */
  public readonly flightPlanChanged = Signal.of(false);

  private readonly subscription: Bond[] = [];
  private readonly sub = this.bus.inbox<FlightPlanEvents>();

  constructor(private readonly bus: Bus) {
    this.subscription.push(
      this.sub.topic('flightPlanManager.create').listen(() => {
        this.flightPlanChanged.set(true);
        this.flightPlanChanged.emit();
      }),
      this.sub.topic('flightPlanManager.delete').listen(() => {
        this.flightPlanChanged.set(true);
        this.flightPlanChanged.emit();
      }),
      this.sub.topic('flightPlanManager.deleteAll').listen(() => {
        this.flightPlanChanged.set(true);
        this.flightPlanChanged.emit();
      }),
      this.sub.topic('flightPlanManager.swap').listen(() => {
        this.flightPlanChanged.set(true);
        this.flightPlanChanged.emit();
      }),
      this.sub.topic('flightPlanManager.copy').listen(() => {
        this.flightPlanChanged.set(true);
        this.flightPlanChanged.emit();
      }),
    );
  }

  destroy(): void {
    this.subscription.forEach((sub) => sub.destroy());
  }
}
