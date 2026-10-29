// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  BusSignal,
  Debouncer,
  Bus,
  Derived,
  SignalRecord,
  Signal,
  Readable,
} from 'xplane-js/ui';

import {
  Arinc429LocalVarConsumerSubject,
  FrequencyMode,
  RadioChannelType,
  RadioUtils,
  VhfComIndices,
} from '@flybywiresim/fbw-sdk';
import { VhfComManagerControlEvents, VhfComManagerDataEvents } from '@flybywiresim/rmp';
import { RmpMessageControlEvents } from '../../Systems/RmpMessageManager';
import { AudioControlLocalVarEvents } from '../../Data/AudioControlPublisher';
import { VhfRadioEvents } from '../../Data/VhfRadioPublisher';
import { FlasherEvents } from '../..//Systems/Flasher';

export enum ReceptionMode {
  Off,
  Receive,
  TransmitOnly,
}

export const StandbyModeTitles: Record<FrequencyMode, string> = {
  [FrequencyMode.Frequency]: 'STBY',
  [FrequencyMode.Data]: 'STBY',
  [FrequencyMode.Emergency]: 'EMER',
};

export interface StandbyDigit {
  digit: Readable<string>;
  isPilotEntered: Readable<boolean>;
}

// FIXME make all the data pausable when the page is not active

interface StandbyEntry {
  /** Entered characters. */
  entered: string;
  /** The number of digits the entry is shifted to the right if auto-completed, else 0. */
  entryOffset: number;
  /** Text displayed in the field. */
  displayed: string;
  /** BCD32 frequency, or 0 if no completed (auto or manual) entry yet. */
  frequency: number;
}

export class VhfComController {
  private static readonly EMPTY_FREQUENCY = '---.---';

  private static readonly EMPTY_STANDBY_ENTRY: StandbyEntry = {
    entered: '',
    entryOffset: 0,
    displayed: VhfComController.EMPTY_FREQUENCY,
    frequency: 0,
  };

  private static readonly TRANSMIT_ONLY_FLASH_TIME = 2_000;

  private readonly standbyControlTopic: keyof VhfComManagerControlEvents = `vhf_com_set_standby_frequency_${this.vhfIndex}`;

  private readonly standbyModeControlTopic: keyof VhfComManagerControlEvents = `vhf_com_set_standby_mode_${this.vhfIndex}`;

  private readonly swapControlTopic: keyof VhfComManagerControlEvents = `vhf_com_swap_frequencies_${this.vhfIndex}`;

  private readonly audioSub = this.bus.inbox<AudioControlLocalVarEvents>();
  private readonly vhfSub = this.bus.inbox<VhfComManagerDataEvents & VhfRadioEvents>();
  private readonly flashSub = this.bus.inbox<FlasherEvents>();
  private readonly vhfPub = this.bus.outbox<VhfComManagerControlEvents>();
  private readonly messagePub = this.bus.outbox<RmpMessageControlEvents>();

  private readonly flash1Hz = BusSignal.from(this.flashSub.topic('flash_1hz'), false);

  private readonly _activeFrequency = BusSignal.from<number | null>(
    this.vhfSub.topic(`vhf_com_active_frequency_${this.vhfIndex}`),
    null,
  );
  public activeFrequency = this._activeFrequency as Readable<number | null>;

  private readonly radioFrequency = Arinc429LocalVarConsumerSubject.create(
    this.vhfSub.topic(`vhf_radio_frequency_${this.vhfIndex}`),
    0,
  );

  private readonly _standbyFrequency = BusSignal.from<number | null>(
    this.vhfSub.topic(`vhf_com_standby_frequency_${this.vhfIndex}`),
    null,
  );
  public readonly standbyFrequency = this._standbyFrequency as Readable<number | null>;

  // FIXME use the delayed var
  private isReceiveOn = BusSignal.from(this.audioSub.topic(`vhf_receive_${this.vhfIndex}`), false);

  public isTransmitOn = BusSignal.from(this.audioSub.topic(`vhf_transmit_${this.vhfIndex}`), false);

  private readonly _receptionMode = Derived.from(
    ([rx, tx]) => {
      if (rx) {
        return ReceptionMode.Receive;
      }
      if (tx) {
        return ReceptionMode.TransmitOnly;
      }
      return ReceptionMode.Off;
    },
    this.isReceiveOn,
    this.isTransmitOn,
  );
  public receptionMode = this._receptionMode as Readable<ReceptionMode>;

  private readonly _standbyDigits = Array.from({ length: 6 }, () => ({
    digit: Signal.of('-'),
    isPilotEntered: Signal.of(false),
  }));
  public readonly standbyDigits = this._standbyDigits as StandbyDigit[];

  private readonly standbyEntry = SignalRecord.of<StandbyEntry>({ ...VhfComController.EMPTY_STANDBY_ENTRY });

  public readonly standbyEntryInProgress = Signal.of(false);
  private readonly isStandbyEntryInvalid = Signal.of(false);

  public isStandbyAmber = Derived.from(
    ([standbyFreq, isInvalid]) => isInvalid || standbyFreq === null,
    this.standbyFrequency,
    this.isStandbyEntryInvalid,
  ) as Readable<boolean>;

  private readonly activeMode = BusSignal.from(
    this.vhfSub.topic(`vhf_com_active_mode_${this.vhfIndex}`),
    this.vhfIndex === 3 ? FrequencyMode.Data : FrequencyMode.Frequency,
  );

  private readonly standbyModesAvailable =
    this.vhfIndex === 3
      ? [FrequencyMode.Data, FrequencyMode.Emergency, FrequencyMode.Frequency]
      : [FrequencyMode.Frequency];
  public readonly standbyMode = BusSignal.from(
    this.vhfSub.topic(`vhf_com_standby_mode_${this.vhfIndex}`),
    this.vhfIndex === 3 ? FrequencyMode.Data : FrequencyMode.Frequency,
  ) as Readable<FrequencyMode>;
  public readonly standbyModeTitle = this.standbyMode.derive((v) =>
    v === null ? '' : StandbyModeTitles[v],
  ) as Readable<string>;
  public readonly isStandbyModeDownNotAvailable = this.standbyMode.derive(
    (v) => v === null || v <= 0,
  ) as Readable<boolean>;
  public readonly isStandbyModeUpNotAvailable = this.standbyMode.derive(
    (v) => v === null || v >= this.standbyModesAvailable.length - 1,
  ) as Readable<boolean>;

  public isRadioFailed = this.radioFrequency.derive((v) => v.isFailureWarning()) as Readable<boolean>;

  public readonly activeText = Derived.from(
    ([rmpFrequency, mode, failed]) => {
      if (mode === FrequencyMode.Data) {
        return rmpFrequency === null || failed ? '----' : 'DATA';
      }
      return rmpFrequency === null || failed ? VhfComController.EMPTY_FREQUENCY : RadioUtils.formatBcd32(rmpFrequency);
    },
    this.activeFrequency,
    this.activeMode,
    this.isRadioFailed,
  ) as Readable<string>;

  private readonly transmitOnlyFlashing = Signal.of(false);

  private readonly transmitOnlyFlashingTimer = new Debouncer();

  public readonly isLoudspeakerHidden = Derived.from(
    ([receptionMode, transmitOnlyFlashing, flash1Hz]) =>
      receptionMode === ReceptionMode.Off ||
      (receptionMode === ReceptionMode.TransmitOnly && transmitOnlyFlashing && !flash1Hz),
    this.receptionMode,
    this.transmitOnlyFlashing,
    this.flash1Hz,
  ) as Readable<boolean>;

  public readonly transceiverName = `VHF${this.vhfIndex}`;

  constructor(
    private readonly bus: Bus,
    private readonly vhfIndex: VhfComIndices,
    private readonly isRowSelected: Readable<boolean>,
  ) {
    this.isStandbyEntryInvalid.watch(
      (v) => v && this.messagePub.post('rmp_message_set', `${this.transceiverName} FREQ NOT VALID`),
    );
    this.isRowSelected.watch((v) => !v && this.standbyEntry.set(VhfComController.EMPTY_STANDBY_ENTRY));

    Derived.from(this.standbyEntry, this.standbyFrequency).watch(([entry, frequency]) => {
      this.isStandbyEntryInvalid.set(
        entry.frequency !== 0 && entry.frequency !== 0x1100000 && !RadioUtils.isValidFrequency(entry.frequency),
      );
      const entryInProgress = entry.entered.length > 0;
      this.standbyEntryInProgress.set(entryInProgress);

      const displayString = entryInProgress
        ? entry.displayed
        : frequency === null
          ? '------'
          : RadioUtils.formatBcd32(frequency, '');
      for (let i = 0; i < 6; i++) {
        const displayDigit = this._standbyDigits[i];
        displayDigit.digit.set(displayString[i] ?? '_');
        displayDigit.isPilotEntered.set(
          entryInProgress && i >= entry.entryOffset && i < entry.entryOffset + entry.entered.length,
        );
      }
    });

    this.receptionMode.watch((mode) => {
      if (mode === ReceptionMode.TransmitOnly) {
        this.transmitOnlyFlashing.set(true);
        this.transmitOnlyFlashingTimer.arm(
          () => this.transmitOnlyFlashing.set(false),
          VhfComController.TRANSMIT_ONLY_FLASH_TIME,
        );
      }
    });
  }

  public onAdkPressed(): void {
    this.vhfPub.post(this.swapControlTopic, undefined, false, false);
  }

  public onLskPressed(): void {
    if (this.standbyEntry.get().entered.length < 1) {
      this.messagePub.post('rmp_message_set', `${this.transceiverName} FREQ NOT COMPLETED`);
      return;
    }

    // then check validity and send if valid
    const standbyFreq = this.standbyEntry.get().frequency;
    if (RadioUtils.isValidFrequency(standbyFreq)) {
      this.vhfPub.post(this.standbyControlTopic, standbyFreq);
    } else {
      this.messagePub.post('rmp_message_set', `${this.transceiverName} STBY REVERTED TO PREV ENTRY`);
    }
    this.standbyEntry.set(VhfComController.EMPTY_STANDBY_ENTRY);
  }

  /*

      17 = 17_.___ invalid amber "VHFx FREQ NOT VALID"
      18 = 118.000
      187 = 118.700
      1875 = 118.750
      8 = 118.000
      87 = 118.700
      875 = 118.750

      // TODO 13x.xxx not auto-completed?
      3 = 13_.___
      37 = 137.___ amber = "VHFx FREQ NOT VALID"

      pushing a digit does nothing when not valid

      CLR a digit = that digit and ones after go to _
      CLR all digits = "VHFx STBY REVERTED TO PREV ENTRY"
      entry cannot shift left with subsequent digits, but can shift right (e.g. 1 => 18)!!

    */

  public onDigitEntered(digit: number): void {
    if (this.standbyMode.get() !== FrequencyMode.Frequency) {
      return;
    }

    const standbyEntry = this.standbyEntry.get();
    // 1 = 1__.___
    if (standbyEntry.entered.length === 0 && digit === 1) {
      this.standbyEntry.set({
        entered: '1',
        entryOffset: 0,
        displayed: '1_____',
        frequency: 0,
      });
      return;
    }
    if (standbyEntry.entered.length >= 6 || this.isStandbyEntryInvalid.get()) {
      return;
    }

    const entered = `${standbyEntry.entered}${digit}`;
    let frequency = parseInt(entered, 16) << (4 + 4 * (6 - entered.length));

    // FIXME ensure entryOffset can never decrease
    if (RadioUtils.isValidFrequency(frequency) || entered.length === 6) {
      this.standbyEntry.set({
        entered,
        entryOffset: 0,
        displayed: entered.padEnd(6, '0'),
        frequency,
      });
      return;
    }

    const channelInfo = RadioUtils.CHANNEL_TYPES[RadioChannelType.VhfCom8_33_25];

    // there doesn't seem to be any auto-fill for leading 13
    // FIXME should we handle leading 13 similarly?
    if (entered.charAt(0) === '3') {
      frequency = 0x1000000 | (frequency >> 4);
      if (entered.length < 5 && frequency < channelInfo.max) {
        frequency = 0;
      }
      this.standbyEntry.set({
        entered,
        entryOffset: 1,
        displayed: `1${entered.padEnd(5, '_')}`,
        frequency,
      });
      return;
    }

    // try shift 1 right, filling leading 1, see if valid
    // then try shift 2 right, filling leading 11, see if valid
    for (let i = 0; i < 3; i++) {
      if (i > 0) {
        frequency >>>= 4;
        frequency |= 0x1000000;
        if (RadioUtils.isValidFrequency(frequency)) {
          this.standbyEntry.set({
            entered,
            entryOffset: i,
            displayed: RadioUtils.formatBcd32(frequency, ''),
            frequency,
          });
          return;
        }
      }
      // we need to handle the case where the entry could be valid with the correct channel spacing applied
      if (standbyEntry.entryOffset + entered.length === 5) {
        const channelMask = 0xff;
        for (const chan of channelInfo.channels) {
          if ((chan & 0xf00) !== (frequency & 0xf00)) {
            continue;
          }
          frequency &= ~channelMask;
          frequency |= chan;
          if (RadioUtils.isValidFrequency(frequency)) {
            this.standbyEntry.set({
              entered,
              entryOffset: i,
              displayed: RadioUtils.formatBcd32(frequency, ''),
              frequency,
            });
            return;
          }
        }
      }
    }

    frequency = parseInt(entered, 16) << (4 + 4 * (6 - entered.length));
    this.standbyEntry.set({
      entered,
      entryOffset: 0,
      displayed: RadioUtils.formatBcd32(frequency, '').slice(0, entered.length).padEnd(6, '_'),
      frequency,
    });
  }

  public onClear(singleDigit = true): void {
    const standbyEntry = this.standbyEntry.get().entered;
    if (standbyEntry.length === 0) {
      return;
    }
    this.standbyEntry.set(VhfComController.EMPTY_STANDBY_ENTRY);
    if (singleDigit && standbyEntry.length > 1) {
      // simply lovely hack
      for (let i = 0; i < standbyEntry.length - 1; i++) {
        this.onDigitEntered(parseInt(standbyEntry[i]));
      }
      const newEntry = this.standbyEntry.get();
      this.standbyEntry.set({
        displayed: newEntry.displayed.slice(0, newEntry.entryOffset + newEntry.entered.length).padEnd(6, '_'),
      });
    } else if (standbyEntry.length === 1) {
      this.messagePub.post('rmp_message_set', `${this.transceiverName} STBY REVERTED TO PREV ENTRY`);
    }
  }

  public onUpPressed(): void {
    const oldMode = this.standbyMode.get();
    if (this.isStandbyModeUpNotAvailable.get() || oldMode == null) {
      return;
    }

    this.standbyEntry.set(VhfComController.EMPTY_STANDBY_ENTRY);
    this.vhfPub.post(this.standbyModeControlTopic, oldMode + 1, false, false);
  }

  public onDownPressed(): void {
    const oldMode = this.standbyMode.get();
    if (this.isStandbyModeDownNotAvailable.get() || oldMode == null) {
      return;
    }

    this.standbyEntry.set(VhfComController.EMPTY_STANDBY_ENTRY);
    this.vhfPub.post(this.standbyModeControlTopic, oldMode - 1, false, false);
  }
}
