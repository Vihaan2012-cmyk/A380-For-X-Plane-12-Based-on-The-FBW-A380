// Copyright (c) 2021-2024 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import {
  UI,
  View,
  ViewNode,
  Signal,
  Derived,
  Bus,
  Readable,
} from 'xplane-js/ui';

import { EfisNdMode, TcasWxrMessage } from '@flybywiresim/fbw-sdk';
import { Layer } from '../MsfsAvionicsCommon/Layer';
import { GenericTcasEvents } from './types/GenericTcasEvents';

export interface NdWxrMessage {
  text: string;
  className: string;
  secondLine?: string;
}

export interface NdWxrMessageEvents {
  ndWxrMessage: NdWxrMessage | null;
}

export interface TcasWXMessagesProps {
  bus: Bus;
  mode: Readable<EfisNdMode>;
}

export class TcasWxrMessages extends View<TcasWXMessagesProps> {
  private readonly taOnlySub = Signal.of(false);

  private readonly tcasModeSub = Signal.of(-1);

  private readonly failSub = Signal.of(false);

  private readonly leftMessage = Signal.of<TcasWxrMessage | undefined>(undefined);

  private readonly rightMessage = Signal.of<TcasWxrMessage | undefined>(undefined);

  private textClassSub = Signal.of('');

  private readonly rightTextClass = Signal.of('');

  private readonly rightSecondLine = Signal.of('');

  private rectClassSub = Signal.of('');

  private handleClass() {
    const taOnly = this.taOnlySub.get();
    const failed = this.failSub.get();
    const mode = this.tcasModeSub.get();

    if (failed) {
      this.textClassSub.set('Amber TcasMessage');
      this.rectClassSub.set('Grey BackgroundFill');
    } else if (taOnly) {
      this.textClassSub.set('White TcasMessage');
      this.rectClassSub.set('Grey BackgroundFill');
    } else if (mode === 0) {
      this.textClassSub.set('White TcasMessage TcasStandbyMessage');
      this.rectClassSub.set('Grey BackgroundFill TcasStandbyMessage');
    } else {
      this.textClassSub.set('');
      this.rectClassSub.set('');
    }
  }

  private readonly y = this.props.mode.derive((mode) =>
    mode === EfisNdMode.ROSE_VOR || mode === EfisNdMode.ROSE_ILS ? 713 : 680,
  );

  private readonly backgroundFillShown = this.props.mode.derive(
    (it) => it === EfisNdMode.ARC || it === EfisNdMode.ROSE_NAV,
  );

  private shown = Derived.from(
    ([mode, left, right]) => {
      const improperMode =
        mode !== EfisNdMode.ARC &&
        mode !== EfisNdMode.ROSE_NAV &&
        mode !== EfisNdMode.ROSE_VOR &&
        mode !== EfisNdMode.ROSE_ILS;

      return !improperMode && (left !== undefined || right !== undefined);
    },
    this.props.mode,
    this.leftMessage,
    this.rightMessage,
  );

  afterRender(node: ViewNode) {
    super.afterRender(node);

    const sub = this.props.bus.inbox<GenericTcasEvents & NdWxrMessageEvents>();

    sub
      .topic('ndWxrMessage')
      .distinct()
      .listen((message) => {
        this.rightMessage.set(message ? { text: message.text } : undefined);
        this.rightTextClass.set(message ? `${message.className} TcasMessage` : '');
        this.rightSecondLine.set(message?.secondLine ?? '');
      });

    sub
      .topic('tcasTaOnly')
      .distinct()
      .listen((value) => {
        this.taOnlySub.set(value);
        this.handleClass();
      });
    sub
      .topic('tcasFault')
      .distinct()
      .listen((value) => {
        this.failSub.set(value);
        this.handleClass();
      });
    sub
      .topic('tcasMode')
      .distinct()
      .listen((value) => {
        this.tcasModeSub.set(value);
        this.handleClass();
      });

    Derived.from(
      ([taOnly, failed, mode]) => {
        if (failed) {
          this.leftMessage.set({ text: 'TCAS' });
        } else if (taOnly) {
          this.leftMessage.set({ text: 'TA ONLY' });
        } else if (mode === 0) {
          this.leftMessage.set({ text: 'TCAS STBY' });
        } else {
          this.leftMessage.set(undefined);
        }
      },
      this.taOnlySub,
      this.failSub,
      this.tcasModeSub,
    );
  }

  render(): ViewNode | null {
    return (
      <Layer x={Signal.of(164)} y={this.y} visible={this.shown}>
        <rect
          visibility={this.backgroundFillShown.derive((shown) => (shown ? 'inherit' : 'hidden'))}
          x={0}
          y={0}
          width={440}
          height={88}
          class="BackgroundFill"
          stroke="none"
        />

        <rect x={0} y={0} width={440} height={27} class={this.rectClassSub} stroke-width={1.75} />

        <text x={8} y={25} class={this.textClassSub} text-anchor="start" font-size={27}>
          {this.leftMessage.derive((it) => it?.text ?? '')}
        </text>

        <text x={425} y={25} class={this.rightTextClass} text-anchor="end" font-size={27}>
          {this.rightMessage.derive((it) => it?.text ?? '')}
        </text>

        <text x={425} y={55} class={this.rightTextClass} text-anchor="end" font-size={27}>
          {this.rightSecondLine}
        </text>
      </Layer>
    );
  }
}
