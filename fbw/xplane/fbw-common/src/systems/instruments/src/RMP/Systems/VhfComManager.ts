// @ts-strict-ignore
// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  BusSignal,
  Bus,
  SessionState,
  TopicsByIndex,
  BoardDevice,
  Writable,
  VarUnit,
  Signal,
  Bond,
  Waits,
} from 'xplane-js/ui';

import { Arinc429SignStatusMatrix, Arinc429Word, RadioUtils } from '@flybywiresim/fbw-sdk';
import { FrequencyMode, VhfComIndices } from '../../../../shared/src/RadioTypes';

import { RmpState, RmpStateControllerEvents } from './RmpStateControllerTypes';
import { RmpUtils } from '../RmpUtils';
import { SessionPhase, Vars } from 'xplane-js/data';

interface VhfComManagerIndexedDataEventsRoot {
  vhf_com_active_frequency: number | null;
  vhf_com_standby_frequency: number | null;
  vhf_com_active_mode: FrequencyMode | null;
  vhf_com_standby_mode: FrequencyMode | null;
}

/** VHF RMP internal state events. */
export type VhfComManagerDataEvents = TopicsByIndex<VhfComManagerIndexedDataEventsRoot, VhfComIndices>;

interface VhfComManagerIndexedControlEventsRoot {
  vhf_com_set_standby_frequency: number | null;
  vhf_com_set_standby_mode: FrequencyMode | null;
  vhf_com_swap_frequencies: unknown;
}

export type VhfComManagerControlEvents = TopicsByIndex<VhfComManagerIndexedControlEventsRoot, VhfComIndices>;

/** Manages the external interfaces to other RMPs and to the transceiver for one VHF COM radio. */
export class VhfComManager implements BoardDevice {
  // FIXME ATSU should supply this. Use SITA worldwide primary frequency until then.
  private static readonly DATA_FREQUENCY = 0x131_550_0;
  private static readonly EMERGENCY_FREQUENCY = 0x121_500_0;

  private readonly sub = this.bus.inbox<RmpStateControllerEvents & VhfComManagerControlEvents>();

  private readonly pub = this.bus.outbox<VhfComManagerDataEvents>();

  private readonly activeFrequency = Signal.of<number | null>(null);

  private readonly standbyFrequency = Signal.of<number | null>(null);

  private readonly activeMode = Signal.of<FrequencyMode>(
    this.index === 3 ? FrequencyMode.Data : FrequencyMode.Frequency,
  );

  private readonly standbyMode = Signal.of<FrequencyMode>(
    this.index === 3 ? FrequencyMode.Data : FrequencyMode.Frequency,
  );

  private readonly activeDataTopic: keyof VhfComManagerDataEvents = `vhf_com_active_frequency_${this.index}`;
  private readonly activeFrequencyLocalVar = `L:FBW_RMP_FREQUENCY_ACTIVE_${this.index}` as const;

  private readonly activeModeDataTopic: keyof VhfComManagerDataEvents = `vhf_com_active_mode_${this.index}`;
  private readonly activeModeLocalVar = `L:FBW_RMP_MODE_ACTIVE_${this.index}` as const;

  private readonly standbyDataTopic: keyof VhfComManagerDataEvents = `vhf_com_standby_frequency_${this.index}`;
  private readonly standbyFrequencyLocalVar = `L:FBW_RMP_FREQUENCY_STANDBY_${this.index}` as const;

  private readonly standbyModeDataTopic: keyof VhfComManagerDataEvents = `vhf_com_standby_mode_${this.index}`;
  private readonly standbyModeLocalVar = `L:FBW_RMP_MODE_STANDBY_${this.index}` as const;

  private readonly standbyControlTopic: keyof VhfComManagerControlEvents = `vhf_com_set_standby_frequency_${this.index}`;

  private readonly standbyModeControlTopic: keyof VhfComManagerControlEvents = `vhf_com_set_standby_mode_${this.index}`;

  private readonly swapControlTopic: keyof VhfComManagerControlEvents = `vhf_com_swap_frequencies_${this.index}`;

  private readonly subs: Bond[] = [
    this.activeFrequency.watch(this.onActiveFrequencyChanged.bind(this), false, true),
    this.standbyFrequency.watch(this.onStandbyFrequencyChanged.bind(this), false, true),
    this.activeMode.watch(this.onActiveModeChanged.bind(this), false, true),
    this.standbyMode.watch(this.onStandbyModeChanged.bind(this), false, true),
    this.sub.topic(this.standbyControlTopic).listen(this.setStandbyFrequency.bind(this)),
    this.sub.topic(this.standbyModeControlTopic).listen(this.setStandbyMode.bind(this)),
    this.sub.topic(this.swapControlTopic).listen(this.swapFrequencies.bind(this)),
  ];

  private readonly tuningVar: string;

  private readonly powerOffRelayVar?: string;

  private readonly powerOffRelayValue?: Writable<number>;

  private readonly powerOffRelaySub?: Bond;

  private readonly rmpState = BusSignal.from(this.sub.topic('rmp_state'), RmpState.OffStandby);

  constructor(
    private readonly bus: Bus,
    public readonly index: VhfComIndices,
    private readonly rmpIndex: 1 | 2 | 3,
  ) {
    let isPrimary = false;
    switch (this.index) {
      case VhfComIndices.Vhf1:
        isPrimary = this.rmpIndex === 1;
        break;
      case VhfComIndices.Vhf2:
        isPrimary = this.rmpIndex === 2;
        break;
      case VhfComIndices.Vhf3:
        isPrimary = this.rmpIndex === 1;
        break;
    }
    this.tuningVar = `L:FBW_RMP${this.rmpIndex}_${isPrimary ? 'PRIMARY' : 'BACKUP'}_VHF${this.index}_FREQUENCY`;
    if (isPrimary) {
      this.powerOffRelayVar = `L:FBW_RMP3_BACKUP_VHF${this.index}_FREQUENCY`;
      this.powerOffRelayValue = Signal.of(0);
      this.powerOffRelaySub = this.powerOffRelayValue.watch(
        (v) => Vars.write(this.tuningVar, 'string', v.toString()),
        false,
        true,
      );
    }
  }

  /** @inheritdoc */
  setup(): void {
    this.rmpState.watch((rmpState) => {
      console.log('rmpState', RmpState[rmpState]);
      if (rmpState === RmpState.On) {
        this.powerOffRelaySub?.hold();
      }

      for (const sub of this.subs) {
        if (rmpState === RmpState.On) {
          sub.release(true);
        } else {
          sub.hold();
        }
      }

      if (rmpState !== RmpState.On) {
        Arinc429Word.toSimVarValue(this.tuningVar, 0, Arinc429SignStatusMatrix.FailureWarning);
        this.powerOffRelayValue?.set(0);
        this.powerOffRelaySub?.release();
      }
    }, true);

    // Initial load from sim variables (flt file) on RMP 1 only
    if (this.rmpIndex === 1) {
      Waits.forValue(SessionState.get(), (v) => v === SessionPhase.flying, true).then(() => {
        // VHF3 default is data mode
        if (this.index === 3) {
          Vars.write(this.activeFrequencyLocalVar, 'Frequency BCD32', VhfComManager.DATA_FREQUENCY);
          Vars.write(this.activeModeLocalVar, VarUnit.Enum, FrequencyMode.Data);
        } else {
          Vars.write(
            this.activeFrequencyLocalVar,
            'Frequency BCD32',
            Vars.read(`fbw/COM_ACTIVE_FREQUENCY_${this.index}`, 'Frequency BCD32'),
          );
        }

        Vars.write(
          this.standbyFrequencyLocalVar,
          'Frequency BCD32',
          Vars.read(`fbw/COM_STANDBY_FREQUENCY_${this.index}`, 'Frequency BCD32'),
        );
      });
    }
  }

  /** @inheritdoc */
  tick(): void {
    if (this.rmpState.get() !== RmpState.On && this.powerOffRelayValue && this.powerOffRelayVar) {
      this.powerOffRelayValue.set(Vars.read(this.powerOffRelayVar, 'number'));
    }

    this.updateInterRmpBus();
  }

  private updateInterRmpBus(): void {
    this.activeFrequency.set(Vars.read(this.activeFrequencyLocalVar, 'Frequency BCD32'));
    this.activeMode.set(Vars.read(this.activeModeLocalVar, VarUnit.Enum));
    this.standbyFrequency.set(Vars.read(this.standbyFrequencyLocalVar, 'Frequency BCD32'));
    this.standbyMode.set(Vars.read(this.standbyModeLocalVar, VarUnit.Enum));
  }

  private onActiveModeChanged(mode: FrequencyMode): void {
    this.pub.post(this.activeModeDataTopic, mode);
  }

  private onActiveFrequencyChanged(frequency: number): void {
    if (frequency < 1) {
      Arinc429Word.toSimVarValue(this.tuningVar, 0, Arinc429SignStatusMatrix.NoComputedData);
    } else {
      Arinc429Word.toSimVarValue(
        this.tuningVar,
        RadioUtils.packBcd32VhfComFrequencyToArinc(frequency),
        Arinc429SignStatusMatrix.NormalOperation,
      );
    }
    this.pub.post(this.activeDataTopic, frequency > 0 ? frequency : null);
  }

  private onStandbyModeChanged(mode: FrequencyMode): void {
    this.pub.post(this.standbyModeDataTopic, mode);
  }

  private onStandbyFrequencyChanged(frequency: number | null): void {
    this.pub.post(this.standbyDataTopic, frequency > 0 ? frequency : null);
  }

  /**
   * Sets the active frequency.
   * @param frequency Frequency in BCD32.
   */
  public setActiveFrequency(frequency: number): void {
    if (this.rmpState.get() === RmpState.On) {
      Vars.write(this.activeFrequencyLocalVar, 'Frequency BCD32', frequency);
    }
  }

  /**
   * Sets the active mode.
   * @param mode New mode.
   */
  public setActiveMode(mode: FrequencyMode): void {
    if (this.rmpState.get() === RmpState.On) {
      Vars.write(this.activeModeLocalVar, VarUnit.Enum, mode);

      switch (mode) {
        case FrequencyMode.Data:
          Vars.write(this.activeFrequencyLocalVar, 'Frequency BCD32', VhfComManager.DATA_FREQUENCY);
          break;
        case FrequencyMode.Emergency:
          Vars.write(this.activeFrequencyLocalVar, 'Frequency BCD32', VhfComManager.EMERGENCY_FREQUENCY);
          break;
      }
    }
  }

  /**
   * Sets the standby frequency.
   * @param frequency Frequency in BCD32.
   */
  public setStandbyFrequency(frequency: number): void {
    if (this.rmpState.get() === RmpState.On) {
      Vars.write(this.standbyFrequencyLocalVar, 'Frequency BCD32', frequency);
    }
  }

  /**
   * Sets the standby mode.
   * @param mode New mode.
   */
  public setStandbyMode(mode: FrequencyMode): void {
    if (this.rmpState.get() === RmpState.On) {
      Vars.write(this.standbyModeLocalVar, VarUnit.Enum, mode);

      switch (mode) {
        case FrequencyMode.Data:
          Vars.write(this.standbyFrequencyLocalVar, 'Frequency BCD32', VhfComManager.DATA_FREQUENCY);
          break;
        case FrequencyMode.Emergency:
          Vars.write(this.standbyFrequencyLocalVar, 'Frequency BCD32', VhfComManager.EMERGENCY_FREQUENCY);
          break;
      }
    }
  }

  public swapFrequencies(): void {
    if (this.rmpState.get() === RmpState.On) {
      RmpUtils.swapVhfFrequency(this.index);
    }
  }
}
