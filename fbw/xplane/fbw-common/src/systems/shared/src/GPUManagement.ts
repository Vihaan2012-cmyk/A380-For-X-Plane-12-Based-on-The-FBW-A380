// @ts-strict-ignore
// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  BusSignal,
  Bus,
  SessionState,
  BoardDevice,
  Derived,
  VarUnit,
  Signal,
  Waits,
} from 'xplane-js/ui';
import { GroundSupportEvents, MsfsElectricsEvents, MsfsFlightModelEvents, MsfsMiscEvents } from '@flybywiresim/fbw-sdk';
import { SessionPhase, Vars } from 'xplane-js/data';

export class GPUManagement implements BoardDevice {
  private readonly sub = this.bus.inbox<
    MsfsElectricsEvents & MsfsFlightModelEvents & MsfsMiscEvents & GroundSupportEvents & GPUControlEvents
  >();

  private readonly gpuDoorOpenPercent = BusSignal.from(
    this.sub.topic(`msfs_interactive_point_open_${this.gpuDoorIndex}`),
    0,
  );
  private readonly gpuHookedUp = Derived.from(
    ([gpuDoorOpenPercent]) => gpuDoorOpenPercent >= 1,
    this.gpuDoorOpenPercent,
  );

  private readonly groundVelocity = BusSignal.from(this.sub.topic('msfs_ground_velocity'), 0);

  private readonly isInAircraft = Signal.of(0);

  private readonly msfsExtPowerAvailStates = new Map<number, BusSignal<boolean>>();

  private readonly ExtPowerAvailStates = new Map<number, BusSignal<boolean>>();

  private readonly isIngame = Derived.from(
    ([gameState, isInAircraft]) => gameState === SessionPhase.flying && isInAircraft === 1,
    SessionState.get(),
    this.isInAircraft,
  );

  private initialIngameFrame: boolean;
  constructor(
    private readonly bus: Bus,
    private readonly gpuDoorIndex: number,
    private readonly numberOfGPUs: number,
  ) {
    for (let index = 1; index <= numberOfGPUs; index++) {
      const element = BusSignal.from(this.sub.topic(`msfs_external_power_available_${index}`), false);
      this.msfsExtPowerAvailStates.set(index, element);
    }

    for (let index = 1; index <= numberOfGPUs; index++) {
      const element = BusSignal.from(this.sub.topic(`ext_pwr_avail_${index}`), false);
      this.ExtPowerAvailStates.set(index, element);
    }
  }

  public setup(): void {
    Waits.forValue(this.isIngame, (state) => state, true).then(() => {
      this.sub.topic('gpu_toggle').listen(this.toggleGPU.bind(this));
      this.gpuHookedUp.watch((v) => this.setEXTpower(v));
      this.groundVelocity.watch((v) => {
        // disable ext power when aircraft starts moving
        if (v > 0.3 && this.anyGPUAvail()) {
          this.toggleGPU();
        }
      });
      this.initialIngameFrame = true;
    });
  }

  public tick(): void {
    if (this.initialIngameFrame) {
      if (this.anyMSFSGPUAvail()) {
        this.setEXTpower(true);
      }
      this.initialIngameFrame = false;
    } else {
      this.isInAircraft.set(Vars.read('E:fbw/IS_AIRCRAFT', 'enum'));
    }
  }

  private toggleGPU(): void {
    if (!this.anyGPUAvail()) {
      if (this.anyMSFSGPUAvail()) {
        // if msfs ground power is avail we are at a powered stand
        this.setEXTpower(true);
      } else {
        this.toggleMSFSGpu(); // if msfs ground power is not avail we call for gpu cart
      }
    } else {
      if (this.gpuHookedUp.get()) {
        this.toggleMSFSGpu();
      } else {
        this.setEXTpower(false);
      }
    }
  }

  private toggleMSFSGpu(): void {
    Vars.write('K:REQUEST_POWER_SUPPLY', 'Bool', true);
  }

  private setEXTpower(connect: boolean): void {
    for (let index = 1; index <= this.numberOfGPUs; index++) {
      Vars.write(`L:A32NX_EXT_PWR_AVAIL:${index}`, VarUnit.Bool, connect);
      if (!connect) {
        if (this.numberOfGPUs === 1) {
          // provision for the a32nx
          Vars.write(`L:A32NX_OVHD_ELEC_EXT_PWR_PB_IS_ON`, VarUnit.Bool, false);
        } else {
          Vars.write(`L:A32NX_OVHD_ELEC_EXT_PWR_${index}_PB_IS_ON`, VarUnit.Bool, false);
        }
      }
    }
  }

  private anyMSFSGPUAvail(): boolean {
    let state = false;
    for (let index = 1; index <= this.numberOfGPUs; index++) {
      state ||= this.msfsExtPowerAvailStates.get(index).get();
    }
    return state;
  }

  private anyGPUAvail(): boolean {
    let state = false;
    for (let index = 1; index <= this.numberOfGPUs; index++) {
      state ||= this.ExtPowerAvailStates.get(index).get();
    }
    return state;
  }
}

export interface GPUControlEvents {
  /** event to toggle the GPU*/
  gpu_toggle: unknown;
}
