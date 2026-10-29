// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import {
  UI,
  View,
  Bus,
  Derived,
  Signal,
  Readable,
  ViewNode,
  Debouncer,
} from 'xplane-js/ui';
import { Arinc429RegisterSubject, EfisNdMode, NavAidMode } from '@flybywiresim/fbw-sdk';

import { GenericFcuEvents } from '../types/GenericFcuEvents';
import { GenericDisplayManagementEvents } from '../types/GenericDisplayManagementEvents';
import { GenericVorEvents } from '../types/GenericVorEvents';
import { GenericFlightManagementBusEvents } from '../types/GenericFlightManagementBusEvents';

export class RadioNavInfo extends View<{ bus: Bus; index: 1 | 2; mode: Readable<EfisNdMode> }> {
  private readonly isVor = Signal.of(true);

  private readonly isAdf = Signal.of(true);

  afterRender(node: ViewNode) {
    super.afterRender(node);

    const sub = this.props.bus.inbox<GenericFcuEvents>();

    sub
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
        <VorInfo bus={this.props.bus} index={this.props.index} visible={this.isVor} mode={this.props.mode} />
        <AdfInfo bus={this.props.bus} index={this.props.index} visible={this.isAdf} mode={this.props.mode} />
      </>
    );
  }
}

class VorInfo extends View<{
  bus: Bus;
  index: 1 | 2;
  visible: Readable<boolean>;
  mode: Readable<EfisNdMode>;
}> {
  private readonly VOR_1_NEEDLE = 'M25,675 L25,680 L37,696 L13,696 L25,680 M25,696 L25,719';

  private readonly VOR_2_NEEDLE = 'M749,719 L749,696 L755,696 L743,680 L731,696 L737,696 L737,719 M743,680 L743,675';

  private readonly identSub = Signal.of('');

  private readonly frequencySub = Signal.of(0);

  private readonly hasDmeSub = Signal.of(false);

  private readonly dmeDistanceSub = Signal.of(0);

  private readonly availableSub = Signal.of(false);

  private readonly stationDeclination = Signal.of(0);

  private readonly stationLatitude = Signal.of(0);

  private readonly trueRefActive = Signal.of(false);

  private readonly dashedDme = Derived.from(
    ([hasDme, dmeDistance]) => !hasDme || dmeDistance <= 0,
    this.hasDmeSub,
    this.dmeDistanceSub,
  );

  private readonly identReceived = Derived.from(
    // TODO hasDme added from master ND (KBA is weird)
    ([dmeDist, hasNav, ident, hasDme]) => {
      return (dmeDist > 0 || hasNav || hasDme) && ident;
    },
    this.dmeDistanceSub,
    this.availableSub,
    this.identSub,
    this.hasDmeSub,
  );

  private readonly identVisible = Signal.of(false);

  private readonly identTimer = new Debouncer();

  private readonly frequencyVisible = Derived.from(
    ([identVisible, frequency]) => !identVisible && frequency > 1,
    this.identVisible,
    this.frequencySub,
  );

  private readonly stationTrueRef = Derived.from(
    ([latitude, declination]) => Math.abs(latitude) > 75 && declination < Number.EPSILON,
    this.stationLatitude,
    this.stationDeclination,
  );

  private readonly stationCorrected = Derived.from(
    ([trueRef, stationTrueRef, available, mode]) =>
      trueRef !== stationTrueRef && available && mode !== EfisNdMode.ROSE_VOR && mode !== EfisNdMode.ROSE_ILS,
    this.trueRefActive,
    this.stationTrueRef,
    this.availableSub,
    this.props.mode,
  );

  private readonly magWarning = Derived.from(
    ([trueRef, stationTrueRef, available, mode]) =>
      trueRef && !stationTrueRef && available && (mode === EfisNdMode.ROSE_VOR || mode === EfisNdMode.ROSE_ILS),
    this.trueRefActive,
    this.stationTrueRef,
    this.availableSub,
    this.props.mode,
  );

  private readonly trueWarning = Derived.from(
    ([trueRef, stationTrueRef, available, mode]) =>
      !trueRef && stationTrueRef && available && (mode === EfisNdMode.ROSE_VOR || mode === EfisNdMode.ROSE_ILS),
    this.trueRefActive,
    this.stationTrueRef,
    this.availableSub,
    this.props.mode,
  );

  private readonly isVisible = Derived.from(
    ([logicallyVisible, ndMode]) => (logicallyVisible && ndMode !== EfisNdMode.PLAN ? 'inherit' : 'hidden'),
    this.props.visible,
    this.props.mode,
  );

  private readonly x = this.props.index === 1 ? 37 : 668;

  afterRender(node: ViewNode) {
    super.afterRender(node);

    const sub = this.props.bus.inbox<GenericDisplayManagementEvents & GenericVorEvents>();

    sub
      .topic(`nav${this.props.index}Ident`)
      .distinct()
      .listen((v) => this.identSub.set(v));
    sub
      .topic(`nav${this.props.index}Frequency`)
      .distinct()
      .listen((v) => this.frequencySub.set(v));
    sub
      .topic(`nav${this.props.index}HasDme`)
      .distinct()
      .listen((v) => this.hasDmeSub.set(!!v));
    sub
      .topic(`nav${this.props.index}DmeDistance`)
      .distinct()
      .listen((v) => this.dmeDistanceSub.set(v));
    sub
      .topic(`nav${this.props.index}Available`)
      .distinct()
      .listen((v) => this.availableSub.set(!!v));
    sub.topic(`nav${this.props.index}StationDeclination`).listen((v) => this.stationDeclination.set(v));
    sub.topic(`nav${this.props.index}Location`).listen((v) => this.stationLatitude.set(v.lat));

    sub
      .topic('trueRefActive')
      .distinct()
      .listen((v) => this.trueRefActive.set(!!v));

    this.identReceived.watch((v) => {
      if (v) {
        // the morse code is sent 3 times in a 30 second period, depending on when you pick up the signal you could be lucky or unlucky
        this.identTimer.arm(() => this.identVisible.set(true), 5000 + Math.random() * 10000);
      } else {
        this.identTimer.cancel();
        this.identVisible.set(false);
      }
    }, true);
  }

  render(): ViewNode | null {
    return (
      <g visibility={this.isVisible}>
        <path
          d={this.props.index === 1 ? this.VOR_1_NEEDLE : this.VOR_2_NEEDLE}
          stroke-width={2}
          class={this.stationCorrected.derive((c) => (c ? 'Magenta' : 'White'))}
          stroke-linejoin="round"
          stroke-linecap="round"
        />

        <text x={this.x} y={692} font-size={24} class="White">
          {`VOR${this.props.index}`}
        </text>

        <text
          x={this.x + (this.props.index === 1 ? 61 : -54)}
          y={692}
          class="FontTiny Magenta"
          visibility={this.stationCorrected.derive((c) => (c ? 'inherit' : 'hidden'))}
        >
          CORR
        </text>
        <text
          x={this.x + (this.props.index === 1 ? 73 : -54)}
          y={692}
          class="FontTiny Amber"
          visibility={this.magWarning.derive((c) => (c ? 'inherit' : 'hidden'))}
        >
          MAG
        </text>
        <text
          x={this.x + (this.props.index === 1 ? 61 : -54)}
          y={692}
          class="FontTiny Amber"
          visibility={this.trueWarning.derive((c) => (c ? 'inherit' : 'hidden'))}
        >
          TRUE
        </text>

        <text
          visibility={this.identVisible.derive((v) => (v ? 'inherit' : 'hidden'))}
          x={this.x}
          y={722}
          font-size={24}
          class="White"
        >
          {this.identSub}
        </text>

        <text x={this.props.index === 2 ? this.x - 26 : this.x} y={722} font-size={24} class="White">
          <BigLittle visible={this.frequencyVisible} value={this.frequencySub} digits={2} class="White" />
        </text>

        <g transform={`translate(${this.props.index === 1 ? -16 : 0})`}>
          <text
            x={this.dmeDistanceSub.derive((v) => (v > 20 ? this.x + 46 : this.x + 58))}
            y={759}
            font-size={24}
            text-anchor="end"
          >
            <tspan
              visibility={Derived.from(
                ([visible, dashedDme]) => visible && dashedDme,
                this.props.visible,
                this.dashedDme,
              ).derive((v) => (v ? 'inherit' : 'hidden'))}
              class="Green"
            >
              ---
            </tspan>
            <BigLittle
              visible={this.dashedDme.derive((v) => !v)}
              value={this.dmeDistanceSub}
              digits={1}
              roundedThreshold={20}
              class="Green"
            />
          </text>

          <text x={this.x + 66} y={759} font-size={20} class="Cyan">
            NM
          </text>
        </g>

        <TuningModeIndicator bus={this.props.bus} index={this.props.index} />
      </g>
    );
  }
}

class AdfInfo extends View<{
  bus: Bus;
  index: 1 | 2;
  visible: Readable<boolean>;
  mode: Readable<EfisNdMode>;
}> {
  private readonly x = this.props.index === 1 ? 37 : 668;

  private readonly path =
    this.props.index === 1
      ? 'M31,686 L25,680 L19,686 M25,680 L25,719'
      : 'M749,719 L749,696 L743,690 L737,696 L737,719 M743,690 L743,675';

  private readonly adfIdent = Signal.of('');

  private readonly adfFrequency = Signal.of(-1);

  private readonly adfAvailable = Signal.of(false);

  private readonly identVisibility = this.adfAvailable.derive((it) => (it ? 'inherit' : 'hidden'));

  private readonly frequencyVisibility = Derived.from(
    ([adfAvailable, adfFrequency]) => (!adfAvailable && adfFrequency > 0 ? 'inherit' : 'hidden'),
    this.adfAvailable,
    this.adfFrequency,
  );

  private readonly isVisible = Derived.from(
    ([logicallyVisible, ndMode]) => (logicallyVisible && ndMode !== EfisNdMode.PLAN ? 'inherit' : 'hidden'),
    this.props.visible,
    this.props.mode,
  );

  afterRender(node: ViewNode) {
    super.afterRender(node);

    const subs = this.props.bus.inbox<GenericVorEvents>();

    subs
      .topic(`adf${this.props.index}Ident`)
      .distinct()
      .listen((value) => this.adfIdent.set(value));
    subs
      .topic(`adf${this.props.index}ActiveFrequency`)
      .distinct()
      .listen((value) => this.adfFrequency.set(value));
    subs
      .topic(`adf${this.props.index}SignalStrength`)
      .distinctBy(1)
      .listen((value) => this.adfAvailable.set(value > 0));
  }

  render(): ViewNode | null {
    return (
      <g visibility={this.isVisible}>
        <path d={this.path} strokw-width={2} class="Green" stroke-linejoin="round" stroke-linecap="round" />
        <text x={this.x} y={692} font-size={24} class="Green">
          {`ADF${this.props.index}`}
        </text>

        <text visibility={this.identVisibility} x={this.x} y={722} font-size={24} class="Green">
          {this.adfIdent}
        </text>

        <text visibility={this.frequencyVisibility} x={this.x} y={722} font-size={24} class="Green">
          {this.adfFrequency.derive((it) => Math.floor(it).toFixed(0))}
        </text>

        <TuningModeIndicator bus={this.props.bus} index={this.props.index} adf />
      </g>
    );
  }
}

export interface BigLittleProps {
  visible: Readable<boolean>;
  value: Readable<number>;
  digits: number;
  roundedThreshold?: number;
  class: string;
}

export class BigLittle extends View<BigLittleProps> {
  private readonly intPartText = Signal.of('');

  private readonly decimalPartText = Signal.of('');

  private showDecimal = Signal.of(true);

  afterRender(node: ViewNode) {
    super.afterRender(node);

    this.props.value.watch((value) => {
      if (!this.props.visible.get()) {
        return;
      }

      if (this.props.roundedThreshold && value >= this.props.roundedThreshold) {
        this.intPartText.set(value.toFixed(0));
        this.decimalPartText.set('');
        this.showDecimal.set(false);
      } else {
        const [intPart, decimalPart] = value.toFixed(this.props.digits).split('.', 2);

        this.intPartText.set(intPart);
        this.decimalPartText.set(`.${decimalPart}`);
        this.showDecimal.set(true);
      }
    });

    this.props.visible.watch((visible) => {
      if (!visible) {
        this.intPartText.set('');
        this.decimalPartText.set('');
      }
    });
  }

  render(): ViewNode | null {
    return (
      <>
        <tspan visibility={this.props.visible.derive((v) => (v ? 'inherit' : 'hidden'))} class={this.props.class}>
          {this.intPartText}
        </tspan>
        <tspan
          font-size={20}
          visibility={this.showDecimal.derive((showDecimal) => (showDecimal ? 'inherit' : 'hidden'))}
          class={this.props.class}
        >
          {this.decimalPartText}
        </tspan>
      </>
    );
  }
}

interface TuningModeIndicatorProps {
  bus: Bus;
  index: number;
  adf?: boolean;
}

class TuningModeIndicator extends View<TuningModeIndicatorProps> {
  private readonly fm1Healthy = Signal.of(false);

  private readonly fm2Healthy = Signal.of(false);

  private readonly fm1NavTuningWord = Arinc429RegisterSubject.createEmpty();

  private readonly fm2NavTuningWord = Arinc429RegisterSubject.createEmpty();

  private readonly tuningMode = Derived.from(
    ([fm1Healthy, fm2Healthy, fm1NavTuningWord, fm2NavTuningWord]) => {
      const bitIndex = 10 + this.props.index + (this.props.adf ? 2 : 0);

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

  private readonly x = this.props.index === 1 ? 138 : 616;

  afterRender(node: ViewNode) {
    super.afterRender(node);

    const subs = this.props.bus.inbox<GenericFlightManagementBusEvents>();

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
  }

  render(): ViewNode | null {
    return (
      <text x={this.x} y={720} font-size={20} text-decoration="underline" fill="#ffffff">
        {this.tuningMode}
      </text>
    );
  }
}
