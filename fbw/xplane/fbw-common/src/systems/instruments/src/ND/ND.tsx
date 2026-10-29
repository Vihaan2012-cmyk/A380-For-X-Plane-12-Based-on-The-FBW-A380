// @ts-strict-ignore
// Copyright (c) 2021-2024 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import {
  ClockTopics,
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  Signal,
  Readable,
  UnitList,
  ViewNode,
} from 'xplane-js/ui';

import { FMMessage, GenericAdirsEvents, NXDataStore, OansControlEvents } from '@flybywiresim/fbw-sdk';

import { clampAngle } from 'msfs-geo';
import { BtvRunwayInfo } from './shared/BtvRunwayInfo';
import { RwyAheadAdvisory } from './shared/RwyAheadAdvisory';
import { SelectedHeadingBug } from './pages/arc/SelectedHeadingBug';
import { VnavStatus } from './shared/VnavStatus';
import { LnavStatus } from './shared/LnavStatus';
import { CrossTrackError } from './shared/CrossTrackError';
import { RadioNeedle } from './shared/RadioNeedle';
import { GenericFmsEvents } from './types/GenericFmsEvents';
import { NDSimvars } from './NDSimvarPublisher';
import { ArcModePage } from './pages/arc';
import { Layer } from '../MsfsAvionicsCommon/Layer';
import { FmMessages } from './FmMessages';
import { Flag, FlagProps } from './shared/Flag';
import { CanvasMap } from './shared/map/CanvasMap';
import { NDPage } from './pages/NDPage';
import { PlanModePage } from './pages/plan';
import { RadioNavInfo } from './shared/RadioNavInfo';
import { RoseNavPage } from './pages/rose/RoseNavPage';
import { RoseLSPage } from './pages/rose/RoseLSPage';
import { RoseVorPage } from './pages/rose/RoseVorPage';
import { NDControlEvents } from './NDControlEvents';
import { Airplane } from './shared/Airplane';
import { TcasWxrMessages } from './TcasWxrMessages';
import { Chrono } from './Chrono';
import { WindIndicator } from './shared/WindIndicator';
import { TerrainMapThresholds } from './TerrainMapThresholds';
import { TrackLine } from './shared/TrackLine';
import { TrackBug } from './shared/TrackBug';
import { GenericFcuEvents } from './types/GenericFcuEvents';
import { ArincEventBus } from '../../../shared/src/ArincEventBus';
import { EfisNdMode, EfisRecomputingReason, EfisSide } from '../NavigationDisplay';
import { Arinc429RegisterSubject } from '../../../shared/src/Arinc429RegisterSubject';
import { Arinc429ConsumerSubject } from '../../../shared/src/Arinc429ConsumerSubject';
import { FmsOansData } from '../../../shared/src/publishers/OansBtv/FmsOansPublisher';
import { MathUtils } from '../../../shared/src/MathUtils';
import { SimVarString } from '../../../shared/src/simvar';
import { GenericDisplayManagementEvents } from './types/GenericDisplayManagementEvents';
import { MapOptions } from './types/MapOptions';

const PAGE_GENERATION_BASE_DELAY = 500;
const PAGE_GENERATION_RANDOM_DELAY = 70;

export const getDisplayIndex = () => {
  const url = document.querySelector('vcockpit-panel > *').getAttribute('url');
  return url ? parseInt(url.substring(url.length - 1), 10) : 0;
};

export interface NDProps<T extends number> {
  bus: ArincEventBus;

  side: EfisSide;

  rangeValues: T[];

  terrainThresholdPaddingText: string;

  rangeChangeMessage: string;

  modeChangeMessage: string;

  mapOptions?: Partial<MapOptions>;

  fmMessages: FMMessage[];
}

export class NDComponent<T extends number> extends View<NDProps<T>> {
  private readonly pposLatWord = Arinc429RegisterSubject.createEmpty();

  private readonly pposLonWord = Arinc429RegisterSubject.createEmpty();

  private readonly isUsingTrackUpMode = Signal.of(false);

  private readonly trueHeadingWord = Arinc429RegisterSubject.createEmpty();

  private readonly trueTrackWord = Arinc429RegisterSubject.createEmpty();

  /** either magnetic or true heading depending on true ref mode */
  private readonly headingWord = Arinc429ConsumerSubject.create(null);

  /** either magnetic or true track depending on true ref mode */
  private readonly trackWord = Arinc429ConsumerSubject.create(null);

  private readonly trueRefActive = Signal.of(false);

  private readonly roseLSPage = UI.ref<RoseLSPage<T>>();

  private readonly roseVorPage = UI.ref<RoseVorPage<T>>();

  private readonly roseNavPage = UI.ref<RoseNavPage<T>>();

  private readonly arcPage = UI.ref<ArcModePage<T>>();

  private readonly planPage = UI.ref<PlanModePage<T>>();

  private currentPageMode = Signal.of(EfisNdMode.ARC);

  private currentPageInstance: NDPage;

  private readonly pageChangeInProgress = Signal.of(false);

  private pageChangeInvalidationTimeout = -1;

  private readonly rangeChangeInProgress = Signal.of(false);

  private rangeChangeInvalidationTimeout = -1;

  private readonly mapRecomputing = Derived.from(
    ([pageChange, rangeChange]) => pageChange || rangeChange,
    this.pageChangeInProgress,
    this.rangeChangeInProgress,
  );

  private readonly trkFlagShown = Derived.from(
    ([isUsingTrackUpMode, trackWord, currentPageMode]) => {
      if (currentPageMode === EfisNdMode.PLAN) {
        return false;
      }

      if (isUsingTrackUpMode) {
        return !trackWord.isNormalOperation();
      }

      return false;
    },
    this.isUsingTrackUpMode,
    this.trackWord,
    this.currentPageMode,
  );

  private readonly hdgFlagShown = Derived.from(
    ([headingWord, currentPageMode]) => {
      if (currentPageMode === EfisNdMode.PLAN) {
        return false;
      }

      return !headingWord.isNormalOperation();
    },
    this.headingWord,
    this.currentPageMode,
  );

  private showOans = Signal.of<boolean>(false);

  private readonly lengthUnit = NXDataStore.getSetting('CONFIG_USING_METRIC_UNIT').derive((v) =>
    v ? UnitList.METER : UnitList.FOOT,
  );

  afterRender(node: ViewNode) {
    super.afterRender(node);

    this.currentPageInstance = this.arcPage.target;

    this.currentPageInstance.isVisible.set(true);
    this.currentPageInstance.onShow();

    const sub = this.props.bus.inbox<
      GenericFcuEvents &
        GenericDisplayManagementEvents &
        GenericFmsEvents &
        NDControlEvents &
        NDSimvars &
        OansControlEvents
    >();

    sub
      .topic('trueHeadingRaw')
      .distinct()
      .listen((value) => {
        this.trueHeadingWord.setWord(value);
      });

    sub
      .topic('trueTrackRaw')
      .distinct()
      .listen((value) => {
        this.trueTrackWord.setWord(value);
      });

    sub
      .topic('latitude')
      .distinct()
      .listen((value) => {
        this.pposLatWord.setWord(value);
      });

    sub
      .topic('longitude')
      .distinct()
      .listen((value) => {
        this.pposLonWord.setWord(value);
      });

    this.headingWord.setConsumer(sub.topic('heading'));

    this.trackWord.setConsumer(sub.topic('track'));

    sub
      .topic('trueRefActive')
      .distinct()
      .listen((v) => this.trueRefActive.set(v));

    sub
      .topic('ndRangeSetting')
      .distinct()
      .listen(() => {
        this.invalidateRange();
      });

    sub
      .topic('ndMode')
      .distinct()
      .listen((mode) => {
        this.handleNewMapPage(mode);
      });

    this.mapRecomputing.watch((recomputing) => {
      this.props.bus.outbox<NDControlEvents>().post('set_map_recomputing', recomputing);

      let reason = EfisRecomputingReason.None;
      if (this.pageChangeInProgress.get() && this.rangeChangeInProgress.get()) {
        reason = EfisRecomputingReason.ModeAndRangeChange;
      } else if (this.pageChangeInProgress.get()) {
        reason = EfisRecomputingReason.ModeChange;
      } else if (this.rangeChangeInProgress.get()) {
        reason = EfisRecomputingReason.RangeChange;
      }
      this.props.bus.outbox<NDControlEvents>().post('set_map_recomputing_reason', reason);
    });

    sub
      .topic('nd_show_oans')
      .distinct()
      .listen((data) => {
        if (data.side === this.props.side) {
          this.showOans.set(data.show);
        }
      });
  }

  // eslint-disable-next-line arrow-body-style
  private readonly mapFlagShown = Derived.from(
    ([headingWord, latWord, longWord, currentPageMode]) => {
      return (
        (!headingWord.isNormalOperation() || !latWord.isNormalOperation() || !longWord.isNormalOperation()) &&
        currentPageMode !== EfisNdMode.PLAN
      );
    },
    this.headingWord,
    this.pposLatWord,
    this.pposLonWord,
    this.currentPageMode,
  );

  private readonly planeRotation = Derived.from(
    ([isUsingTrackUpMode, headingWord, trackWord]) => {
      if (isUsingTrackUpMode) {
        if (headingWord.isNormalOperation() && trackWord.isNormalOperation()) {
          // FIXME move that file to MsfsAvionicsCommon or another shared folder
          return MathUtils.getSmallestAngle(headingWord.value, trackWord.value);
        }
      }

      return 0;
    },
    this.isUsingTrackUpMode,
    this.headingWord,
    this.trackWord,
  );

  private handleNewMapPage(mode: EfisNdMode) {
    if (mode === this.currentPageMode.get()) {
      return;
    }

    this.currentPageInstance.isVisible.set(false);
    this.currentPageInstance.onHide();

    this.currentPageMode.set(mode);

    switch (mode) {
      case EfisNdMode.ROSE_ILS:
        this.currentPageInstance = this.roseLSPage.target;
        break;
      case EfisNdMode.ROSE_VOR:
        this.currentPageInstance = this.roseVorPage.target;
        break;
      case EfisNdMode.ROSE_NAV:
        this.currentPageInstance = this.roseNavPage.target;
        break;
      case EfisNdMode.ARC:
        this.currentPageInstance = this.arcPage.target;
        break;
      case EfisNdMode.PLAN:
        this.currentPageInstance = this.planPage.target;
        break;
      default:
        console.warn(`Unknown ND page mode=${mode}`);
        break;
    }

    this.currentPageInstance.isVisible.set(true);
    this.currentPageInstance.onShow();

    this.invalidatePage();
  }

  private invalidateRange() {
    if (this.rangeChangeInvalidationTimeout !== -1) {
      window.clearTimeout(this.rangeChangeInvalidationTimeout);
    }

    if (
      this.currentPageMode.get() !== EfisNdMode.ROSE_ILS &&
      this.currentPageMode.get() !== EfisNdMode.ROSE_VOR &&
      !this.mapFlagShown.get()
    ) {
      // Range has priority over mode change
      this.pageChangeInProgress.set(false);
      this.rangeChangeInProgress.set(true);
      this.rangeChangeInvalidationTimeout = window.setTimeout(
        () => {
          this.rangeChangeInProgress.set(false);
        },
        Math.random() * PAGE_GENERATION_RANDOM_DELAY + PAGE_GENERATION_BASE_DELAY,
      );
    } else {
      this.rangeChangeInProgress.set(false);
    }
  }

  private invalidatePage() {
    if (this.pageChangeInvalidationTimeout !== -1) {
      window.clearTimeout(this.pageChangeInvalidationTimeout);
    }

    if (
      this.currentPageMode.get() !== EfisNdMode.ROSE_ILS &&
      this.currentPageMode.get() !== EfisNdMode.ROSE_VOR &&
      !this.rangeChangeInProgress.get() &&
      !this.mapFlagShown.get()
    ) {
      this.pageChangeInProgress.set(true);
      this.pageChangeInvalidationTimeout = window.setTimeout(
        () => {
          this.pageChangeInProgress.set(false);
        },
        Math.random() * PAGE_GENERATION_RANDOM_DELAY + PAGE_GENERATION_BASE_DELAY,
      );
    } else {
      this.pageChangeInProgress.set(false);
    }
  }

  render(): ViewNode | null {
    return (
      <>
        <div style={{ display: this.showOans.derive((it) => (it ? 'block' : 'none')) }}>
          <div style={{ display: this.currentPageMode.derive((it) => (it === EfisNdMode.PLAN ? 'none' : 'block')) }}>
            <svg class="nd-svg" viewBox="0 0 768 768" style="transform: rotateX(0deg);">
              <WindIndicator bus={this.props.bus} />
              <SpeedIndicator bus={this.props.bus} />
              <Chrono bus={this.props.bus} />
              <TopMessages bus={this.props.bus} ndMode={this.currentPageMode} showOans={this.showOans} />
            </svg>
          </div>
          <div style={{ display: this.currentPageMode.derive((it) => (it === EfisNdMode.PLAN ? 'block' : 'none')) }}>
            <svg class="nd-svg" viewBox="0 0 768 768" style="transform: rotateX(0deg);">
              <BtvRunwayInfo bus={this.props.bus} lengthUnit={this.lengthUnit} />
              <SpeedIndicator bus={this.props.bus} />
            </svg>
          </div>
          <svg class="nd-svg nd-top-layer" viewBox="0 0 768 768" style="transform: rotateX(0deg);">
            <TcasWxrMessages bus={this.props.bus} mode={this.currentPageMode} />
            <FmMessages bus={this.props.bus} mode={this.currentPageMode} fmMessages={this.props.fmMessages} />
            <RwyAheadAdvisory bus={this.props.bus} />
          </svg>
        </div>
        <div style={{ display: this.showOans.derive((it) => (it ? 'none' : 'block')) }}>
          {/* ND Vector graphics - bottom layer */}
          <svg class="nd-svg" viewBox="0 0 768 768" style="transform: rotateX(0deg);">
            <RoseLSPage
              bus={this.props.bus}
              ref={this.roseLSPage}
              headingWord={this.headingWord}
              trueHeadingWord={this.trueHeadingWord}
              trackWord={this.trackWord}
              trueTrackWord={this.trueTrackWord}
              rangeValues={this.props.rangeValues}
              isUsingTrackUpMode={this.isUsingTrackUpMode}
              /* Capt ND shows ILS2  */
              index={this.props.side === 'L' ? 2 : 1}
            />
            <RoseVorPage
              bus={this.props.bus}
              ref={this.roseVorPage}
              headingWord={this.headingWord}
              trueHeadingWord={this.trueHeadingWord}
              trackWord={this.trackWord}
              trueTrackWord={this.trueTrackWord}
              rangeValues={this.props.rangeValues}
              isUsingTrackUpMode={this.isUsingTrackUpMode}
              /* Capt ND shows VOR1  */
              index={this.props.side === 'L' ? 1 : 2}
            />
            <RoseNavPage
              bus={this.props.bus}
              ref={this.roseNavPage}
              headingWord={this.headingWord}
              trueHeadingWord={this.trueHeadingWord}
              trackWord={this.trackWord}
              trueTrackWord={this.trueTrackWord}
              rangeValues={this.props.rangeValues}
              isUsingTrackUpMode={this.isUsingTrackUpMode}
            />
            <ArcModePage
              ref={this.arcPage}
              bus={this.props.bus}
              rangeValues={this.props.rangeValues}
              headingWord={this.headingWord}
              trueHeadingWord={this.trueHeadingWord}
              trackWord={this.trackWord}
              trueTrackWord={this.trueTrackWord}
              isUsingTrackUpMode={this.isUsingTrackUpMode}
            />
            <PlanModePage
              ref={this.planPage}
              bus={this.props.bus}
              rangeValues={this.props.rangeValues}
              aircraftTrueHeading={this.trueHeadingWord}
            />

            <SelectedHeadingBug bus={this.props.bus} rotationOffset={this.planeRotation} mode={this.currentPageMode} />

            <TrackLine bus={this.props.bus} isUsingTrackUpMode={this.isUsingTrackUpMode} />
            <TrackBug bus={this.props.bus} isUsingTrackUpMode={this.isUsingTrackUpMode} ndMode={this.currentPageMode} />

            <WindIndicator bus={this.props.bus} />
            <SpeedIndicator bus={this.props.bus} />
            <ToWaypointIndicator
              bus={this.props.bus}
              isNormalOperation={this.pposLatWord.derive((it) => it.isNormalOperation())}
            />
            <TopMessages bus={this.props.bus} ndMode={this.currentPageMode} showOans={this.showOans} />

            {false && <LnavStatus />}
            {true && <VnavStatus />}

            <Flag visible={Signal.of(false)} x={350} y={84} class="Amber FontSmall">
              DISPLAY SYSTEM VERSION INCONSISTENCY
            </Flag>
            <Flag visible={Signal.of(false)} x={384} y={170} class="Amber FontMedium">
              CHECK HDG
            </Flag>
            <Flag visible={this.trkFlagShown} x={381} y={204} class="Red FontSmallest">
              TRK
            </Flag>
            <Flag visible={this.hdgFlagShown} x={384} y={241} class="Red FontLarge">
              HDG
            </Flag>

            <Flag visible={this.rangeChangeInProgress} x={384} y={320} class="Green FontIntermediate mode-range-change">
              {this.props.rangeChangeMessage}
            </Flag>
            <Flag
              visible={Derived.from(
                ([rangeChange, pageChange]) => !rangeChange && pageChange,
                this.rangeChangeInProgress,
                this.pageChangeInProgress,
              )}
              x={384}
              y={320}
              class="Green mode-range-change"
            >
              {this.props.modeChangeMessage}
            </Flag>

            <TerrainMapThresholds bus={this.props.bus} paddingText={this.props.terrainThresholdPaddingText} />

            <RadioNavInfo bus={this.props.bus} index={1} mode={this.currentPageMode} />
            <RadioNavInfo bus={this.props.bus} index={2} mode={this.currentPageMode} />
          </svg>

          {/* ND Raster map - middle layer */}
          <CanvasMap
            bus={this.props.bus}
            x={Signal.of(384)}
            y={Signal.of(384)}
            options={this.props.mapOptions}
          />

          {/* ND Vector graphics - top layer */}
          <svg class="nd-svg nd-top-layer" viewBox="0 0 768 768" style="transform: rotateX(0deg);">
            <Airplane bus={this.props.bus} ndMode={this.currentPageMode} />

            <Chrono bus={this.props.bus} />

            <TcasWxrMessages bus={this.props.bus} mode={this.currentPageMode} />
            <FmMessages bus={this.props.bus} mode={this.currentPageMode} fmMessages={this.props.fmMessages} />
            <CrossTrackError
              bus={this.props.bus}
              currentPageMode={this.currentPageMode}
              isNormalOperation={this.mapFlagShown.derive((it) => !it)}
            />

            <g
              id="radio_needles"
              clip-path={this.currentPageMode.derive((m) => (m === EfisNdMode.ARC ? 'url(#arc-mode-map-clip)' : ''))}
            >
              <RadioNeedle
                bus={this.props.bus}
                headingWord={this.headingWord}
                trackWord={this.trackWord}
                isUsingTrackUpMode={this.isUsingTrackUpMode}
                index={1}
                mode={this.currentPageMode}
              />
              <RadioNeedle
                bus={this.props.bus}
                headingWord={this.headingWord}
                trackWord={this.trackWord}
                isUsingTrackUpMode={this.isUsingTrackUpMode}
                index={2}
                mode={this.currentPageMode}
              />
            </g>
          </svg>
        </div>
      </>
    );
  }
}

class SpeedIndicator extends View<{ bus: Bus }> {
  private readonly groundSpeedRef = UI.ref<SVGTextElement>();

  private readonly trueAirSpeedRef = UI.ref<SVGTextElement>();

  private readonly groundSpeedRegister = Arinc429RegisterSubject.createEmpty();

  private readonly trueAirSpeedRegister = Arinc429RegisterSubject.createEmpty();

  afterRender(node: ViewNode) {
    super.afterRender(node);

    const sub = this.props.bus.inbox<GenericAdirsEvents>();

    sub
      .topic('groundSpeed')
      .limit(2)
      .listen((value) => this.groundSpeedRegister.setWord(value));

    this.groundSpeedRegister.watch((data) => {
      const element = this.groundSpeedRef.target;

      element.textContent = data.isNormalOperation() ? Math.round(data.value).toString() : '';
    }, true);

    sub
      .topic('trueAirSpeed')
      .limit(2)
      .listen((value) => this.trueAirSpeedRegister.setWord(value));

    this.trueAirSpeedRegister.watch((data) => {
      const element = this.trueAirSpeedRef.target;

      element.textContent = data.isNormalOperation() ? Math.round(data.value).toString() : '';
    }, true);
  }

  render(): ViewNode | null {
    return (
      <Layer x={2} y={25}>
        <text x={0} y={0} class="White FontSmallest">
          GS
        </text>
        <text ref={this.groundSpeedRef} x={89} y={0} class="Green FontIntermediate EndAlign" />
        <text x={95} y={0} class="White FontSmallest">
          TAS
        </text>
        <text ref={this.trueAirSpeedRef} x={201} y={0} class="Green FontIntermediate EndAlign" />
      </Layer>
    );
  }
}

interface TrueFlagProps extends FlagProps {
  x: Readable<number>;
  y: Readable<number>;
  boxed: Readable<boolean>;
}

// FIXME add a generic box to Flag
class TrueFlag extends View<TrueFlagProps> {
  private readonly boxRef = UI.ref<SVGTextElement>();

  private readonly boxVisible = Derived.from(
    ([visible, boxed]) => visible && boxed,
    this.props.visible,
    this.props.boxed,
  );

  private readonly boxX = Derived.from(([x]) => x - 34, this.props.x);

  private readonly boxY = Derived.from(([y]) => y - 20, this.props.y);

  afterRender(node: ViewNode) {
    super.afterRender(node);

    this.boxVisible.watch((visible) => {
      this.boxRef.target.style.visibility = visible ? 'inherit' : 'hidden';
    }, true);
  }

  render(): ViewNode | null {
    return (
      <>
        <Flag x={this.props.x} y={this.props.y} class={this.props.class} visible={this.props.visible}>
          TRUE
        </Flag>
        <rect
          x={this.boxX}
          y={this.boxY}
          width={68}
          height={23}
          class={this.props.class}
          strokeWidth={1.5}
          ref={this.boxRef}
        />
      </>
    );
  }
}

interface GridTrackProps {
  x: Readable<number>;
  y: Readable<number>;
  gridTrack: Readable<number>;
  visible: Readable<boolean>;
}

class GridTrack extends View<GridTrackProps> {
  private gridTrackText = Derived.from(
    ([gridTrack]) => gridTrack.toFixed(0).padStart(3, '0'),
    this.props.gridTrack,
  );

  render(): ViewNode | null {
    return (
      <Layer x={this.props.x} y={this.props.y} visible={this.props.visible}>
        <rect x={0} width={94} y={-20} height={23} class="White" strokeWidth={1.5} />
        <text x={45} class="FontSmallest MiddleAlign">
          <tspan class="Green">◇{this.gridTrackText}</tspan>
          <tspan class="Cyan">
            <tspan dx="-5" dy="8" class="FontIntermediate">
              °
            </tspan>
            <tspan dy="-8">G</tspan>
          </tspan>
        </text>
      </Layer>
    );
  }
}

class TopMessages extends View<{
  bus: Bus;
  ndMode: Readable<EfisNdMode>;
  showOans: Readable<boolean>;
}> {
  private readonly sub = this.props.bus.inbox<
    ClockTopics & GenericDisplayManagementEvents & NDSimvars & GenericFmsEvents & FmsOansData
  >();

  private readonly trueRefActive = Signal.of(false);

  private readonly pposLatWord = Arinc429RegisterSubject.createEmpty();

  private readonly pposLonWord = Arinc429RegisterSubject.createEmpty();

  private readonly trueTrackWord = Arinc429RegisterSubject.createEmpty();

  private needApprMessageUpdate = true;

  private apprMessage0: number;

  private apprMessage1: number;

  private readonly approachMessageValue = Signal.of('');

  private readonly btvMessageValue = Signal.of('');

  private readonly isPlanMode = this.props.ndMode.derive((mode) => mode === EfisNdMode.PLAN);

  private readonly gridTrack = Derived.from(
    ([lat, lon, trueTrack]) => clampAngle(Math.round(trueTrack.value - Math.sign(lat.value) * lon.value)),
    this.pposLatWord,
    this.pposLonWord,
    this.trueTrackWord,
  );

  private readonly trueRefVisible = Derived.from(
    ([isTrueRef, isPlanMode]) => isTrueRef && !isPlanMode,
    this.trueRefActive,
    this.isPlanMode,
  );

  private readonly gridTrackVisible = Derived.from(
    ([lat, lon, trueTrack, apprMsg, trueRef]) =>
      trueRef &&
      apprMsg.length === 0 &&
      lon.isNormalOperation() &&
      trueTrack.isNormalOperation() &&
      Math.abs(lat.valueOr(0)) > 65,
    this.pposLatWord,
    this.pposLonWord,
    this.trueTrackWord,
    this.approachMessageValue,
    this.trueRefVisible,
  );

  private readonly trueFlagX = Derived.from(
    ([gridTrack]) => 384 + (gridTrack ? -50 : 4),
    this.gridTrackVisible,
  );

  private readonly trueFlagY = Derived.from(
    ([apprMsg]) => (apprMsg === null ? 36 : 56),
    this.approachMessageValue,
  );

  private readonly trueFlagBoxed = Derived.from(([apprMsg]) => apprMsg.length === 0, this.approachMessageValue);

  private readonly showApproachMessages = this.props.showOans.derive((oans) => !oans);

  afterRender(node: ViewNode) {
    super.afterRender(node);

    this.sub
      .topic('apprMessage0')
      .distinct()
      .listen((value) => {
        this.apprMessage0 = value;
        this.needApprMessageUpdate = true;
      });

    this.sub
      .topic('apprMessage1')
      .distinct()
      .listen((value) => {
        this.apprMessage1 = value;
        this.needApprMessageUpdate = true;
      });

    this.sub
      .topic('ndBtvMessage')
      .distinct()
      .listen((value) => {
        this.btvMessageValue.set(value);
      });

    this.sub.topic('simTime').distinctBy(100).listen(this.refreshApproachMessage.bind(this));

    this.sub.topic('trueTrackRaw').listen((v) => this.trueTrackWord.setWord(v));

    this.sub.topic('latitude').listen((v) => this.pposLatWord.setWord(v));

    this.sub.topic('longitude').listen((v) => this.pposLonWord.setWord(v));

    this.sub
      .topic('trueRefActive')
      .distinct()
      .listen((v) => this.trueRefActive.set(!!v));
  }

  private refreshApproachMessage(): void {
    if (this.needApprMessageUpdate) {
      const ident = SimVarString.unpack([this.apprMessage0, this.apprMessage1]);

      this.approachMessageValue.set(ident);
      this.needApprMessageUpdate = false;
    }
  }

  render(): ViewNode | null {
    return (
      <>
        <Layer x={384} y={25} visible={this.showApproachMessages}>
          <text class="Green FontIntermediate MiddleAlign" style="white-space: pre">
            {this.approachMessageValue}
          </text>
        </Layer>
        <Layer
          x={384}
          y={56}
          visible={Derived.from(
            ([btv, trueRef]) => btv !== '' && !trueRef,
            this.btvMessageValue,
            this.trueRefVisible,
          )}
        >
          {/* TODO verify */}
          <text class="Green FontSmallest MiddleAlign">{this.btvMessageValue}</text>
        </Layer>
        <TrueFlag
          x={this.trueFlagX}
          y={this.trueFlagY}
          class="Cyan FontSmallest"
          boxed={this.trueFlagBoxed}
          visible={this.trueRefVisible}
        />
        <GridTrack
          x={Signal.of(384)}
          y={this.trueFlagY}
          visible={this.gridTrackVisible}
          gridTrack={this.gridTrack}
        />
      </>
    );
  }
}

interface ToWaypointIndicatorProps {
  bus: Bus;
  isNormalOperation: Readable<boolean>; // TODO replace with ARINC429 word
}

class ToWaypointIndicator extends View<ToWaypointIndicatorProps> {
  private readonly sub = this.props.bus.inbox<
    ClockTopics & GenericDisplayManagementEvents & GenericFcuEvents & NDSimvars & GenericFmsEvents
  >();

  private readonly trueRefActive = BusSignal.from(null, false);

  private readonly bearing = BusSignal.from(null, NaN);

  private readonly trueBearing = BusSignal.from(null, NaN);

  private readonly efisMode = BusSignal.from(null, EfisNdMode.ARC);

  private readonly toWptIdent0 = BusSignal.from(null, -1);

  private readonly toWptIdent1 = BusSignal.from(null, -1);

  private readonly toWptDistance = BusSignal.from(null, -1);

  private readonly toWptEta = BusSignal.from(null, -1);

  private readonly largeDistanceNumberRef = UI.ref<SVGTextElement>();

  private readonly smallDistanceIntegerPartRef = UI.ref<SVGTextElement>();

  private readonly smallDistanceDecimalPartRef = UI.ref<SVGTextElement>();

  private readonly visibleSub = Signal.of(false);

  private readonly bearingContainerVisible = Derived.from(
    ([trueRef, bearing, trueBearing, isNormalOperation]) => {
      const activeBearing = trueRef ? trueBearing : bearing;
      return isNormalOperation && Number.isFinite(activeBearing) && activeBearing !== -1;
    },
    this.trueRefActive,
    this.bearing,
    this.trueBearing,
    this.props.isNormalOperation,
  );

  private readonly bearingText = Derived.from(
    ([trueRef, bearing, trueBearing]) =>
      Number.isFinite(trueRef ? trueBearing : bearing)
        ? Math.round(trueRef ? trueBearing : bearing)
            .toString()
            .padStart(3, '0')
        : '',
    this.trueRefActive,
    this.bearing,
    this.trueBearing,
  );

  private readonly toWptIdentValue = Signal.of('');

  private readonly distanceSmallContainerVisible = Signal.of(false);

  private readonly distanceLargeContainerVisible = Signal.of(false);

  private readonly distanceNmUnitVisible = Signal.of(false);

  private readonly etaValue = Signal.of('');

  afterRender(node: ViewNode) {
    super.afterRender(node);

    this.efisMode.rebind(this.sub.topic('ndMode').distinct());

    this.efisMode.watch(() => this.handleVisibility(), true);

    this.toWptIdent0.rebind(this.sub.topic('toWptIdent0').distinct());

    this.toWptIdent1.rebind(this.sub.topic('toWptIdent1').distinct());

    this.toWptDistance.rebind(this.sub.topic('toWptDistance').distinct());

    this.toWptDistance.watch(() => this.handleToWptDistance(), true);
    this.props.isNormalOperation.watch(() => this.handleToWptDistance(), true);

    this.toWptEta.rebind(this.sub.topic('toWptEta').distinct());

    this.toWptEta.watch(() => this.handleToWptEta(), true);
    this.props.isNormalOperation.watch(() => this.handleToWptEta(), true);

    this.bearing.rebind(this.sub.topic('toWptBearing'));

    this.trueBearing.rebind(this.sub.topic('toWptTrueBearing'));

    this.trueRefActive.rebind(this.sub.topic('trueRefActive').distinct());

    this.sub
      .topic('simTime')
      .distinctBy(100)
      .listen(() => {
        this.refreshToWptIdent();
      });
  }

  private handleVisibility() {
    const efisMode = this.efisMode.get();

    const visible = efisMode === EfisNdMode.ROSE_NAV || efisMode === EfisNdMode.ARC || efisMode === EfisNdMode.PLAN;

    this.visibleSub.set(visible);
  }

  // FIXME distance, eta and bearing are currently not transmitted as arinc429 labels which would be needed for proper visibility determination
  private handleToWptDistance() {
    const value = this.toWptDistance.get();

    if (!value || value < 0 || !this.props.isNormalOperation.get()) {
      this.distanceSmallContainerVisible.set(false);
      this.distanceLargeContainerVisible.set(false);
      this.distanceNmUnitVisible.set(false);
      return;
    }

    this.distanceNmUnitVisible.set(true);

    if (value > 20) {
      this.distanceSmallContainerVisible.set(false);
      this.distanceLargeContainerVisible.set(true);

      this.largeDistanceNumberRef.target.textContent = Math.round(value).toString();
    } else {
      this.distanceSmallContainerVisible.set(true);
      this.distanceLargeContainerVisible.set(false);

      const distanceFixed = value.toFixed(1);
      const [distanceIntegralPart, distanceDecimalPart] = distanceFixed.split('.');

      this.smallDistanceIntegerPartRef.target.textContent = distanceIntegralPart.toString();
      this.smallDistanceDecimalPartRef.target.textContent = distanceDecimalPart;
    }
  }

  private handleToWptEta() {
    const eta = this.toWptEta.get();

    if (eta === -1 || !this.props.isNormalOperation.get()) {
      this.etaValue.set('');
      return;
    }

    const hh = Math.floor(eta / 3600);
    const mm = Math.floor((eta % 3600) / 60);

    const utc = `${hh.toString().padStart(2, '0')}:${mm.toString().padStart(2, '0')}`;

    this.etaValue.set(utc);
  }

  private refreshToWptIdent(): void {
    const ident = SimVarString.unpack([this.toWptIdent0.get(), this.toWptIdent1.get()]);

    this.toWptIdentValue.set(ident);
  }

  private readonly visibilityFn = (v: boolean) => (v ? 'inherit' : 'hidden');

  private readonly inverseVisibilityFn = (v: boolean) => (v ? 'hidden' : 'inherit');

  render(): ViewNode | null {
    return (
      <Layer x={690} y={25} visible={this.visibleSub}>
        {/* This is always visible */}
        <text class="White FontIntermediate EndAlign WaypointIndicator" transform="translate(-13, 0)">
          {this.toWptIdentValue}
        </text>

        <g visibility={this.bearingContainerVisible.derive(this.visibilityFn)}>
          <text x={57} y={0} class="Green FontIntermediate EndAlign">
            {this.bearingText}
          </text>
          <text
            x={73}
            y={2}
            class="Cyan FontIntermediate EndAlign"
            visibility={this.trueRefActive.derive(this.inverseVisibilityFn)}
          >
            &deg;
          </text>
          <text
            x={71}
            y={-3}
            class="Cyan FontIntermediate EndAlign"
            visibility={this.trueRefActive.derive(this.visibilityFn)}
          >
            T
          </text>
        </g>

        <g visibility={this.distanceLargeContainerVisible.derive(this.visibilityFn)}>
          <text ref={this.largeDistanceNumberRef} x={39} y={32} class="Green FontIntermediate EndAlign" />
        </g>

        <g visibility={this.distanceSmallContainerVisible.derive(this.visibilityFn)}>
          <text ref={this.smallDistanceIntegerPartRef} x={6} y={32} class="Green FontIntermediate EndAlign" />
          <text x={3} y={32} class="Green FontSmallest StartAlign">
            .
          </text>
          <text ref={this.smallDistanceDecimalPartRef} x={20} y={32} class="Green FontSmallest StartAlign" />
        </g>

        <text
          x={72}
          y={32}
          class="Cyan FontSmallest EndAlign"
          visibility={this.distanceNmUnitVisible.derive(this.visibilityFn)}
        >
          NM
        </text>

        <text x={72} y={66} class="Green FontIntermediate EndAlign">
          {this.etaValue}
        </text>
      </Layer>
    );
  }
}
