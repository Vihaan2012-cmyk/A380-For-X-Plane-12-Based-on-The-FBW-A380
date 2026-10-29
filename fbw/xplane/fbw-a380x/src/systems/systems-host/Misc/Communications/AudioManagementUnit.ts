// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  BusSignal,
  Bus,
  BoardDevice,
  Derived,
  VarUnit,
  Signal,
  Readable,
} from 'xplane-js/ui';
import { RadioNavSelectedNavaid, RmpAmuBusEvents } from './RmpAmuBusPublisher';
import { FailuresConsumer, RegisteredSimVar, RmpState } from '@flybywiresim/fbw-sdk';
import { A380Failure } from '@failures';
import { Vars } from 'xplane-js/data';

export type AmuIndex = 1 | 2;

export enum ComIndex {
  Vhf1 = 1,
  Vhf2 = 2,
  Vhf3 = 3,
}

/**
 * An audio management unit that controls the MSFS audio systems.
 * AMU1 primarily covers the captain's side, while AMU2 covers the F/O's side.
 */
export class AudioManagementUnit implements BoardDevice {
  private readonly sub = this.bus.inbox<RmpAmuBusEvents>();

  private readonly onsideRmpIndex = this.index;
  private readonly offsideRmpIndex = this.index === 2 ? 1 : 2;

  private readonly onsideRmpStateLocalVar = RegisteredSimVar.create(
    `L:A380X_RMP_${this.onsideRmpIndex}_STATE`,
    VarUnit.Enum,
  );
  private readonly rmp3StateLocalVar = RegisteredSimVar.create(`L:A380X_RMP_3_STATE`, VarUnit.Enum);

  private readonly mainPowerVar = 'L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED';

  private readonly _isHealthy = Signal.of(false);
  public readonly isHealthy = this._isHealthy as Readable<boolean>;

  private readonly failureKey = this.index === 2 ? A380Failure.AudioManagementUnit2 : A380Failure.AudioManagementUnit1;

  private readonly onsideRmpSwitchedOn = Signal.of(false);
  private readonly rmp3SwitchedOn = Signal.of(false);

  private readonly activeRmpIndex = Derived.from(
    ([onsideHealthy, rmp3Healthy]) => (onsideHealthy ? this.onsideRmpIndex : rmp3Healthy ? 3 : this.offsideRmpIndex),
    this.onsideRmpSwitchedOn,
    this.rmp3SwitchedOn,
  );

  private readonly vhf1Receive = BusSignal.from(null, false);
  private readonly vhf2Receive = BusSignal.from(null, false);
  private readonly vhf3Receive = BusSignal.from(null, false);
  private readonly vhf1Transmit = BusSignal.from(null, false);
  private readonly vhf2Transmit = BusSignal.from(null, false);
  private readonly vhf3Transmit = BusSignal.from(null, false);
  private readonly vhf1Volume = BusSignal.from(null, 0);
  private readonly vhf2Volume = BusSignal.from(null, 0);
  private readonly vhf3Volume = BusSignal.from(null, 0);

  private readonly navaidReceive = BusSignal.from(null, false);
  private readonly navaidVoiceFilter = BusSignal.from(null, false);
  private readonly navaidVolume = BusSignal.from(null, 0);
  private readonly navaidSelected = BusSignal.from(null, RadioNavSelectedNavaid.Adf1);

  constructor(
    private readonly bus: Bus,
    private readonly index: AmuIndex,
    private readonly failuresConsumer: FailuresConsumer,
  ) {}

  /** @inheritdoc */
  setup(): void {
    this.activeRmpIndex.watch((rmpIndex) => {
      this.vhf1Receive.rebind(this.sub.topic(`rmp_amu_vhf1_receive_${rmpIndex}`));
      this.vhf2Receive.rebind(this.sub.topic(`rmp_amu_vhf2_receive_${rmpIndex}`));
      this.vhf3Receive.rebind(this.sub.topic(`rmp_amu_vhf3_receive_${rmpIndex}`));
      this.vhf1Transmit.rebind(this.sub.topic(`rmp_amu_vhf1_transmit_${rmpIndex}`));
      this.vhf2Transmit.rebind(this.sub.topic(`rmp_amu_vhf2_transmit_${rmpIndex}`));
      this.vhf3Transmit.rebind(this.sub.topic(`rmp_amu_vhf3_transmit_${rmpIndex}`));
      this.vhf1Volume.rebind(this.sub.topic(`rmp_amu_vhf1_volume_${rmpIndex}`));
      this.vhf2Volume.rebind(this.sub.topic(`rmp_amu_vhf2_volume_${rmpIndex}`));
      this.vhf3Volume.rebind(this.sub.topic(`rmp_amu_vhf3_volume_${rmpIndex}`));
      this.navaidReceive.rebind(this.sub.topic(`rmp_amu_nav_receive_${rmpIndex}`));
      this.navaidVoiceFilter.rebind(this.sub.topic(`rmp_amu_nav_filter_${rmpIndex}`));
      this.navaidVolume.rebind(this.sub.topic(`rmp_amu_nav_volume_${rmpIndex}`));
      this.navaidSelected.rebind(this.sub.topic(`rmp_amu_nav_sel_${rmpIndex}`));
    }, true);

    this.failuresConsumer.register(this.failureKey);
  }

  private static isRmpOn(state: RmpState): boolean {
    return state === RmpState.On || state === RmpState.OnFailed;
  }

  /** @inheritdoc */
  tick(): void {
    const failed = this.failuresConsumer.isActive(this.failureKey);
    const powered = Vars.read(this.mainPowerVar, VarUnit.Bool) > 0;
    this._isHealthy.set(!failed && powered);

    this.onsideRmpSwitchedOn.set(AudioManagementUnit.isRmpOn(this.onsideRmpStateLocalVar.get()));
    this.rmp3SwitchedOn.set(AudioManagementUnit.isRmpOn(this.rmp3StateLocalVar.get()));
  }

  public getActiveNavAudio(): RadioNavSelectedNavaid {
    return this.navaidSelected.get();
  }

  public isNavVoiceFiltered(): boolean {
    return this.navaidVoiceFilter.get();
  }

  public isNavOutputOn(): boolean {
    return this.navaidReceive.get();
  }

  public getNavVolume(): number {
    return this.navaidVolume.get();
  }

  public getSelectedComTransmitter(): ComIndex | null {
    switch (true) {
      case this.vhf1Transmit.get():
        return ComIndex.Vhf1;
      case this.vhf2Transmit.get():
        return ComIndex.Vhf2;
      case this.vhf3Transmit.get():
        return ComIndex.Vhf3;
      default:
        return null;
    }
  }

  public isComReceiveOn(index: ComIndex): boolean {
    switch (index) {
      case ComIndex.Vhf1:
        return this.vhf1Receive.get();
      case ComIndex.Vhf2:
        return this.vhf2Receive.get();
      case ComIndex.Vhf3:
        return this.vhf3Receive.get();
    }
  }

  public getComVolume(index: ComIndex): number {
    switch (index) {
      case ComIndex.Vhf1:
        return this.vhf1Volume.get();
      case ComIndex.Vhf2:
        return this.vhf2Volume.get();
      case ComIndex.Vhf3:
        return this.vhf3Volume.get();
    }
  }
}
