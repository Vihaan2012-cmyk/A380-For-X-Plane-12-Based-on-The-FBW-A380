// Copyright (c) 2021-2024 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import {
  UI,
  ViewProps,
  View,
  Bus,
  Signal,
  ViewNode,
  Derived,
} from 'xplane-js/ui';
import { Arinc429RegisterSubject } from '@flybywiresim/fbw-sdk';

import { GenericVorEvents } from '../../types/GenericVorEvents';
import { GenericFlightManagementBusEvents } from '../../types/GenericFlightManagementBusEvents';
import { Layer } from '../../../MsfsAvionicsCommon/Layer';

export interface VorInfoIndicatorProps extends ViewProps {
  bus: Bus;
  index: 1 | 2;
}

export class VorInfoIndicator extends View<VorInfoIndicatorProps> {
  private readonly adf = Signal.of(false);

  private readonly vorIdent = Signal.of('');

  private readonly vorFrequency = Signal.of(-1);

  private readonly vorCourse = Signal.of(-1);

  // private readonly vorAvailable = Subject.create(false);

  private readonly fm1Healthy = Signal.of(false);

  private readonly fm2Healthy = Signal.of(false);

  private readonly fm1NavTuningWord = Arinc429RegisterSubject.createEmpty();

  private readonly fm2NavTuningWord = Arinc429RegisterSubject.createEmpty();

  private readonly tuningMode = Derived.from(
    ([adf, fm1Healthy, fm2Healthy, fm1NavTuningWord, fm2NavTuningWord]) => {
      const bitIndex = 10 + this.props.index + (adf ? 2 : 0);

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
    this.adf,
    this.fm1Healthy,
    this.fm2Healthy,
    this.fm1NavTuningWord,
    this.fm2NavTuningWord,
  );

  private readonly frequencyIntTextSub = Signal.of('---');

  private readonly frequencyDecimalTextSub = Signal.of('--');

  private readonly courseTextSub = Signal.of('---');

  afterRender(node: ViewNode) {
    super.afterRender(node);

    const subs = this.props.bus.inbox<GenericVorEvents & GenericFlightManagementBusEvents>();

    subs
      .topic(`nav${this.props.index}Ident`)
      .distinct()
      .listen((value) => {
        this.vorIdent.set(value);
      });

    subs
      .topic(`nav${this.props.index}Frequency`)
      .distinct()
      .listen((value) => {
        this.vorFrequency.set(value);
      });

    subs
      .topic(`nav${this.props.index}Obs`)
      .distinct()
      .listen((value) => {
        this.vorCourse.set(value);
      });

    /*      subs.on(`nav${this.props.index}Available`).whenChanged().handle((value) => {
            this.vorAvailable.set(value);
        }); */

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

    this.vorFrequency.watch((frequency) => {
      const [int, dec] = frequency.toFixed(2).split('.', 2);

      if (frequency > 0) {
        this.frequencyIntTextSub.set(int);
        this.frequencyDecimalTextSub.set(dec);
      } else {
        this.frequencyIntTextSub.set('---');
        this.frequencyDecimalTextSub.set('--');
      }
    }, true);

    this.vorCourse.watch((course) => {
      this.courseTextSub.set(this.vorFrequency.get() > 0 ? Math.round(course).toString().padStart(3, '0') : '---');
    }, true);
  }

  private readonly visibilityFn = (v: boolean) => (v ? 'inherit' : 'hidden');

  render(): ViewNode | null {
    return (
      <Layer x={748} y={28}>
        <text x={-102} y={0} font-size={25} class="White" text-anchor="end">
          VOR
          {this.props.index.toString()}
        </text>

        <g class="White" text-anchor="end">
          <text x={-40} y={0} font-size={25}>
            {this.frequencyIntTextSub}
          </text>
          <text x={0} y={0} font-size={20}>
            .{this.frequencyDecimalTextSub}
          </text>
        </g>

        <text x={-56} y={30} font-size={25} class="White" text-anchor="end">
          CRS
        </text>
        <text x={20} y={30} font-size={25} class="Cyan" text-anchor="end">
          {this.courseTextSub}
          &deg;
        </text>

        <g visibility={this.vorFrequency.derive((v) => v > 0).derive(this.visibilityFn)}>
          <text x={-80} y={58} font-size={20} class="White" text-anchor="end" text-decoration="underline">
            {this.tuningMode}
          </text>
        </g>

        <text x={0} y={60} font-size={25} class="White" text-anchor="end">
          {this.vorIdent}
        </text>
      </Layer>
    );
  }
}
