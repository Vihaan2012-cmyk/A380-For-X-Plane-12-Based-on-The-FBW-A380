// @ts-strict-ignore
// Copyright (c) 2021-2025 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import { Arinc429Register, ConfirmationNode, NXDataStore } from '@flybywiresim/fbw-sdk';
import { Gettable, BusValue, Bus, VarUnit } from 'xplane-js/ui';
import { VerticalMode } from '@shared/autopilot';
import {
  FmgcFlightPhase,
  getAutopilotVerticalMode,
  isAnEngineOn,
  isOnGround,
  conditionTakeOff,
} from '@shared/flightphase';
import { EngineOutEvents } from '../events/EngineOutEvents';
import { NavigationEvents } from '../navigation/Navigation';
import { PlaneState, Vars } from 'xplane-js/data';

export abstract class Phase {
  constructor(
    protected readonly bus: Bus,
    protected readonly pressureAltitude: Gettable<number | null>,
  ) {}
  // eslint-disable-next-line no-empty-function
  init(): void {
    /* prototype function */
  }

  abstract shouldActivateNextPhase(time: any): boolean;

  nextPhase: FmgcFlightPhase;
}

export class PreFlightPhase extends Phase {
  takeoffConfirmation = new ConfirmationNode(0.2 * 1000);

  init() {
    this.nextPhase = FmgcFlightPhase.Takeoff;
  }

  shouldActivateNextPhase(_deltaTime) {
    this.takeoffConfirmation.input = conditionTakeOff();
    this.takeoffConfirmation.update(_deltaTime);
    return this.takeoffConfirmation.output;
  }
}

export class TakeOffPhase extends Phase {
  accelerationAltitudeMsl: number;

  accelerationAltitudeMslEo: number;

  readonly fmAccelerationAltitude = Arinc429Register.empty();

  readonly fmEoAccelerationAltitude = Arinc429Register.empty();

  private readonly sub = this.bus.inbox<EngineOutEvents & NavigationEvents>();

  private readonly isEngineOutCondition = BusValue.from(this.sub.topic('fms_engine_out_active'), false);

  private readonly baroCorrectedAlt = BusValue.from(this.sub.topic('fms_nav_baro_corrected_altitude'), null);

  private readonly cas = BusValue.from(this.sub.topic('fms_nav_computed_airspeed'), null);

  init() {
    this.nextPhase = FmgcFlightPhase.Climb;
    Vars.write('L:A32NX_COLD_AND_DARK_SPAWN', 'Bool', false);

    this.fmAccelerationAltitude.setFromSimVar('L:A32NX_FM1_ACC_ALT');
    this.accelerationAltitudeMsl = this.fmAccelerationAltitude.valueOr(
      Vars.read('fbw/INDICATED_ALTITUDE', 'feet') + parseInt(NXDataStore.getLegacy('CONFIG_ACCEL_ALT', '1500')),
    );
    this.fmEoAccelerationAltitude.setFromSimVar('L:A32NX_FM1_EO_ACC_ALT');
    this.accelerationAltitudeMslEo = this.fmEoAccelerationAltitude.valueOr(
      Vars.read('fbw/INDICATED_ALTITUDE', 'feet') + parseInt(NXDataStore.getLegacy('CONFIG_ACCEL_ALT', '1500')),
    );
  }

  shouldActivateNextPhase(_deltaTime) {
    const engineOut = this.isEngineOutCondition.get();
    const baroAlti = this.baroCorrectedAlt.get();

    if (baroAlti !== null) {
      if (engineOut) {
        const gd = Vars.read('L:A32NX_SPEEDS_GD', VarUnit.Number);
        const cas = this.cas.get();

        if (cas >= gd && baroAlti > this.accelerationAltitudeMslEo) {
          return true;
        }
      } else {
        return baroAlti > this.accelerationAltitudeMsl;
      }
    }

    const verticalMode = getAutopilotVerticalMode();

    return (
      verticalMode === VerticalMode.VS ||
      verticalMode === VerticalMode.FPA ||
      verticalMode === VerticalMode.ALT ||
      verticalMode === VerticalMode.ALT_CPT ||
      verticalMode === VerticalMode.ALT_CST ||
      verticalMode === VerticalMode.ALT_CST_CPT ||
      verticalMode === VerticalMode.OP_CLB ||
      verticalMode === VerticalMode.CLB
    );
  }
}

export class ClimbPhase extends Phase {
  init() {
    this.nextPhase = FmgcFlightPhase.Cruise;
  }

  shouldActivateNextPhase(_deltaTime) {
    const pressureAltitude = this.pressureAltitude.get();
    if (pressureAltitude === null) {
      return;
    }

    const cruiseFl = Vars.read('L:A32NX_AIRLINER_CRUISE_ALTITUDE', 'number') / 100;
    const fl = Math.round(pressureAltitude / 100);

    // If no cruise alt has been entered, cruiseFl is 0. We don't want to switch to cruise phase in that case
    return cruiseFl > 0 && fl >= cruiseFl;
  }
}

export class CruisePhase extends Phase {
  init() {
    // switch out of cruise phase is handled in FlightPhaseManager
    this.nextPhase = FmgcFlightPhase.Cruise;
  }

  shouldActivateNextPhase(_deltaTime) {
    return false;
  }
}

export class DescentPhase extends Phase {
  init() {
    this.nextPhase = FmgcFlightPhase.Approach;
  }

  shouldActivateNextPhase(_deltaTime) {
    const pressureAltitude = this.pressureAltitude.get();
    const fl = pressureAltitude === null ? null : Math.round(pressureAltitude / 100);
    const fcuSelFl = PlaneState.apAltitudeTarget('feet') / 100;
    const cruiseFl = Vars.read('L:A32NX_AIRLINER_CRUISE_ALTITUDE', 'number') / 100;

    if (fl !== null && cruiseFl > 0 && fl === cruiseFl && fcuSelFl === fl) {
      this.nextPhase = FmgcFlightPhase.Cruise;
      return true;
    }

    // APPROACH phase from DECEL pseudo waypoint case. This is decided by the new TS FMS.
    return !!Vars.read('L:A32NX_FM_ENABLE_APPROACH_PHASE', 'Bool');
  }
}

export class ApproachPhase extends Phase {
  landingConfirmation = new ConfirmationNode(30 * 1000);

  init() {
    Vars.write('L:A32NX_AIRLINER_TO_FLEX_TEMP', 'Number', 0);
    this.nextPhase = FmgcFlightPhase.Done;
  }

  shouldActivateNextPhase(_deltaTime) {
    if (getAutopilotVerticalMode() === VerticalMode.SRS_GA) {
      this.nextPhase = FmgcFlightPhase.GoAround;
      return true;
    }

    this.landingConfirmation.input = isOnGround();
    this.landingConfirmation.update(_deltaTime);
    return this.landingConfirmation.output || !isAnEngineOn();
  }
}

export class GoAroundPhase extends Phase {
  init() {
    Vars.write('L:A32NX_AIRLINER_TO_FLEX_TEMP', 'Number', 0);
    this.nextPhase = FmgcFlightPhase.GoAround;
  }

  shouldActivateNextPhase(_deltaTime) {
    // there is no automatic switch from this phase
    return false;
  }
}

export class DonePhase extends Phase {
  init() {
    Vars.write('L:A32NX_AIRLINER_TO_FLEX_TEMP', 'Number', 0);
    this.nextPhase = FmgcFlightPhase.Done;
  }

  shouldActivateNextPhase(_deltaTime) {
    // there is no automatic switch from this phase
    return false;
  }
}
