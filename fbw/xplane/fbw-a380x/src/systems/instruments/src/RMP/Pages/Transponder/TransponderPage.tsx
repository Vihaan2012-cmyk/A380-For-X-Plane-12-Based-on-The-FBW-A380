// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  Debouncer,
  View,
  Bus,
  UI,
  Signal,
  Bond,
  ViewNode,
} from 'xplane-js/ui';
import { RmpPage, RmpPageProps } from '../../Components/RmpPage';
import { TransponderController } from './TransponderController';
import { PageKeys } from '../../Systems/KeypadController';
import { RmpMessageControlEvents } from '../../Systems/RmpMessageManager';

import './TransponderPage.scss';

export interface TransponderPageProps extends RmpPageProps {
  bus: Bus;
}

export class TransponderPage extends View<TransponderPageProps> implements RmpPage {
  private static readonly CLR_HELD_TIME = 1000;

  private readonly clearHeldTimer = new Debouncer();

  private readonly isHidden = Signal.of(true);

  private readonly pub = this.props.bus.outbox<RmpMessageControlEvents>();

  private readonly controller = new TransponderController(this.props.bus);

  private readonly squawkMessageSub: Bond;

  constructor(props: TransponderPageProps) {
    super(props);

    this.squawkMessageSub = this.controller.isAuto.watch(
      (v) => this.pub.post('rmp_message_set_squawk_message', !v),
      false,
      true,
    );
  }

  public setActive(isActive: boolean): void {
    this.isHidden.set(!isActive);

    if (isActive) {
      this.controller.resume();
      this.squawkMessageSub.release(true);
    } else {
      this.controller.pause();
      this.squawkMessageSub.hold();
    }
  }

  public onKeyPressed(key: PageKeys): void {
    switch (key) {
      case PageKeys.Adk3:
        this.controller.onIdentPressed();
        break;
      case PageKeys.Lsk1:
      case PageKeys.Lsk2:
      case PageKeys.Lsk3:
        this.controller.onValidate();
        break;
      case PageKeys.Digit0:
        this.controller.onDigitEntered(0);
        break;
      case PageKeys.Digit1:
        this.controller.onDigitEntered(1);
        break;
      case PageKeys.Digit2:
        this.controller.onDigitEntered(2);
        break;
      case PageKeys.Digit3:
        this.controller.onDigitEntered(3);
        break;
      case PageKeys.Digit4:
        this.controller.onDigitEntered(4);
        break;
      case PageKeys.Digit5:
        this.controller.onDigitEntered(5);
        break;
      case PageKeys.Digit6:
        this.controller.onDigitEntered(6);
        break;
      case PageKeys.Digit7:
        this.controller.onDigitEntered(7);
        break;
      case PageKeys.Digit8:
        this.controller.onDigitEntered(8);
        break;
      case PageKeys.Digit9:
        this.controller.onDigitEntered(9);
        break;
      case PageKeys.Clear:
        this.controller.onClear(true);
        this.clearHeldTimer.arm(() => this.controller.onClear(false), TransponderPage.CLR_HELD_TIME);
        break;
    }
  }

  onKeyReleased(key: PageKeys): void {
    if (key === PageKeys.Clear) {
      this.clearHeldTimer.cancel();
    }
  }

  // FIXME _ missing in size 19 font

  render(): ViewNode | null {
    return (
      <div
        class={{
          hidden: this.isHidden,
          'transponder-page': true,
        }}
      >
        <div class="centred-row">
          <div
            class={{
              'transponder-mode': true,
              'font-11': true,
              green: this.controller.isAuto,
            }}
          >
            {this.controller.isAuto.derive((v) => (v ? 'AUTO' : 'STBY'))}
          </div>
        </div>
        <div class="centred-row">
          <div class="transponder-code">
            <div class="font-10">SQWK</div>
            <div
              class={{
                cyan: true,
                'font-19': this.controller.entryInProgress.derive((v) => !v),
                'font-16': this.controller.entryInProgress,
                amber: this.controller.entryInvalid,
              }}
            >
              {this.controller.displayedCode}
            </div>
          </div>
        </div>
        <div>
          <div class="transponder-ident font-11 cyan">
            {this.controller.isIdentActive.derive((v) => (v ? '\xa0IDENT' : '*IDENT'))}
          </div>
        </div>
      </div>
    );
  }
}
