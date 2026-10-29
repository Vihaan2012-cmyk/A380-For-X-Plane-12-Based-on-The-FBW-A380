// @ts-strict-ignore
// Copyright (c) 2021-2024 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import {
  UI,
  View,
  ViewNode,
  Signal,
  Bus,
  BusSignal,
  Derived,
} from 'xplane-js/ui';
import { Arinc429RegisterSubject } from '@flybywiresim/fbw-sdk';

import { GenericVorEvents } from '../../types/GenericVorEvents';
import { GenericFlightManagementBusEvents } from '../../types/GenericFlightManagementBusEvents';
import { Layer } from '../../../MsfsAvionicsCommon/Layer';

export interface IlsInfoIndicatorProps {
  bus: Bus;
  index: 1 | 2;
}

export class IlsInfoIndicator extends View<IlsInfoIndicatorProps> {
  private readonly ilsIdent = BusSignal.from(null, '');

  private readonly ilsFrequency = BusSignal.from(null, -1);

  private readonly ilsCourse = BusSignal.from(null, -1);

  private readonly fm1Healthy = Signal.of(false);

  private readonly fm2Healthy = Signal.of(false);

  private readonly fm1NavTuningWord = Arinc429RegisterSubject.createEmpty();

  private readonly fm2NavTuningWord = Arinc429RegisterSubject.createEmpty();

  private readonly ilsFrequencyValid = Signal.of(false);

  private readonly ilsCourseValid = Signal.of(false);

  private readonly frequencyIntTextSub = Signal.of('');

  private readonly frequencyDecimalTextSub = Signal.of('');

  private readonly courseTextSub = Signal.of('');

  private readonly tuningMode = Derived.from(
    ([fm1Healthy, fm2Healthy, fm1NavTuningWord, fm2NavTuningWord]) => {
      const bitIndex = 14 + this.props.index;

      if (
        (!fm1Healthy && !fm2Healthy) ||
        (!fm1NavTuningWord.isNormalOperation() && !fm2NavTuningWord.isNormalOperation())
      ) {
        return 'R';
      }

      if (fm1NavTuningWord.bitValueOr(bitIndex, false) || fm2NavTuningWord.bitValueOr(bitIndex, false)) {
        return 'M';
      }

      return '';
    },
    this.fm1Healthy,
    this.fm2Healthy,
    this.fm1NavTuningWord,
    this.fm2NavTuningWord,
  );

  afterRender(node: ViewNode) {
    super.afterRender(node);

    const subs = this.props.bus.inbox<GenericVorEvents & GenericFlightManagementBusEvents>();

    // TODO select correct MMR
    // Fixed now??
    const index = (this.props.index + 2) as 3 | 4;

    this.ilsIdent.rebind(subs.topic(`nav${index}Ident`).distinct());

    this.ilsFrequency.rebind(subs.topic(`nav${index}Frequency`).distinct());

    this.ilsCourse.rebind(subs.topic(`nav${index}Obs`).distinct());

    this.ilsFrequency.watch((freq) => this.ilsFrequencyValid.set(freq >= 108 && freq <= 112), true);
    this.ilsFrequencyValid.watch((valid) => this.ilsCourseValid.set(valid), true);

    subs
      .topic('fm.1.healthy_discrete')
      .distinct()
      .listen((healthy) => this.fm1Healthy.set(healthy));

    subs
      .topic('fm.2.healthy_discrete')
      .distinct()
      .listen((healthy) => this.fm2Healthy.set(healthy));

    subs
      .topic('fm.1.tuning_discrete_word')
      .distinct()
      .listen((word) => this.fm1NavTuningWord.setWord(word));

    subs
      .topic('fm.2.tuning_discrete_word')
      .distinct()
      .listen((word) => this.fm2NavTuningWord.setWord(word));

    Derived.from(
      ([freq, valid]) => {
        if (valid) {
          const [int, dec] = freq.toFixed(2).split('.', 2);

          this.frequencyIntTextSub.set(int);
          this.frequencyDecimalTextSub.set(dec);
        } else {
          this.frequencyIntTextSub.set('---');
          this.frequencyDecimalTextSub.set('--');
        }
      },
      this.ilsFrequency,
      this.ilsFrequencyValid,
    );

    this.ilsCourse.watch((course) => {
      this.courseTextSub.set(this.ilsFrequency.get() > 0 ? Math.round(course).toString().padStart(3, '0') : '---');
    }, true);
  }

  private readonly visibilityFn = (v) => (v ? 'inherit' : 'hidden');

  render(): ViewNode | null {
    return (
      <Layer x={748} y={28}>
        <text x={-102} y={0} font-size={25} class="White" text-anchor="end">
          ILS
          {this.props.index.toString()}
        </text>

        <g>
          <text x={0} y={0} font-size={25} class="Magenta" text-anchor="end">
            {this.frequencyIntTextSub}
            <tspan font-size={20}>.{this.frequencyDecimalTextSub}</tspan>
          </text>
        </g>

        <text x={-56} y={30} font-size={25} class="White" text-anchor="end">
          CRS
        </text>
        <text x={20} y={30} font-size={25} text-anchor="end">
          <tspan class="Magenta">{this.courseTextSub}</tspan>
          <tspan class="Cyan">&deg;</tspan>
        </text>

        <g visibility={this.ilsFrequency.derive((v) => v > 0).derive(this.visibilityFn)}>
          <text x={-80} y={58} font-size={20} class="White" text-anchor="end" text-decoration="underline">
            {this.tuningMode}
          </text>
        </g>

        <text x={0} y={60} font-size={25} class="Magenta" text-anchor="end">
          {this.ilsIdent}
        </text>
      </Layer>
    );
  }
}
