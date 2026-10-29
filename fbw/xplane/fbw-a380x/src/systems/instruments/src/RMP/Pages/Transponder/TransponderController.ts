// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  BusSignal,
  Debouncer,
  Bus,
  Derived,
  VarUnit,
  Signal,
  Readable,
  Bond,
} from 'xplane-js/ui';
import { TransponderEvents } from '../../Data/TransponderPublisher';
import { RmpMessageControlEvents } from '../../Systems/RmpMessageManager';
import { Vars } from 'xplane-js/data';

// FIXME make all the data pausable when the page isn't active

export class TransponderController {
  /** IDENT inhibit time in ms. */
  private static readonly IDENT_TIME = 5_000;
  /** Time after entering last digit before automatic validation occurs. */
  private static readonly AUTO_VALIDATE_TIME = 1_000;

  private readonly sub = this.bus.inbox<TransponderEvents>();

  private readonly messagePub = this.bus.outbox<RmpMessageControlEvents>();

  private readonly activeCode = BusSignal.from(this.sub.topic('transponder_code_1'), null);

  private readonly enteredCode = Signal.of<string | null>(null);

  private readonly _entryInvalid = Signal.of(false);
  public readonly entryInvalid = this._entryInvalid as Readable<boolean>;

  public readonly entryInProgress = this.enteredCode.derive((v) => v !== null) as Readable<boolean>;

  private readonly autoValidateTimer = new Debouncer();
  private readonly onValidateHandler = this.onValidate.bind(this);

  public readonly displayedCode = Derived.from(
    ([activeCode, enteredCode]) =>
      enteredCode !== null ? enteredCode : activeCode?.toString(16).padStart(4, '0') ?? '----',
    this.activeCode,
    this.enteredCode,
  ) as Readable<string>;

  private readonly _isAuto = BusSignal.from(this.sub.topic('transponder_auto_1'), false);
  public readonly isAuto = this._isAuto as Readable<boolean>;

  // FIXME need to sync this state over the RMP bus?
  private readonly _isIdentActive = Signal.of(false);
  public readonly isIdentActive = this._isIdentActive as Readable<boolean>;

  private readonly identTimer = new Debouncer();

  private readonly identMessageSub: Bond;

  constructor(private readonly bus: Bus) {
    this.identMessageSub = this.isIdentActive.watch(
      (v) =>
        v
          ? this.messagePub.post('rmp_message_set', 'TRANSMITTING IDENT')
          : this.messagePub.post('rmp_message_clear_message', 'TRANSMITTING IDENT'),
      true,
      true,
    );

    this._entryInvalid.watch((v) => v && this.messagePub.post('rmp_message_set', 'SQUAWK CODE NOT VALID'));

    this.enteredCode.watch(() => this._entryInvalid.set(false));
  }

  public pause(): void {
    this.enteredCode.set(null);
    this.identMessageSub.hold();
  }

  public resume(): void {
    this.identMessageSub.release(false);
  }

  public onIdentPressed(): void {
    if (this._isIdentActive.get()) {
      return;
    }

    Vars.write('K:XPNDR_IDENT_ON', VarUnit.Bool, true);
    this.identTimer.cancel();
    this._isIdentActive.set(true);
    this.identTimer.arm(() => this._isIdentActive.set(false), TransponderController.IDENT_TIME);
  }

  public onClear(singleDigit = true): void {
    this.autoValidateTimer.cancel();
    const oldCode = this.enteredCode.get();
    if (oldCode !== null) {
      if (!singleDigit) {
        this.enteredCode.set(null);
      } else if (oldCode.indexOf('_') > 1) {
        this.enteredCode.set(oldCode.slice(0, oldCode.length - 2).padEnd(4, '_'));
      } else {
        this.enteredCode.set(null);
        this.messagePub.post('rmp_message_set', 'SQUAWK CODE REVERTED TO PREV ENTRY');
      }
    }
  }

  public onDigitEntered(digit: number): void {
    this.autoValidateTimer.cancel();
    const enteredCode = this.enteredCode.get();
    if (this._entryInvalid.get() || (enteredCode !== null && enteredCode.indexOf('_') === -1)) {
      return;
    }

    if (enteredCode === null) {
      this.enteredCode.set(`${digit}___`);
    } else {
      const enteredDigits = `${enteredCode.slice(0, enteredCode.indexOf('_'))}${digit}`;
      this.enteredCode.set(enteredDigits.padEnd(4, '_'));
      if (enteredDigits.length === 4) {
        this.autoValidateTimer.arm(this.onValidateHandler, TransponderController.AUTO_VALIDATE_TIME);
      }
    }

    if (digit === 8 || digit === 9) {
      this._entryInvalid.set(true);
    }
  }

  public onValidate(): void {
    this.autoValidateTimer.cancel();
    const newCode = this.enteredCode.get();
    if (newCode === null) {
      return;
    }

    const notValid = this._entryInvalid.get() || newCode.indexOf('_') !== -1;
    if (notValid) {
      this.messagePub.post('rmp_message_set', 'SQUAWK CODE REVERTED TO PREV ENTRY');
      this.enteredCode.set(null);
      return;
    }

    const codeBcd = parseInt(newCode, 16);

    Vars.write('K:XPNDR_SET', 'number', codeBcd);

    this.enteredCode.set(null);
  }
}
