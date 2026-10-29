// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import {
  UI,
  ViewProps,
  BusSignal,
  Derived,
  Signal,
  Readable,
  ViewNode,
} from 'xplane-js/ui';

import {
  ArincEventBus,
  Arinc429WordData,
  Arinc429RegisterSubject,
  EfisNdMode,
  MathUtils,
  GenericAdirsEvents,
} from '@flybywiresim/fbw-sdk';

import { LsCourseBug } from './LsCourseBug';
import { ArcModeUnderlay } from './ArcModeUnderlay';
import { Flag } from '../../shared/Flag';
import { NDPage } from '../NDPage';
import { NDControlEvents } from '../../NDControlEvents';
import { GenericFcuEvents } from '../../types/GenericFcuEvents';
import { GenericFmsEvents } from '../../types/GenericFmsEvents';

export interface ArcModePageProps<T extends number> extends ViewProps {
  bus: ArincEventBus;
  rangeValues: T[];
  headingWord: Readable<Arinc429WordData>;
  trueHeadingWord: Readable<Arinc429WordData>;
  trackWord: Readable<Arinc429WordData>;
  trueTrackWord: Readable<Arinc429WordData>;
  isUsingTrackUpMode: Readable<boolean>;
}

export class ArcModePage<T extends number> extends NDPage<ArcModePageProps<T>> {
  public isVisible = Signal.of(false);

  // TODO these two should be FM pos maybe ?

  private readonly pposLatWord = Arinc429RegisterSubject.createEmpty();

  private readonly pposLonWord = Arinc429RegisterSubject.createEmpty();

  private readonly mapRangeSub = BusSignal.from(
    this.props.bus.inbox<GenericFcuEvents>().topic('ndRangeSetting').distinct(),
    -1,
  );

  private readonly ringAvailable = Derived.from(
    ([isUsingTrackUpMode, headingWord, trackWord]) => {
      if (isUsingTrackUpMode) {
        return headingWord.isNormalOperation() && trackWord.isNormalOperation();
      }

      return headingWord.isNormalOperation();
    },
    this.props.isUsingTrackUpMode,
    this.props.headingWord,
    this.props.trackWord,
  );

  private readonly ringRotation = Signal.of<number>(0);

  private readonly planeRotation = Derived.from(
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

  private readonly fmsFailed = BusSignal.from(
    this.props.bus.inbox<GenericFmsEvents>().topic('fmsFailed').distinct(),
    false,
  );

  // TODO in the future, this should be looking at stuff like FM position invalid or not map frames transmitted
  private readonly mapFlagShown = Derived.from(
    ([headingWord, latWord, longWord, fmsFailed]) => {
      return (
        !headingWord.isNormalOperation() || !latWord.isNormalOperation() || !longWord.isNormalOperation() || fmsFailed
      );
    },
    this.props.headingWord,
    this.pposLatWord,
    this.pposLonWord,
    this.fmsFailed,
  );

  onShow() {
    super.onShow();

    this.handleMovePlane();
    this.handleRotatePlane();
    this.handleMoveMap();
    this.handleMapRotation();
    this.handleScaleMap();
  }

  afterRender(node: ViewNode) {
    super.afterRender(node);

    const sub = this.props.bus.inbox<GenericAdirsEvents & GenericFcuEvents>();

    sub
      .topic('latitude')
      .distinct()
      .listen((v) => this.pposLatWord.setWord(v));
    sub
      .topic('longitude')
      .distinct()
      .listen((v) => this.pposLonWord.setWord(v));

    this.props.headingWord.watch(() => this.handleRingRotation());
    this.props.trueHeadingWord.watch(() => this.handleMapRotation());
    this.props.trackWord.watch(() => this.handleRingRotation());
    this.props.trueTrackWord.watch(() => this.handleMapRotation());
    this.props.isUsingTrackUpMode.watch(() => this.handleRingRotation());

    this.planeRotation.watch(() => {
      if (this.isVisible.get()) {
        this.handleRotatePlane();
      }
    });

    this.pposLatWord.watch(() => this.handleMoveMap());
    this.pposLonWord.watch(() => this.handleMoveMap());

    this.mapRangeSub.watch(() => this.handleScaleMap());
  }

  private handleRingRotation() {
    const isUsingTrackUpMode = this.props.isUsingTrackUpMode.get();

    const rotationWord = isUsingTrackUpMode ? this.props.trackWord.get() : this.props.headingWord.get();

    if (rotationWord.isNormalOperation()) {
      this.ringRotation.set(rotationWord.value);
    }
  }

  private handleMapRotation() {
    if (!this.isVisible.get()) {
      return;
    }

    const isUsingTrackUpMode = this.props.isUsingTrackUpMode.get();

    const rotationWord = isUsingTrackUpMode ? this.props.trueTrackWord.get() : this.props.trueHeadingWord.get();

    const publisher = this.props.bus.outbox<NDControlEvents>();

    if (rotationWord.isNormalOperation()) {
      publisher.post('set_map_up_course', rotationWord.value);
    } else {
      publisher.post('set_map_up_course', -1);
    }
  }

  private handleMovePlane() {
    if (!this.isVisible.get()) {
      return;
    }

    const publisher = this.props.bus.outbox<NDControlEvents>();

    publisher.post('set_show_plane', true);
    publisher.post('set_plane_x', 384);
    publisher.post('set_plane_y', 626);
    publisher.post('set_show_map', true);
  }

  private handleMoveMap() {
    if (!this.isVisible.get()) {
      return;
    }

    const publisher = this.props.bus.outbox<NDControlEvents>();

    const latWord = this.pposLatWord.get();
    const lonWord = this.pposLonWord.get();

    if (latWord.isNormalOperation() && lonWord.isNormalOperation()) {
      publisher.post('set_show_map', true);
      publisher.post('set_map_center_lat', latWord.value);
      publisher.post('set_map_center_lon', lonWord.value);
    } else {
      publisher.post('set_show_map', false);
    }
  }

  private handleRotatePlane() {
    if (!this.isVisible.get()) {
      return;
    }

    const publisher = this.props.bus.outbox<NDControlEvents>();

    publisher.post('set_plane_rotation', this.planeRotation.get());
  }

  private handleScaleMap() {
    if (!this.isVisible.get()) {
      return;
    }

    const publisher = this.props.bus.outbox<NDControlEvents>();

    publisher.post('set_map_efis_mode', EfisNdMode.ARC);
    publisher.post('set_map_pixel_radius', 498);
    publisher.post('set_map_range_radius', this.props.rangeValues[this.mapRangeSub.get()]);
    publisher.post('set_map_center_y_bias', 242);
  }

  render(): ViewNode | null {
    return (
      <g visibility={this.isVisible.derive((visible) => (visible ? 'visible' : 'hidden'))}>
        {/* inverted map overlays for terrain map in WASM module  */}
        <path
          name="arc-mode-bottom-left-map-area"
          d="M0,625 L122,625 L174,683 L174,768 L0,768 L0,625"
          class="nd-inverted-map-area"
        />
        <path
          name="arc-mode-bottom-right-map-area"
          d="M768,562 L648,562 L591,625 L591,768 L768,768 L768,562"
          class="nd-inverted-map-area"
        />
        <path
          name="arc-mode-top-map-area"
          d="M0,0 L0,312 a492,492 0 0 1 768,0 L768,0 L0,0"
          class="nd-inverted-map-area a380xInvisible"
        />

        <ArcModeUnderlay
          bus={this.props.bus}
          ringAvailable={this.ringAvailable}
          ringRotation={this.ringRotation}
          rangeValues={this.props.rangeValues}
        />

        <LsCourseBug bus={this.props.bus} rotationOffset={this.planeRotation} mode={Signal.of(EfisNdMode.ARC)} />

        <Flag visible={this.mapFlagShown} x={384} y={320.6} class="Red FontLarge">
          MAP NOT AVAIL
        </Flag>
      </g>
    );
  }
}
