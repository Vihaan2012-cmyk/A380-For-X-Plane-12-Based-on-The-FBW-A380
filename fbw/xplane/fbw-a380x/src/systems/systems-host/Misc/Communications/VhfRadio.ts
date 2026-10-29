// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { Bus, BoardDevice, KeyControl, Signal, Readable } from 'xplane-js/ui';

import {
  Arinc429Register,
  Arinc429RegisterSubject,
  Arinc429SignStatusMatrix,
  Arinc429Word,
  FailuresConsumer,
  RadioUtils,
  VhfComIndices,
} from '@flybywiresim/fbw-sdk';
import { A380Failure } from '@failures';
import { Vars } from 'xplane-js/data';

/** A VHF Radio, controlling an MSFS COM radio. */
export class VhfRadio implements BoardDevice {
  private static readonly ARINC_CACHE = Arinc429Register.empty();

  private readonly failureKey =
    this.index === 3 ? A380Failure.Vhf3 : this.index === 2 ? A380Failure.Vhf2 : A380Failure.Vhf1;

  private readonly busARmp = this.index === VhfComIndices.Vhf1 || this.index === VhfComIndices.Vhf3 ? 1 : 2;
  private readonly busBRmp = this.index === VhfComIndices.Vhf1 || this.index === VhfComIndices.Vhf3 ? 2 : 1;
  private readonly comABusVar = `L:FBW_RMP${this.busARmp}_PRIMARY_VHF${this.index}_FREQUENCY`;
  private readonly comBBusVar = `L:FBW_RMP${this.busBRmp}_BACKUP_VHF${this.index}_FREQUENCY`;
  private readonly selectBusAVar = `L:FBW_VHF${this.index}_BUS_A`;
  private readonly frequencyOutVar = `L:FBW_VHF${this.index}_FREQUENCY`;
  private readonly tuneEvent = `COM${this.index === 1 ? '' : this.index}_RADIO_SET_HZ`;
  private readonly frequencyVar = `fbw/COM_ACTIVE_FREQUENCY_${this.index}`;
  private readonly msfsCircuitVar = `fbw/CIRCUIT_SWITCH_ON_${this.msfsCircuit}`;

  /** The current frequency. */
  private readonly frequencyTuned = Arinc429RegisterSubject.createEmpty();
  /** The frequency to send to the MSFS radio in hertz, or 0 to switch it off (preventing transmission/reception). */
  private readonly msfsRadioFrequency = Signal.of(0);
  private readonly isMsfsRadioOn = Signal.of(false);

  private keyEventManager?: KeyControl;

  /**
   * Ctor.
   * @param bus The instrument event bus.
   * @param index The VHF transceiver index (1-based).
   * @param msfsCircuit The MSFS circuit index for this transceiver (used to switch the radio off when it should not transmit/receive).
   * @param isPowered Whether the transceiver is powered.
   * @param failuresConsumer The failures consumer.
   */
  constructor(
    private readonly bus: Bus,
    private readonly index: VhfComIndices,
    private readonly msfsCircuit: number,
    private readonly isPowered: Readable<boolean>,
    private readonly failuresConsumer: FailuresConsumer,
  ) {}

  /** @inheritdoc */
  setup(): void {
    const msfsTuneSub = this.msfsRadioFrequency.watch(
      (v) => {
        if (v > 0) {
          this.keyEventManager?.fire(this.tuneEvent, true, v);
        } else {
          // turn off MSFS radio to prevent reception
          this.isMsfsRadioOn.set(false);
        }

        const isOn = Vars.read(this.msfsCircuitVar, 'boolean') > 0;
        if (v > 0 !== isOn) {
          Vars.write('K:ELECTRICAL_CIRCUIT_TOGGLE', 'number', this.msfsCircuit);
        }
      },
      true,
      true,
    );

    KeyControl.connect(this.bus).then((v) => {
      this.keyEventManager = v;
      msfsTuneSub.release(true);
    });

    this.frequencyTuned.watch((v) => {
      Arinc429Word.toSimVarValue(this.frequencyOutVar, v.value, v.ssm);
      console.log(
        'tuning VHF',
        this.index,
        RadioUtils.unpackVhfComFrequencyFromArincToHz(v.value) / 1_000_000,
        RadioUtils.debugFormatArincBcdData(v.value),
      );
    }, true);

    this.failuresConsumer.register(this.failureKey);
  }

  /** @inheritdoc */
  tick(): void {
    const failed = this.failuresConsumer.isActive(this.failureKey);

    if (this.isPowered.get() && !failed) {
      // read the hardwire bus select discrete input
      const useBusA = Vars.read(this.selectBusAVar, 'boolean') > 0;
      const frequencyWord = Vars.read(useBusA ? this.comABusVar : this.comBBusVar, 'number');
      const frequency = VhfRadio.ARINC_CACHE.set(frequencyWord);

      if (frequency.isNormalOperation() || frequency.isFunctionalTest()) {
        // Unfortunately MSFS doesn't have a BCD32 tuning event, and the BCD16 event can only support 25 kHz spacing, so we have to resort to Hz.
        const freqHz = RadioUtils.unpackVhfComFrequencyFromArincToHz(frequency.value);
        this.msfsRadioFrequency.set(freqHz);
      } else {
        this.msfsRadioFrequency.set(0);
      }

      if (this.isMsfsRadioOn.get()) {
        this.frequencyTuned.setValueSsm(
          RadioUtils.packBcd32VhfComFrequencyToArinc(Vars.read(this.frequencyVar, 'frequency bcd32')),
          Arinc429SignStatusMatrix.NormalOperation,
        );
      } else {
        this.frequencyTuned.setValueSsm(0, Arinc429SignStatusMatrix.NoComputedData);
      }
    } else {
      this.msfsRadioFrequency.set(0);
      this.frequencyTuned.setValueSsm(0, Arinc429SignStatusMatrix.FailureWarning);
    }
  }
}
