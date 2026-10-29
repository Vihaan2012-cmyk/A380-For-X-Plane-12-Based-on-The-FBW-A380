// @ts-strict-ignore
// Copyright (c) 2022 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { FmgcFlightPhase } from '@shared/flightphase';
import { FlightArea } from './FlightArea';
import { BusValue, Bus } from 'xplane-js/ui';
import { FlightPhaseManagerEvents } from '@fmgc/flightphase';
import { FlightPlanService } from '../flightplanning/FlightPlanService';
import { Vars } from 'xplane-js/data';

const rnpDefaults: Record<FlightArea, number> = {
  [FlightArea.Takeoff]: 1,
  [FlightArea.Terminal]: 1,
  [FlightArea.Enroute]: 2,
  [FlightArea.Oceanic]: 2,
  [FlightArea.VorApproach]: 0.5,
  [FlightArea.GpsApproach]: 0.3,
  [FlightArea.PrecisionApproach]: 0.5,
  [FlightArea.NonPrecisionApproach]: 0.5,
};

// FIXME RNP-related scratchpad messages

export class RequiredPerformance {
  activeRnp: number | undefined;

  requestLDev = false;

  manualRnp = false;

  private readonly flightPhase = BusValue.from(
    this.bus.inbox<FlightPhaseManagerEvents>().topic('fmgc_flight_phase'),
    FmgcFlightPhase.Preflight,
  );

  constructor(
    private readonly bus: Bus,
    private flightPlanService: FlightPlanService,
  ) {}

  update(_deltaTime: number): void {
    this.updateAutoRnp();

    this.updateLDev();
  }

  setPilotRnp(rnp): void {
    this.manualRnp = true;
    this.setActiveRnp(rnp);
  }

  clearPilotRnp(): void {
    this.manualRnp = false;
    this.updateAutoRnp();
  }

  private updateAutoRnp(): void {
    if (this.manualRnp) {
      return;
    }

    const plan = this.flightPlanService.active;

    if (plan && plan.activeLeg && plan.activeLeg.isDiscontinuity === false) {
      const legRnp = plan.activeLeg.rnp;

      if (legRnp !== undefined) {
        if (legRnp !== this.activeRnp) {
          this.setActiveRnp(legRnp);
        }
        return;
      }
    }

    const area = this.flightPlanService.active.calculateActiveArea();
    const rnp = rnpDefaults[area];

    if (rnp !== this.activeRnp) {
      this.setActiveRnp(rnp);
    }
  }

  private setActiveRnp(rnp: number): void {
    this.activeRnp = rnp;
    Vars.write('L:A32NX_FMGC_L_RNP', 'number', rnp ?? 0);
    Vars.write('L:A32NX_FMGC_R_RNP', 'number', rnp ?? 0);
  }

  private updateLDev(): void {
    const area = this.flightPlanService.active.calculateActiveArea();
    const ldev =
      area !== FlightArea.Enroute &&
      area !== FlightArea.Oceanic &&
      this.activeRnp < 0.305 &&
      this.flightPhase.get() >= FmgcFlightPhase.Takeoff;
    if (ldev !== this.requestLDev) {
      this.requestLDev = ldev;
      Vars.write('L:A32NX_FMGC_L_LDEV_REQUEST', 'bool', this.requestLDev);
      Vars.write('L:A32NX_FMGC_R_LDEV_REQUEST', 'bool', this.requestLDev);
    }
  }
}
