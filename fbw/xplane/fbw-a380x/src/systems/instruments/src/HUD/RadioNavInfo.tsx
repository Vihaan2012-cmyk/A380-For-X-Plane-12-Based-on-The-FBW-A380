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
  BusSignal,
} from 'xplane-js/ui';
import { Arinc429LocalVarConsumerSubject, Arinc429RegisterSubject, NavAidMode } from '@flybywiresim/fbw-sdk';

import { GenericVorEvents } from '../../../../../../fbw-common/src/systems/instruments/src/ND/types/GenericVorEvents';
import { GenericFlightManagementBusEvents } from '../../../../../../fbw-common/src/systems/instruments/src/ND/types/GenericFlightManagementBusEvents';
import { Layer } from '../../../../../../fbw-common/src/systems/instruments/src/MsfsAvionicsCommon/Layer';
import { GenericFcuEvents } from '../../../../../../fbw-common/src/systems/instruments/src/ND/types/GenericFcuEvents';
import { HUDSimvars } from './shared/HUDSimvarPublisher';
import { HudElems, HudMode } from './HUDUtils';
import { FcuEfisCpBusEvents } from '@shared/publishers/EfisCpBusPublisher';
import { getDisplayIndex } from './HUD';

export class RadioNavInfo extends View<{ bus: Bus; index: 1 | 2 }> {
  private readonly sub = this.props.bus.inbox<GenericFcuEvents & HUDSimvars & HudElems & FcuEfisCpBusEvents>();
  private readonly isVor = Signal.of(true);

  private readonly isAdf = Signal.of(true);

  afterRender(node: ViewNode) {
    super.afterRender(node);

    this.sub
      .topic(`navaidMode${this.props.index}`)
      .distinct()
      .listen((value) => {
        if (value === NavAidMode.VOR) {
          this.isVor.set(true);
          this.isAdf.set(false);
        } else if (value === NavAidMode.ADF) {
          this.isVor.set(false);
          this.isAdf.set(true);
        } else {
          this.isVor.set(false);
          this.isAdf.set(false);
        }
      });
  }

  render(): ViewNode | null {
    return (
      <>
        <g id="VorDmeInfo">
          <VorDmeInfo bus={this.props.bus} index={this.props.index} />
          {/* no ADF inplemented yet */}
          {/* <AdfInfo bus={this.props.bus} index={1} visible={this.isAdf} /> */}
        </g>
      </>
    );
  }
}

export interface VorInfoIndicatorProps extends ViewProps {
  bus: Bus;
  index: 1 | 2;
  //visible: Subject<boolean>;
}

export class VorDmeInfo extends View<VorInfoIndicatorProps> {
  private readonly sub = this.props.bus.inbox<GenericFcuEvents & HUDSimvars & HudElems & FcuEfisCpBusEvents>();

  private readonly adf = Signal.of(false);

  private readonly vorIdent = Signal.of('');

  private readonly vorFrequency = Signal.of(-1);

  private readonly vorCourse = Signal.of(-1);

  private readonly fm1Healthy = Signal.of(false);

  private readonly fm2Healthy = Signal.of(false);

  private readonly fm1NavTuningWord = Arinc429RegisterSubject.createEmpty();

  private readonly fm2NavTuningWord = Arinc429RegisterSubject.createEmpty();
  private readonly fcuEisDiscreteWord2 = Arinc429LocalVarConsumerSubject.create(
    this.sub.topic(getDisplayIndex() === 2 ? 'fcu_efis_r_discrete_word_2' : 'fcu_efis_l_discrete_word_2'),
  );
  private readonly isEfisVor1Pressed = this.fcuEisDiscreteWord2.derive((word) => word.bitValueOr(28, true));
  private readonly isEfisVor2Pressed = this.fcuEisDiscreteWord2.derive((word) => word.bitValueOr(29, true));
  private readonly isEfisLsPressed = this.fcuEisDiscreteWord2.derive((word) => word.bitValueOr(14, true));
  private readonly hudMode = BusSignal.from(this.sub.topic('hudFlightPhaseMode'), 0);

  private readonly isVorVisible = Derived.from(
    ([hudmode, Vor1Pressed, Vor2Pressed, LsPressed]) => {
      const side = this.props.index === 1 ? Vor1Pressed : Vor2Pressed;
      if (hudmode === HudMode.NORMAL) {
        return side && !LsPressed ? 'block' : 'none';
      } else {
        return 'none';
      }
    },
    this.hudMode,
    this.isEfisVor1Pressed,
    this.isEfisVor2Pressed,
    this.isEfisLsPressed,
  );
  private showDecimal = Signal.of(true);
  private readonly intPartText = Signal.of('');
  private readonly roundedThreshold = 20;
  private readonly decimalPartText = Signal.of('');
  private readonly isVor = Signal.of(true);
  private readonly isAdf = Signal.of(true);
  private readonly xPos = this.props.index === 1 ? 240 : 910;
  private readonly dmeDistanceSub = Signal.of(0);

  private readonly hasDmeSub = Signal.of(false);
  private readonly dashedDme = Derived.from(
    ([hasDme, dmeDistance]) => !hasDme || dmeDistance <= 0,
    this.hasDmeSub,
    this.dmeDistanceSub,
  );
  private readonly isVisible = Derived.from(
    ([isVor, isAdf, dashedDme]) => {
      return (isVor || isAdf) && dashedDme ? 'inherit' : 'hidden';
    },
    this.isVor,
    this.isAdf,
    this.dashedDme,
  );

  afterRender(node: ViewNode) {
    super.afterRender(node);
    const isFo = getDisplayIndex() === 2;
    this.fcuEisDiscreteWord2.setConsumer(
      this.sub.topic(isFo ? 'fcu_efis_r_discrete_word_2' : 'fcu_efis_l_discrete_word_2'),
    );
    const subs = this.props.bus.inbox<GenericVorEvents & GenericFlightManagementBusEvents & GenericFcuEvents>();

    subs
      .topic(`nav${this.props.index}Ident`)
      .distinct()
      .listen((value) => {
        this.vorIdent.set(value);
      });

    subs
      .topic(`nav${this.props.index}HasDme`)
      .distinct()
      .listen((v) => this.hasDmeSub.set(!!v));
    subs
      .topic(`nav${this.props.index}DmeDistance`)
      .distinct()
      .listen((v) => {
        this.dmeDistanceSub.set(v);
        this.setDmeText();
      });

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

    subs
      .topic(`navaidMode${this.props.index}`)
      .distinct()
      .listen((value) => {
        if (value === NavAidMode.VOR) {
          this.isVor.set(true);
          this.isAdf.set(false);
        } else if (value === NavAidMode.ADF) {
          this.isVor.set(false);
          this.isAdf.set(true);
        } else {
          this.isVor.set(false);
          this.isAdf.set(false);
        }
      });
  }

  private setDmeText() {
    this.dmeDistanceSub.watch((value) => {
      if (this.dashedDme.get()) {
        this.intPartText.set('');
        this.decimalPartText.set('');
        return;
      }

      if (this.roundedThreshold && this.dmeDistanceSub.get() >= this.roundedThreshold) {
        this.intPartText.set(this.dmeDistanceSub.get().toFixed(0));
        this.decimalPartText.set('');
        this.showDecimal.set(false);
      } else {
        const [intPart, decimalPart] = value.toFixed(1).split('.', 2);

        this.intPartText.set(intPart);
        this.decimalPartText.set(`.${decimalPart}`);
        this.showDecimal.set(true);
      }
    });
  }

  render(): ViewNode | null {
    return (
      <g display={this.isVorVisible}>
        <Layer x={this.xPos} y={820}>
          <text x={0} y={0} class="FontSmallest Green" text-anchor="start">
            VOR
            {this.props.index.toString()}
          </text>

          <text x={0} y={25} class="FontSmallest Green" text-anchor="start">
            {this.vorIdent}
          </text>

          {/* dme */}

          <text x={this.dmeDistanceSub.derive((v) => (v > 20 ? 22 : 34))} y={50} text-anchor="end">
            <tspan visibility={this.isVisible} class="FontSmallest Green">
              ---
            </tspan>
            <tspan visibility={this.dashedDme.derive((v) => (!v ? 'inherit' : 'hidden'))} class="FontTiny Green">
              {this.intPartText}
            </tspan>
            <tspan
              visibility={this.showDecimal.derive((showDecimal) => (showDecimal ? 'inherit' : 'hidden'))}
              class="FontTinyer Green"
            >
              {this.decimalPartText}
            </tspan>
          </text>

          <text x={50} y={50} class="FontTiny Green">
            NM
          </text>
        </Layer>
      </g>
    );
  }
}
