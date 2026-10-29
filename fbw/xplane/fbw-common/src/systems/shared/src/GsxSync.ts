// @ts-strict-ignore
// Copyright (c) 2024-2025 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  BusSignal,
  Debouncer,
  Bus,
  SessionState,
  BoardDevice,
  VarUnit,
  Waits,
} from 'xplane-js/ui';
import { GroundSupportEvents, GsxSimVarEvents, MsfsFlightModelEvents, NXDataStore } from '@flybywiresim/fbw-sdk';
import { SessionPhase, Vars } from 'xplane-js/data';

export enum GsxStates {
  FALSE = 0,
  DISCONNECTED = 0,
  TRUE = 1,
  CONNECTED = 1,
}

export enum GsxServiceStates {
  CALLABLE = 1, //service can be called
  UNVAILABLE = 2, //service is not available
  BYPASSED = 3, //service has been bypassed
  REQUESTED = 4, //service has been requested
  ACTIVE = 5, //service is being performed
  COMPLETED = 6, //service has been completed
}

enum RefuelRate {
  REAL = 0,
  FAST = 1,
  INSTANT = 2,
}

enum SyncServices {
  FUEL = 'GSX_FUEL_SYNC',
  PAYLOAD = 'GSX_PAYLOAD_SYNC',
  POWER = 'GSX_POWER_SYNC',
}

enum DoorTarget {
  CLOSED = 0,
  OPEN = 1,
}

enum PowerAction {
  NOOP = -1,
  DISCONNECT = 0,
  CONNECT = 1,
}

const DELAY_CARGO_CLOSE = 60000;
const DELAY_REFUEL_RESTART = 2000;
const DELAY_RATE_RESTORE = DELAY_REFUEL_RESTART + 1000;
const DELAY_FUEL_START = 500;
const DEFUEL_DIFF_TARGET_A320 = 125;
const DEFUEL_DIFF_TARGET_A380 = 2500;

abstract class GsxSync implements BoardDevice {
  protected readonly sub = this.bus.inbox<GroundSupportEvents & GsxSimVarEvents & MsfsFlightModelEvents>();
  protected readonly extPowerAvailStates = new Map<number, BusSignal<boolean>>();
  protected readonly couatlStarted = BusSignal.from(null, 0);
  protected readonly stateJetway = BusSignal.from(null, 1);
  protected readonly stateBoard = BusSignal.from(null, 1);
  protected readonly stateDeparture = BusSignal.from(null, 1);
  protected readonly stateGpu = BusSignal.from(null, 1);
  protected isRefuelActive = false;
  protected isDefuel = false;
  protected readonly fuelTimer = new Debouncer();
  protected readonly rateTimer = new Debouncer();
  protected desiredFuel = 0;
  protected readonly fuelHose = BusSignal.from(null, 0);
  protected readonly refuelStartedByUser = BusSignal.from(null, false);
  protected initialIngameFrame = false;

  constructor(
    protected readonly bus: Bus,
    protected readonly numberOfGPUs: number,
    protected readonly DEFUEL_DIFF_TARGET: number,
  ) {}

  public setup(): void {
    Waits.forValue(SessionState.get(), (state) => state === SessionPhase.flying, true).then(() => {
      this.initSubscriptions();
      this.initialIngameFrame = true;
    });
  }

  protected initSubscriptions(): void {
    for (let index = 1; index <= this.numberOfGPUs; index++) {
      const element = BusSignal.from(this.sub.topic(`ext_pwr_avail_${index}`), false);
      this.extPowerAvailStates.set(index, element);
    }
    this.couatlStarted.rebind(this.sub.topic('gsx_couatl_started'));
    this.couatlStarted.watch(this.onCouatlStarted.bind(this));
    this.stateJetway.rebind(this.sub.topic('gsx_jetway_state'));
    this.stateBoard.rebind(this.sub.topic('gsx_boarding_state'));
    this.stateDeparture.rebind(this.sub.topic('gsx_departure_state'));
    this.stateGpu.rebind(this.sub.topic('gsx_gpu_state'));
    this.fuelHose.rebind(this.sub.topic('gsx_fuelhose_connected'));
    this.refuelStartedByUser.rebind(this.sub.topic('a32nx_refuel_started_by_user'));

    this.fuelHose.watch(this.onFuelHoseConnected.bind(this));
    this.refuelStartedByUser.watch(this.onRefuelStartedByUser.bind(this));

    this.stateJetway.watch(this.evaluateGsxPowerSource.bind(this));
    this.stateGpu.watch(this.evaluateGsxPowerSource.bind(this));
    this.stateDeparture.watch(this.evaluateGsxPowerSource.bind(this));
  }

  public tick(): void {
    if (this.initialIngameFrame) {
      this.onCouatlStarted(Vars.readNow('L:FSDT_GSX_COUATL_STARTED', VarUnit.Number));
      this.initialIngameFrame = false;
    }
  }

  protected isSyncEnabled(service: SyncServices): boolean {
    return NXDataStore.getLegacy(service, '0') === '1';
  }

  protected setRefuelStartedState(state: boolean): void {
    Vars.write('L:A32NX_REFUEL_STARTED_BY_USR', VarUnit.Bool, state);
  }

  protected getRefuelRate(): RefuelRate {
    return Vars.readNow('L:A32NX_EFB_REFUEL_RATE_SETTING', VarUnit.Number);
  }

  protected setRefuelRate(refuelRate: RefuelRate): void {
    Vars.write('L:A32NX_EFB_REFUEL_RATE_SETTING', VarUnit.Number, refuelRate);
  }

  protected onCouatlStarted(state: number): void {
    if (state === GsxStates.TRUE) {
      //gsx-ism: we don't want gsx progressive fuel when the aircraft progressively refuels
      Vars.write('L:FSDT_GSX_SET_PROGRESS_REFUEL', VarUnit.Number, -1);
    }
  }

  protected abstract getFob(): number;

  protected needsDefuel(): boolean {
    return this.getFob() + this.DEFUEL_DIFF_TARGET >= this.desiredFuel;
  }

  protected abstract getDesiredFuel(): number;

  protected abstract setDesiredFuel(desiredFuel: number): void;

  protected onFuelHoseConnected(hose: number): void {
    if (!this.isSyncEnabled(SyncServices.FUEL)) {
      return;
    }

    //start De/Refuel when Hose is connected
    if (hose === GsxStates.CONNECTED && !this.refuelStartedByUser.get() && !this.isRefuelActive) {
      this.desiredFuel = this.getDesiredFuel();
      this.isDefuel = this.needsDefuel();

      //need an Fuel Increase later, so target is lowered for desired >= FOB Cases
      if (this.isDefuel) {
        if (this.desiredFuel >= this.DEFUEL_DIFF_TARGET) {
          this.setDesiredFuel(this.desiredFuel - this.DEFUEL_DIFF_TARGET);
        } else {
          this.setDesiredFuel(0);
        }
      }
      this.isRefuelActive = true;
      this.fuelTimer.arm(() => {
        this.setRefuelStartedState(true);
      }, DELAY_FUEL_START);
    }

    //something bad happend, hose disconnected while still refueling - cleanup
    if (hose === GsxStates.DISCONNECTED && (this.refuelStartedByUser.get() || this.isRefuelActive)) {
      if (this.isDefuel) {
        this.setDesiredFuel(this.desiredFuel);
      }

      this.isDefuel = false;
      this.desiredFuel = 0;
      this.isRefuelActive = false;
    }
  }

  protected onRefuelStartedByUser(refuel: boolean): void {
    if (!this.isSyncEnabled(SyncServices.FUEL)) {
      return;
    }

    //defuel has finished - instant refuel to desired to get the truck triggered to leave
    if (!refuel && this.fuelHose.get() === GsxStates.CONNECTED && this.isDefuel) {
      this.setDesiredFuel(this.desiredFuel);
      const refuelRate = this.getRefuelRate();
      this.setRefuelRate(RefuelRate.INSTANT);

      this.fuelTimer.arm(() => {
        this.setRefuelStartedState(true);
        this.isDefuel = false;
        this.desiredFuel = 0;
      }, DELAY_REFUEL_RESTART);

      this.rateTimer.arm(() => {
        this.setRefuelRate(refuelRate);
      }, DELAY_RATE_RESTORE);
    }

    if (!refuel) {
      this.isRefuelActive = false;
    }
  }

  protected evaluateGsxPowerSource(): void {
    if (!this.isSyncEnabled(SyncServices.POWER)) {
      return;
    }

    const extAvail = this.isExtPowerAvail();
    const jetwayState = this.stateJetway.get();
    const gpuState = this.stateGpu.get();
    const boardState = this.stateBoard.get();
    const departureState = this.stateDeparture.get();
    let action = PowerAction.NOOP;

    // external is available (as of internal State) - check if disconnect is needed
    if (extAvail) {
      // pushback was requested (when boarding not running)
      if (
        boardState !== GsxServiceStates.REQUESTED &&
        boardState !== GsxServiceStates.ACTIVE &&
        departureState >= GsxServiceStates.REQUESTED
      ) {
        action = PowerAction.DISCONNECT;
        // pushback is active (in it wasn't catched before)
      } else if (departureState === GsxServiceStates.ACTIVE) {
        action = PowerAction.DISCONNECT;
        // jetway and gpu both not connected
      } else if (
        jetwayState >= GsxServiceStates.CALLABLE &&
        jetwayState <= GsxServiceStates.BYPASSED &&
        gpuState !== GsxServiceStates.ACTIVE
      ) {
        action = PowerAction.DISCONNECT;
      }
      // external is not available - check if connect is needed
    } else if (
      //jetway or gpu connected
      (jetwayState >= GsxServiceStates.REQUESTED || gpuState === GsxServiceStates.ACTIVE) &&
      departureState < GsxServiceStates.REQUESTED
    ) {
      action = PowerAction.CONNECT;
    }

    if (action == PowerAction.CONNECT) {
      this.setExtPower(true);
    } else if (action == PowerAction.DISCONNECT) {
      this.setExtPower(false);
    }
  }

  protected isExtPowerAvail(): boolean {
    let state = false;
    for (let index = 1; index <= this.numberOfGPUs; index++) {
      state ||= this.extPowerAvailStates.get(index).get();
    }
    return state;
  }

  protected abstract setOvhdPushBtn(index: number, state: boolean): void;

  protected setExtPower(connect: boolean): void {
    for (let index = 1; index <= this.numberOfGPUs; index++) {
      Vars.write(`L:A32NX_EXT_PWR_AVAIL:${index}`, VarUnit.Bool, connect);
      if (!connect) {
        this.setOvhdPushBtn(index, connect);
      }
    }
  }
}

export class GsxSyncA32NX extends GsxSync {
  constructor(bus: Bus) {
    super(bus, 1, DEFUEL_DIFF_TARGET_A320);
  }

  protected getFob(): number {
    return Vars.readNow('L:A32NX_TOTAL_FUEL_QUANTITY', VarUnit.Number);
  }

  protected getDesiredFuel(): number {
    return Vars.readNow('L:A32NX_FUEL_LEFT_MAIN_DESIRED', VarUnit.Number);
  }

  protected setDesiredFuel(desiredFuel: number): void {
    Vars.write('L:A32NX_FUEL_LEFT_MAIN_DESIRED', VarUnit.Number, desiredFuel);
    Vars.write('L:A32NX_FUEL_RIGHT_MAIN_DESIRED', VarUnit.Number, desiredFuel);
  }

  protected setOvhdPushBtn(index: number, state: boolean): void {
    if (index === 1) {
      Vars.write(`L:A32NX_OVHD_ELEC_EXT_PWR_PB_IS_ON`, VarUnit.Bool, state);
    }
  }
}

export class GsxSyncA380X extends GsxSync {
  protected readonly toggleCargo1 = BusSignal.from(null, 0);
  protected readonly toggleCargo2 = BusSignal.from(null, 0);
  protected readonly stateDeboard = BusSignal.from(null, 1);
  protected readonly cargoPercentBoard = BusSignal.from(null, 0);
  protected readonly cargoPercentDeboard = BusSignal.from(null, 0);
  protected readonly cargoTimer = new Debouncer();
  protected readonly cargoDoorTarget = BusSignal.from(null, 0);

  constructor(bus: Bus) {
    super(bus, 4, DEFUEL_DIFF_TARGET_A380);
  }

  protected initSubscriptions(): void {
    super.initSubscriptions();
    this.toggleCargo1.rebind(this.sub.topic('gsx_aircraft_cargo_1_toggle'));
    this.toggleCargo2.rebind(this.sub.topic('gsx_aircraft_cargo_2_toggle'));
    this.stateDeboard.rebind(this.sub.topic('gsx_deboarding_state'));
    this.cargoPercentBoard.rebind(this.sub.topic('gsx_boarding_cargo_percent'));
    this.cargoPercentDeboard.rebind(this.sub.topic('gsx_deboarding_cargo_percent'));
    this.cargoDoorTarget.rebind(this.sub.topic('msfs_interactive_point_goal_16'));

    this.toggleCargo1.watch(this.onToggleCargo.bind(this));
    this.toggleCargo2.watch(this.onToggleCargo.bind(this));
    this.stateBoard.watch(this.onBoardingCompleted.bind(this));
    this.stateDeboard.watch(this.onBoardingCompleted.bind(this));
    this.cargoPercentBoard.watch(this.onCargoBoardCompleted.bind(this));
    this.cargoPercentDeboard.watch(this.onCargoDeboardCompleted.bind(this));
    this.stateDeparture.watch(this.onPushbackActive.bind(this));
  }

  protected getFob(): number {
    return Vars.readNow('L:A32NX_TOTAL_FUEL_QUANTITY', VarUnit.Number);
  }

  protected getDesiredFuel(): number {
    return Vars.readNow('L:A32NX_FUEL_DESIRED', 'kilograms');
  }

  protected setDesiredFuel(desiredFuel: number): void {
    Vars.write('L:A32NX_FUEL_DESIRED', 'kilograms', desiredFuel);
  }

  protected setOvhdPushBtn(index: number, state: boolean): void {
    Vars.write(`L:A32NX_OVHD_ELEC_EXT_PWR_${index}_PB_IS_ON`, VarUnit.Bool, state);
  }

  protected isCargoOpen(): boolean {
    return this.cargoDoorTarget.get() !== DoorTarget.CLOSED;
  }

  protected setCargoDoor(goal: number, index: number = 16): void {
    Vars.write(`A:fbw/INTERACTIVE_POINT_GOAL_${index}`, VarUnit.PercentOver100, goal);
  }

  protected onToggleCargo(toggle: number) {
    //received first toggle from gsx - only used for opening the doors
    if (toggle !== GsxStates.FALSE && !this.isCargoOpen()) {
      this.setCargoDoor(DoorTarget.OPEN);
    }
  }

  protected onCargoBoardCompleted(percent: number): void {
    this.startCargoTimer(percent, DELAY_CARGO_CLOSE);
  }

  protected onCargoDeboardCompleted(percent: number): void {
    this.startCargoTimer(percent, DELAY_CARGO_CLOSE);
  }

  protected startCargoTimer(percent: number, delay: number): void {
    //close doors with a certain delay, after (un)loading was completed (==100%)
    if (percent === 100 && !this.cargoTimer.waiting() && this.isCargoOpen()) {
      this.fuelTimer.arm(() => {
        this.setCargoDoor(DoorTarget.CLOSED);
      }, delay);
    }
  }

  protected onBoardingCompleted(state: number): void {
    //just to be safe, check again when boarding was completed
    if (state === GsxServiceStates.COMPLETED && this.isCargoOpen()) {
      this.setCargoDoor(DoorTarget.CLOSED);
    }
  }

  protected onPushbackActive(pushState: number): void {
    //just to be safe, check again when pushback is active - maybe something bad happened on boarding
    if (
      this.isCargoOpen() &&
      (pushState === GsxServiceStates.ACTIVE ||
        (pushState === GsxServiceStates.REQUESTED && this.stateBoard.get() !== GsxServiceStates.ACTIVE))
    ) {
      this.setCargoDoor(DoorTarget.CLOSED);
    }
  }
}
