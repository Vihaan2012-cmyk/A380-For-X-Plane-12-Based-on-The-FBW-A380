// @ts-strict-ignore
// Copyright (c) 2021-2024 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import {
  ApproachPhase,
  ClimbPhase,
  CruisePhase,
  DescentPhase,
  DonePhase,
  GoAroundPhase,
  Phase,
  PreFlightPhase,
  TakeOffPhase,
} from '@fmgc/flightphase/Phase';
import { Arinc429Word, ConfirmationNode } from '@flybywiresim/fbw-sdk';
import { VerticalMode } from '@shared/autopilot';
import { FmgcFlightPhase, isAnEngineOn, isOnGround, isReady, isSlewActive } from '@shared/flightphase';
import { BusValue, Bus, SessionState, VarUnit, Signal, Waits } from 'xplane-js/ui';
import { NavigationEvents } from '@fmgc/navigation/Navigation';
import { EngineOutEvents } from '../events/EngineOutEvents';
import { PlaneState, SessionPhase, Vars } from 'xplane-js/data';

export interface FlightPhaseManagerEvents {
  /** The FMGC flight phase. */
  fmgc_flight_phase: FmgcFlightPhase;
}

function canInitiateDes(distanceToDestination: number): boolean {
  const fl = Math.round(PlaneState.altitude() / 100);
  const fcuSelFl = PlaneState.apAltitudeTarget('feet') / 100;
  const cruiseFl = Vars.read('L:A32NX_AIRLINER_CRUISE_ALTITUDE', 'number') / 100;

  // Can initiate descent? OR Can initiate early descent?
  return (
    cruiseFl > 0 &&
    (((distanceToDestination < 200 || fl < 200) && fcuSelFl < cruiseFl && fcuSelFl < fl) ||
      (distanceToDestination >= 200 && fl > 200 && fcuSelFl <= 200))
  );
}

export class FlightPhaseManager {
  private readonly sub = this.bus.inbox<EngineOutEvents & NavigationEvents>();

  private readonly pressureAltitude = BusValue.from(this.sub.topic('fms_nav_pressure_altitude'), null);

  private readonly isEngineOutCondition = BusValue.from(this.sub.topic('fms_engine_out_active'), false);

  private onGroundConfirmationNode = new ConfirmationNode(30 * 1000);

  private readonly activePhase = Signal.of(FmgcFlightPhase.Preflight);

  private phases: { [key in FmgcFlightPhase]: Phase } = {
    [FmgcFlightPhase.Preflight]: new PreFlightPhase(this.bus, this.pressureAltitude),
    [FmgcFlightPhase.Takeoff]: new TakeOffPhase(this.bus, this.pressureAltitude),
    [FmgcFlightPhase.Climb]: new ClimbPhase(this.bus, this.pressureAltitude),
    [FmgcFlightPhase.Cruise]: new CruisePhase(this.bus, this.pressureAltitude),
    [FmgcFlightPhase.Descent]: new DescentPhase(this.bus, this.pressureAltitude),
    [FmgcFlightPhase.Approach]: new ApproachPhase(this.bus, this.pressureAltitude),
    [FmgcFlightPhase.GoAround]: new GoAroundPhase(this.bus, this.pressureAltitude),
    [FmgcFlightPhase.Done]: new DonePhase(this.bus, this.pressureAltitude),
  };

  private phaseChangeListeners: Array<(prev: FmgcFlightPhase, next: FmgcFlightPhase) => void> = [];

  get phase() {
    return this.activePhase.get();
  }

  constructor(private readonly bus: Bus) {}
  init(): void {
    console.log(`FMGC Flight Phase: ${this.phase}`);
    this.phases[this.phase].init();
    this.changePhase(this.phase);
    this.activePhase.watch((v) => this.bus.outbox<FlightPhaseManagerEvents>().post('fmgc_flight_phase', v), true);

    // For simulation purposes we need to handle loading or spawning in various states.
    // In here we should not rely on aircraft systems, but instead simvars from MSFS.
    Waits.forValue(SessionState.get(), (v) => v === SessionPhase.flying).then(() => {
      switch (true) {
        case Vars.read('fbw/GEAR_IS_ON_GROUND', VarUnit.Bool) > 0:
          this.changePhase(FmgcFlightPhase.Preflight);
          break;
        case Vars.read('sim/flightmodel/position/elevation', VarUnit.Feet) > 18_000:
          this.changePhase(FmgcFlightPhase.Cruise);
          break;
        case Vars.read('fbw/FLAPS_HANDLE_INDEX', VarUnit.Enum) > 0:
          this.changePhase(FmgcFlightPhase.Approach);
          break;
        default:
          this.changePhase(FmgcFlightPhase.Climb);
          break;
      }
    });
  }

  shouldActivateNextPhase(_deltaTime: number): void {
    // process transitions only when plane is ready
    if (isReady() && !isSlewActive()) {
      if (this.shouldActivateDonePhase(_deltaTime)) {
        this.changePhase(FmgcFlightPhase.Done);
      } else if (this.phases[this.phase].shouldActivateNextPhase(_deltaTime)) {
        this.changePhase(this.phases[this.phase].nextPhase);
      }
    } else if (isReady() && isSlewActive()) {
      this.handleSlewSituation(_deltaTime);
    }
  }

  /** @deprecated Use {@link FlightPhaseManagerEvents} instead. */ addOnPhaseChanged(
    cb: (prev: FmgcFlightPhase, next: FmgcFlightPhase) => void,
  ): void {
    this.phaseChangeListeners.push(cb);
  }

  handleFcuAltKnobPushPull(distanceToDestination: number): void {
    switch (this.phase) {
      case FmgcFlightPhase.Takeoff:
        this.changePhase(FmgcFlightPhase.Climb);
        break;
      case FmgcFlightPhase.Climb:
      case FmgcFlightPhase.Cruise:
        if (canInitiateDes(distanceToDestination)) {
          this.changePhase(FmgcFlightPhase.Descent);
        }
        break;
      default:
    }
  }

  handleFcuAltKnobTurn(distanceToDestination: number): void {
    if (this.phase === FmgcFlightPhase.Cruise) {
      const activeVerticalMode = Vars.read('L:A32NX_FMA_VERTICAL_MODE', 'Enum');
      const VS = Vars.read('L:A32NX_AUTOPILOT_VS_SELECTED', 'feet per minute');
      const FPA = Vars.read('L:A32NX_AUTOPILOT_FPA_SELECTED', 'Degrees');
      if (
        (activeVerticalMode === VerticalMode.OP_DES ||
          (activeVerticalMode === VerticalMode.VS && VS < 0) ||
          (activeVerticalMode === VerticalMode.FPA && FPA < 0) ||
          activeVerticalMode === VerticalMode.DES) &&
        canInitiateDes(distanceToDestination)
      ) {
        this.changePhase(FmgcFlightPhase.Descent);
      }
    }
  }

  handleFcuVSKnob(distanceToDestination: number, onStepClimbDescent: () => void): void {
    if (this.phase === FmgcFlightPhase.Climb || this.phase === FmgcFlightPhase.Cruise) {
      /** a timeout of 100ms is required in order to receive the updated autopilot vertical mode */
      setTimeout(() => {
        const activeVerticalMode = Vars.read('L:A32NX_FMA_VERTICAL_MODE', 'Enum');
        const VS = Vars.read('L:A32NX_AUTOPILOT_VS_SELECTED', 'feet per minute');
        const FPA = Vars.read('L:A32NX_AUTOPILOT_FPA_SELECTED', 'Degrees');
        if (
          (activeVerticalMode === VerticalMode.VS && VS < 0) ||
          (activeVerticalMode === VerticalMode.FPA && FPA < 0)
        ) {
          if (canInitiateDes(distanceToDestination)) {
            this.changePhase(FmgcFlightPhase.Descent);
          } else {
            onStepClimbDescent();
          }
        }
      }, 100);
    }
  }

  handleNewCruiseAltitudeEntered(newCruiseFlightLevel: number): void {
    const pressureAltitude = this.pressureAltitude.get();
    const currentFlightLevel = pressureAltitude === null ? null : Math.round(pressureAltitude / 100);

    if (this.phase === FmgcFlightPhase.Approach) {
      this.changePhase(FmgcFlightPhase.Climb);
    } else if (
      currentFlightLevel !== null &&
      currentFlightLevel < newCruiseFlightLevel &&
      this.phase === FmgcFlightPhase.Descent
    ) {
      this.changePhase(FmgcFlightPhase.Climb);
    } else if (
      currentFlightLevel !== null &&
      currentFlightLevel > newCruiseFlightLevel &&
      (this.phase === FmgcFlightPhase.Climb || this.phase === FmgcFlightPhase.Descent)
    ) {
      this.changePhase(FmgcFlightPhase.Cruise);
    }
  }

  handleNewDestinationAirportEntered(): void {
    if (this.phase === FmgcFlightPhase.GoAround) {
      // FIXME: EO ACC ALT is advisory only.
      const accAlt = this.isEngineOutCondition.get()
        ? Arinc429Word.fromSimVarValue('L:A32NX_FM1_EO_ACC_ALT')
        : Arinc429Word.fromSimVarValue('L:A32NX_FM1_ACC_ALT');
      if (PlaneState.altitude() > accAlt.valueOr(0)) {
        this.changePhase(FmgcFlightPhase.Climb);
      }
    }
  }

  changePhase(newPhase: FmgcFlightPhase): void {
    const prevPhase = this.phase;
    console.log(`FMGC Flight Phase: ${prevPhase} => ${newPhase}`);
    this.activePhase.set(newPhase);
    Vars.write('L:A32NX_FMGC_FLIGHT_PHASE', 'number', newPhase);
    // Updating old SimVar to ensure backwards compatibility
    Vars.write(
      'L:AIRLINER_FLIGHT_PHASE',
      'number',
      newPhase < FmgcFlightPhase.Takeoff ? FmgcFlightPhase.Preflight : newPhase + 1,
    );

    this.phases[this.phase].init();

    for (const pcl of this.phaseChangeListeners) {
      pcl(prevPhase, newPhase);
    }

    this.shouldActivateNextPhase(0);
  }

  tryGoInApproachPhase(): boolean {
    if (
      this.phase === FmgcFlightPhase.Preflight ||
      this.phase === FmgcFlightPhase.Takeoff ||
      this.phase === FmgcFlightPhase.Done
    ) {
      return false;
    }

    if (this.phase !== FmgcFlightPhase.Approach) {
      this.changePhase(FmgcFlightPhase.Approach);
    }

    return true;
  }

  shouldActivateDonePhase(_deltaTime: number): boolean {
    this.onGroundConfirmationNode.input = isOnGround();
    this.onGroundConfirmationNode.update(_deltaTime);
    return (
      this.onGroundConfirmationNode.output &&
      !isAnEngineOn() &&
      this.phase !== FmgcFlightPhase.Done &&
      this.phase !== FmgcFlightPhase.Preflight
    );
  }

  handleSlewSituation(_deltaTime: number) {
    switch (this.phase) {
      case FmgcFlightPhase.Preflight:
      case FmgcFlightPhase.Takeoff:
      case FmgcFlightPhase.Done:
        if (PlaneState.heightAboveGround() >= 1500) {
          this.changePhase(FmgcFlightPhase.Climb);
        }
        break;
      default:
    }
  }
}
