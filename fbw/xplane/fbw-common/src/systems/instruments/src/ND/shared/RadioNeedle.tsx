// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import {
  UI,
  View,
  Bus,
  Signal,
  Readable,
  ViewNode,
  Derived,
  ClockTopics,
} from 'xplane-js/ui';
import { EfisNdMode, MathUtils, NavAidMode, Arinc429WordData, GenericAdirsEvents } from '@flybywiresim/fbw-sdk';

import { diffAngle } from 'msfs-geo';
import { GenericFcuEvents } from '../types/GenericFcuEvents';
import { GenericDisplayManagementEvents } from '../types/GenericDisplayManagementEvents';
import { GenericVorEvents } from '../types/GenericVorEvents';

export interface RadioNeedleProps {
  bus: Bus;
  headingWord: Readable<Arinc429WordData>;
  trackWord: Readable<Arinc429WordData>;
  isUsingTrackUpMode: Readable<boolean>;
  index: 1 | 2;
  mode: Readable<EfisNdMode>;
}

export class RadioNeedle extends View<RadioNeedleProps> {
  private readonly isVor = Signal.of(false);

  private readonly isAdf = Signal.of(false);

  private readonly trackCorrection = Derived.from(
    ([isUsingTrackUpMode, headingWord, trackWord]) => {
      if (isUsingTrackUpMode) {
        if (headingWord.isNormalOperation() && trackWord.isNormalOperation()) {
          return MathUtils.getSmallestAngle(headingWord.value, trackWord.value);
        }
      }

      return 0;
    },
    this.props.isUsingTrackUpMode,
    this.props.headingWord,
    this.props.trackWord,
  );

  afterRender(node: ViewNode) {
    super.afterRender(node);

    const sub = this.props.bus.inbox<GenericFcuEvents>();

    sub
      .topic(`navaidMode${this.props.index}`)
      .distinct()
      .listen((mode) => {
        if (mode === NavAidMode.VOR) {
          this.isVor.set(true);
          this.isAdf.set(false);
        } else if (mode === NavAidMode.ADF) {
          this.isAdf.set(true);
          this.isVor.set(false);
        } else {
          this.isAdf.set(false);
          this.isVor.set(false);
        }
      });
  }

  render(): ViewNode | null {
    return (
      <>
        <VorNeedle
          bus={this.props.bus}
          shown={this.isVor}
          headingWord={this.props.headingWord}
          trackWord={this.props.trackWord}
          trackCorrection={this.trackCorrection}
          index={this.props.index}
          mode={this.props.mode}
        />
        <AdfNeedle
          bus={this.props.bus}
          shown={this.isAdf}
          headingWord={this.props.headingWord}
          trackWord={this.props.trackWord}
          trackCorrection={this.trackCorrection}
          index={this.props.index}
          mode={this.props.mode}
        />
      </>
    );
  }
}

export interface SingleNeedleProps {
  bus: Bus;
  shown: Readable<boolean>;
  headingWord: Readable<Arinc429WordData>;
  trackWord: Readable<Arinc429WordData>;
  trackCorrection: Readable<number>;
  index: 1 | 2;
  mode: Readable<EfisNdMode>;
}

class VorNeedle extends View<SingleNeedleProps> {
  private readonly ARC_MODE_PATHS = [
    'M384,251 L384,179 M384,128 L384,155 L370,179 L398,179 L384,155 M384,1112 L384,1085 M384,989 L384,1061 L370,1085 L398,1085 L384,1061',
    'M377,251 L377,219 L370,219 L384,195 L398,219 L391,219 L391,251 M384,195 L384,128 M384,1112 L384,1045 M377,989 L377,1045 L391,1045 L391,989',
  ];

  private readonly ROSE_MODE_PATHS = [
    'M384,257 L384,185 M384,134 L384,161 L370,185 L398,185 L384,161 M384,634 L384,607 M384,511 L384,583 L370,607 L398,607 L384,583',
    'M377,257 L377,225 L370,225 L384,201 L398,225 L391,225 L391,256 M384,201 L384,134 M384,634 L384,567 M377,511 L377,567 L391,567 L391,511',
  ];

  private readonly rawRelativeBearing = Signal.of(0);

  private readonly filteredRelativeBearing = Signal.of(0);

  private lastFilterTime = 0;

  private readonly radioAvailable = Signal.of(false);

  private readonly stationDeclination = Signal.of(0);

  private readonly stationLatitude = Signal.of(0);

  private readonly trueRefActive = Signal.of(false);

  // eslint-disable-next-line arrow-body-style
  private readonly availableSub = Derived.from(
    ([shown, radioAvailable, heading, track, mode]) => {
      // TODO in the future we will get the radio values via ARINC429 so this will no longer be needed
      return (
        shown && radioAvailable && heading.isNormalOperation() && track.isNormalOperation() && mode !== EfisNdMode.PLAN
      );
    },
    this.props.shown,
    this.radioAvailable,
    this.props.headingWord,
    this.props.trackWord,
    this.props.mode,
  );

  // eslint-disable-next-line arrow-body-style
  private readonly rotationSub = Derived.from(
    ([relativeBearing, trackCorrection, ndMode]) => {
      return `rotate(${relativeBearing + trackCorrection} 384 ${ndMode === EfisNdMode.ARC ? 620 : 384})`;
    },
    this.filteredRelativeBearing,
    this.props.trackCorrection,
    this.props.mode,
  );

  private readonly needlePaths = Signal.of(['', '']);

  private readonly stationTrueRef = Derived.from(
    ([latitude, declination]) => Math.abs(latitude) > 75 && declination < Number.EPSILON,
    this.stationLatitude,
    this.stationDeclination,
  );

  private readonly stationCorrected = Derived.from(
    ([trueRef, stationTrueRef, available, ndMode]) =>
      trueRef !== stationTrueRef && available && ndMode !== EfisNdMode.ROSE_VOR && ndMode !== EfisNdMode.ROSE_ILS,
    this.trueRefActive,
    this.stationTrueRef,
    this.availableSub,
    this.props.mode,
  );

  afterRender(node: ViewNode) {
    super.afterRender(node);

    const sub = this.props.bus.inbox<
      GenericAdirsEvents & ClockTopics & GenericDisplayManagementEvents & GenericFcuEvents & GenericVorEvents
    >();

    sub
      .topic(`nav${this.props.index}RelativeBearing`)
      .distinct()
      .listen((value) => this.rawRelativeBearing.set(value));
    sub
      .topic(`nav${this.props.index}Available`)
      .distinct()
      .listen((value) => this.radioAvailable.set(!!value));

    sub
      .topic(`nav${this.props.index}StationDeclination`)
      .distinct()
      .listen((v) => this.stationDeclination.set(v));
    sub
      .topic(`nav${this.props.index}Location`)
      .distinct()
      .listen((v) => this.stationLatitude.set(v.lat));

    sub
      .topic('trueRefActive')
      .distinct()
      .listen((v) => this.trueRefActive.set(!!v));

    sub.topic('simTime').listen((simTime) => {
      const dt = Math.min(simTime - this.lastFilterTime, 100);
      this.lastFilterTime = simTime;

      const diff = diffAngle(this.filteredRelativeBearing.get(), this.rawRelativeBearing.get());
      if (!this.availableSub.get() || Math.abs(diff) < 0.1) {
        return;
      }

      const bearing = this.filteredRelativeBearing.get() + (dt / 800) * diff;
      this.filteredRelativeBearing.set(bearing % 360);
    });

    this.props.mode.watch((mode) => {
      switch (mode) {
        case EfisNdMode.ARC:
          this.needlePaths.set(this.ARC_MODE_PATHS);
          break;
        case EfisNdMode.ROSE_ILS:
        case EfisNdMode.ROSE_VOR:
        case EfisNdMode.ROSE_NAV:
          this.needlePaths.set(this.ROSE_MODE_PATHS);
          break;
        default:
      }
    }, true);
  }

  render(): ViewNode | null {
    return (
      <g visibility={this.availableSub.derive((v) => (v ? 'inherit' : 'hidden'))} transform={this.rotationSub}>
        <path d={this.needlePaths.derive((arr) => arr[this.props.index - 1])} stroke-width={3.7} class="rounded shadow" />
        <path
          d={this.needlePaths.derive((arr) => arr[this.props.index - 1])}
          stroke-width={3.2}
          class={this.stationCorrected.derive((c) => (c ? 'Magenta rounded' : 'White rounded'))}
        />
      </g>
    );
  }
}

class AdfNeedle extends View<SingleNeedleProps> {
  private readonly ARC_MODE_PATHS = [
    'M384,251 L384,128 M370,179 L384,155 L398,179 M384,1112 L384,989 M370,1085 L384,1061 L398,1085',
    'M370,251 L370,219 L384,195 L398,219 L398,251 M384,195 L384,128 M384,1112 L384,1023 M370,989 L370,1040 L384,1023 L398,1040 L398,989',
  ];

  private readonly ROSE_MODE_PATHS = [
    'M384,257 L384,134 M370,185 L384,161 L398,185 M384,634 L384,511 M370,607 L384,583 L398,607',
    'M370,257 L370,225 L384,201 L398,225 L398,257 M384,201 L384,134 M384,634 L384,545 M370,511 L370,562 L384,545 L398,562 L398,511',
  ];

  private readonly relativeBearing = Signal.of(0);

  private readonly radioAvailable = Signal.of(false);

  // eslint-disable-next-line arrow-body-style
  private readonly availableSub = Derived.from(
    ([shown, radioAvailable, mode]) => {
      // TODO in the future we will get the radio values via ARINC429 so this will no longer be needed
      return shown && radioAvailable && mode !== EfisNdMode.PLAN;
    },
    this.props.shown,
    this.radioAvailable,
    this.props.mode,
  );

  // eslint-disable-next-line arrow-body-style
  private readonly rotationSub = Derived.from(
    ([relativeBearing, ndMode]) => {
      return `rotate(${relativeBearing} 384 ${ndMode === EfisNdMode.ARC ? 620 : 384})`;
    },
    this.relativeBearing,
    this.props.mode,
  );

  private readonly needlePaths = Signal.of(['', '']);

  afterRender(node: ViewNode) {
    super.afterRender(node);

    const sub = this.props.bus.inbox<GenericVorEvents>();

    sub
      .topic(`adf${this.props.index}Radial`)
      .distinct()
      .listen((value) => this.relativeBearing.set(value));
    sub
      .topic(`adf${this.props.index}SignalStrength`)
      .distinctBy(1)
      .listen((value) => this.radioAvailable.set(value > 0));

    this.props.mode.watch((mode) => {
      switch (mode) {
        case EfisNdMode.ARC:
          this.needlePaths.set(this.ARC_MODE_PATHS);
          break;
        case EfisNdMode.ROSE_ILS:
        case EfisNdMode.ROSE_VOR:
        case EfisNdMode.ROSE_NAV:
          this.needlePaths.set(this.ROSE_MODE_PATHS);
          break;
        default:
      }
    }, true);
  }

  render(): ViewNode | null {
    return (
      <g visibility={this.availableSub.derive((v) => (v ? 'inherit' : 'hidden'))} transform={this.rotationSub}>
        <path d={this.needlePaths.derive((arr) => arr[this.props.index - 1])} stroke-width={3.7} class="rounded shadow" />
        <path d={this.needlePaths.derive((arr) => arr[this.props.index - 1])} stroke-width={3.2} class="Green rounded" />
      </g>
    );
  }
}
