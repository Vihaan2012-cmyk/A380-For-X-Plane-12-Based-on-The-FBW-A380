// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { Bus, BoardDevice, KeyControl, Derived, VarUnit, Signal } from 'xplane-js/ui';
import { RmpState, RmpStateControllerEvents } from '@flybywiresim/rmp';
import { FailuresConsumer } from '@flybywiresim/fbw-sdk';
import { A380Failure } from '@failures';
import { Vars } from 'xplane-js/data';

/** A controller to manage the power and init state of the RMP. */
export class A380xRmpStateController implements BoardDevice {
  private readonly pub = this.bus.outbox<RmpStateControllerEvents>();

  private readonly state = Signal.of(RmpState.OffStandby);
  private readonly stateVar = `L:A380X_RMP_${this.rmpIndex}_STATE`;

  private readonly dcPowerVar: string;

  private readonly brightnessKnobVar = `L:A380X_RMP_${this.rmpIndex}_BRIGHTNESS_KNOB`;

  private readonly failureKey: number;

  private readonly selectBusAVars: string[];

  private readonly failedLedVar = `L:A380X_RMP_${this.rmpIndex}_RED_LED`;

  private readonly standbyLedVar = `L:A380X_RMP_${this.rmpIndex}_GREEN_LED`;

  private readonly failuresConsumer = new FailuresConsumer();

  private readonly rmp3BusA = Signal.of(false);

  private readonly rmp3BusAVar = 'L:FBW_RMP3_BUS_A';

  private readonly screenPotentiometer: number;

  private readonly screenBrightness = Signal.of(0);

  private readonly busAOutput = Derived.from(
    ([state, rmp3BusA]) => {
      switch (state) {
        case RmpState.On:
        case RmpState.OnFailed:
          return true;
        default:
          return rmp3BusA;
      }
    },
    this.state,
    this.rmp3BusA,
  );

  constructor(
    private readonly bus: Bus,
    private readonly rmpIndex: 1 | 2 | 3,
  ) {
    switch (rmpIndex) {
      case 1:
        this.dcPowerVar = 'L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED';
        this.failureKey = A380Failure.RadioManagementPanel1;
        this.selectBusAVars = ['L:FBW_VHF1_BUS_A', 'L:FBW_VHF3_BUS_A', 'L:FBW_HF1_BUS_A'];
        this.screenPotentiometer = 80;
        break;
      case 2:
        this.dcPowerVar = 'L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED';
        this.failureKey = A380Failure.RadioManagementPanel2;
        this.selectBusAVars = ['L:FBW_VHF2_BUS_A', 'L:FBW_HF2_BUS_A'];
        this.screenPotentiometer = 81;
        break;
      case 3:
        this.dcPowerVar = 'L:A32NX_ELEC_DC_1_BUS_IS_POWERED';
        this.failureKey = A380Failure.RadioManagementPanel3;
        this.selectBusAVars = ['L:FBW_RMP3_BUS_A'];
        this.screenPotentiometer = 82;
        break;
    }
  }

  /** @inheritdoc */
  setup(): void {
    this.state.watch((v) => {
      this.pub.post('rmp_state', v);

      Vars.write(
        this.failedLedVar,
        VarUnit.Bool,
        v === RmpState.OffFailed || v === RmpState.OnFailed,
      );
      Vars.write(this.standbyLedVar, VarUnit.Bool, v === RmpState.OffStandby);
      Vars.write(this.stateVar, VarUnit.Enum, v);
    }, true);

    this.failuresConsumer.register(this.failureKey);

    this.busAOutput.watch((v) => {
      for (const lVar of this.selectBusAVars) {
        Vars.write(lVar, VarUnit.Bool, v);
      }
    });

    KeyControl.connect(this.bus).then((manager) => {
      this.screenBrightness.watch(
        (brightness) => manager.fire('LIGHT_POTENTIOMETER_SET', true, this.screenPotentiometer, brightness),
        true,
      );
    });
  }

  /** @inheritdoc */
  tick(): void {
    const powerOn = Vars.read(this.dcPowerVar, VarUnit.Bool) > 0;
    const brightnessSetting = Vars.read(this.brightnessKnobVar, VarUnit.Number);
    const brightnessOn = brightnessSetting > 0;
    this.failuresConsumer.update();
    const failed = this.failuresConsumer.isActive(this.failureKey);

    if (powerOn) {
      if (failed) {
        this.state.set(brightnessOn ? RmpState.OnFailed : RmpState.OffFailed);
      } else {
        this.state.set(brightnessOn ? RmpState.On : RmpState.OffStandby);
      }
    } else {
      this.state.set(brightnessOn ? RmpState.OnFailed : RmpState.OffFailed);
    }

    if (this.rmpIndex !== 3) {
      this.rmp3BusA.set(Vars.read(this.rmp3BusAVar, VarUnit.Bool) > 0);
    }

    this.screenBrightness.set(this.state.get() === RmpState.On ? Math.max(brightnessSetting, 5) : 0);
  }
}
