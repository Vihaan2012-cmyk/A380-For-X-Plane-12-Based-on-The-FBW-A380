// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import { UI, BusSignal, Derived, Signal, ViewNode } from 'xplane-js/ui';

import { Arinc429RegisterSubject, EfisNdMode, MathUtils, GenericAdirsEvents } from '@flybywiresim/fbw-sdk';

import { LsCourseBug } from '../arc/LsCourseBug';
import { Flag } from '../../shared/Flag';
import { RoseMode } from './RoseMode';
import { RoseModeUnderlay } from './RoseModeUnderlay';
import { NDControlEvents } from '../../NDControlEvents';
import { GenericFcuEvents } from '../../types/GenericFcuEvents';
import { GenericFmsEvents } from '../../types/GenericFmsEvents';

export class RoseNavPage<T extends number> extends RoseMode<T> {
  private readonly pposLatWord = Arinc429RegisterSubject.createEmpty();

  private readonly pposLonWord = Arinc429RegisterSubject.createEmpty();

  private readonly mapRangeSub = BusSignal.from(
    this.props.bus.inbox<GenericFcuEvents>().topic('ndRangeSetting').distinct(),
    -1,
  );

  private readonly fmsFailed = BusSignal.from(
    this.props.bus.inbox<GenericFmsEvents>().topic('fmsFailed').distinct(),
    false,
  );

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

  isVisible = Signal.of(false);

  afterRender(node: ViewNode) {
    super.afterRender(node);

    this.isVisible.watch((visible) => {
      if (visible) {
        this.handleMoveMap();
        this.handleScaleMap();
      }
    });

    const sub = this.props.bus.getArincSubscriber<GenericAdirsEvents & GenericFcuEvents>();

    sub
      .on('latitude')
      .distinct()
      .listen((v) => this.pposLatWord.setWord(v));
    sub
      .on('longitude')
      .distinct()
      .listen((v) => this.pposLonWord.setWord(v));

    this.props.isUsingTrackUpMode.watch(() => this.handleRotateMap());
    this.props.trueHeadingWord.watch(() => {
      if (!this.props.isUsingTrackUpMode.get()) {
        this.handleRotateMap();
      }
    });
    this.props.trueTrackWord.watch(() => {
      if (this.props.isUsingTrackUpMode.get()) {
        this.handleRotateMap();
      }
    });

    this.pposLatWord.watch(() => this.handleMoveMap());
    this.pposLonWord.watch(() => this.handleMoveMap());

    this.mapRangeSub.watch(() => this.handleScaleMap());
  }

  private handleRotateMap() {
    if (!this.isVisible.get()) {
      return;
    }

    const publisher = this.props.bus.outbox<NDControlEvents>();

    const rotation = this.props.isUsingTrackUpMode.get()
      ? this.props.trueTrackWord.get()
      : this.props.trueHeadingWord.get();

    if (rotation.isNormalOperation()) {
      publisher.post('set_map_up_course', rotation.value);
    } else {
      publisher.post('set_show_map', false);
    }
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

  private handleScaleMap() {
    if (!this.isVisible.get()) {
      return;
    }

    const publisher = this.props.bus.outbox<NDControlEvents>();

    const rangeSetting = this.mapRangeSub.get();
    const range = this.props.rangeValues[rangeSetting];

    publisher.post('set_map_efis_mode', EfisNdMode.ROSE_NAV);
    publisher.post('set_map_pixel_radius', 250);
    publisher.post('set_map_range_radius', range / 2);
    publisher.post('set_map_center_y_bias', 0);
  }

  render(): ViewNode | null {
    return (
      <g visibility={this.isVisible.derive((v) => (v ? 'visible' : 'hidden'))}>
        {/* inverted map overlays for terrain map in WASM module  */}
        <path
          name="rose-mode-bottom-left-map-area"
          d="M45,625 L122,625 L174,683 L174,768 L0,768 L0,0 L45,0L45,625"
          class="nd-inverted-map-area"
        />
        <path
          name="rose-mode-bottom-right-map-area"
          d="M591,768 L591,626 L648,562 L723,562 L723,0 L768,0 L768,768 L591,769"
          class="nd-inverted-map-area"
        />
        <path
          name="rose-mode-top-map-area"
          d="M45,0 L45,155, L282,155 a250,250 0 0 1 204,0 L723,155 L723,0 L45,0"
          class="nd-inverted-map-area"
        />

        <RoseModeUnderlay
          bus={this.props.bus}
          heading={this.props.headingWord}
          visible={this.isVisible}
          rangeValues={this.props.rangeValues}
        />

        <LsCourseBug
          bus={this.props.bus}
          rotationOffset={this.planeRotation}
          mode={Signal.of(EfisNdMode.ROSE_NAV)}
        />

        <Flag visible={this.mapFlagShown} x={384} y={320.6} class="Red FontLarge">
          MAP NOT AVAIL
        </Flag>
      </g>
    );
  }
}
