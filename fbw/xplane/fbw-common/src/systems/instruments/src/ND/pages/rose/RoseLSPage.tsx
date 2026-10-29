// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import {
  ViewProps,
  BusSignal,
  View,
  UI,
  Derived,
  Signal,
  Readable,
  ViewNode,
} from 'xplane-js/ui';
import { Arinc429WordData, MathUtils, GenericAdirsEvents } from '@flybywiresim/fbw-sdk';

import { RoseMode, RoseModeProps } from './RoseMode';
import { RoseModeUnderlay } from './RoseModeUnderlay';
import { NDControlEvents } from '../../NDControlEvents';
import { IlsInfoIndicator } from './IlsInfoIndicator';
import { GlideSlope } from './Glideslope';
import { GenericDisplayManagementEvents } from '../../types/GenericDisplayManagementEvents';
import { GenericVorEvents } from '../../types/GenericVorEvents';
import { GenericFlightManagementBusEvents } from '../../types/GenericFlightManagementBusEvents';

export interface RoseLsProps<T extends number> extends RoseModeProps<T> {
  index: 1 | 2;
}

export class RoseLSPage<T extends number> extends RoseMode<T, RoseLsProps<T>> {
  private readonly sub = this.props.bus.inbox<
    GenericAdirsEvents & GenericDisplayManagementEvents & GenericVorEvents & GenericFlightManagementBusEvents
  >();

  isVisible = Signal.of(false);

  //  private readonly headingWord = Arinc429ConsumerSubject.create(null);

  private readonly courseSub = Signal.of(0);

  private readonly courseDeviationSub = Signal.of(0);

  private readonly ilsAvailableSub = Signal.of(false);

  private readonly ilsFrequencySub = Signal.of(0);

  private readonly localizerValidSub = Signal.of(false);

  private readonly backbeam = BusSignal.from(this.sub.topic('fm.1.backbeam'), false);

  onShow() {
    super.onShow();

    const publisher = this.props.bus.outbox<NDControlEvents>();

    const index: 3 | 4 = (this.props.index + 2) as 3 | 4;

    this.sub
      .topic(`nav${index}Obs`)
      .distinct()
      .listen((v) => this.courseSub.set(v));

    this.sub
      .topic(`nav${index}RadialError`)
      .distinct()
      .listen((v) => this.courseDeviationSub.set(v));

    this.sub
      .topic(`nav${index}Available`)
      .distinct()
      .listen((v) => this.ilsAvailableSub.set(v));

    this.sub
      .topic(`nav${index}Frequency`)
      .distinct()
      .listen((v) => this.ilsFrequencySub.set(v));

    this.sub
      .topic('localizerValid')
      .distinct()
      .listen((v) => this.localizerValidSub.set(v));

    publisher.post('set_show_map', false);
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

        <IlsInfoIndicator bus={this.props.bus} index={this.props.index} />

        <GlideSlope bus={this.props.bus} backbeam={this.backbeam} />

        <RoseModeUnderlay
          bus={this.props.bus}
          heading={this.props.headingWord}
          visible={this.isVisible}
          rangeValues={this.props.rangeValues}
        />

        <IlsCaptureOverlay
          heading={this.props.headingWord}
          course={this.courseSub}
          courseDeviation={this.courseDeviationSub}
          backbeam={this.backbeam}
          available={this.localizerValidSub}
          ilsFrequency={this.ilsFrequencySub}
        />
      </g>
    );
  }
}

interface IlsCaptureOverlayProps extends ViewProps {
  heading: Readable<Arinc429WordData>;
  course: Readable<number>;
  courseDeviation: Readable<number>;
  backbeam: Readable<boolean>;
  available: Readable<boolean>;
  ilsFrequency: Readable<number>;
}

class IlsCaptureOverlay extends View<IlsCaptureOverlayProps> {
  // we can't tell if the course is valid from the MSFS radio, so at least check that the frequency is
  private readonly pointerVisibilitySub = Derived.from(([ilsFrequency]) => {
    return ilsFrequency >= 108 && ilsFrequency <= 112 ? 'inherit' : 'hidden';
  }, this.props.ilsFrequency);

  private readonly visible = Derived.from(([heading]) => {
    return heading.isNormalOperation();
  }, this.props.heading);

  private readonly rotation = Derived.from(
    ([heading, course]) => {
      if (heading.isNormalOperation()) {
        return course - heading.value;
      }
      return 0;
    },
    this.props.heading,
    this.props.course,
  );

  private readonly pointerColor = Derived.from(([heading]) => {
    if (heading.isNormalOperation()) {
      return 'Cyan';
    }

    return 'White';
  }, this.props.heading);

  private readonly deviation = Derived.from(
    ([courseDeviation, backbeam]) => {
      const dots =
        (backbeam ? -1 : 1) * Math.max(-2, Math.min(2, MathUtils.correctMsfsLocaliserError(courseDeviation) / 0.8));
      return dots * 74;
    },
    this.props.courseDeviation,
    this.props.backbeam,
  );

  // FIXME hook up when MMR ready
  private readonly mixLocVnav = Signal.of(false);

  private readonly locBcReminderText = Derived.from(
    ([backbeam, mixLocVnav]) => (backbeam ? 'B/C' : mixLocVnav ? 'LOC' : ''),
    this.props.backbeam,
    this.mixLocVnav,
  );

  render(): ViewNode {
    return (
      <g
        visibility={this.visible.derive((visible) => (visible ? 'inherit' : 'hidden'))}
        transform={this.rotation.derive((deg) => `rotate(${deg} 384 384)`)}
        stroke="white"
        strokeWidth={3}
        fill="none"
      >
        <g id="loc-bc-reminder">
          {/* FIXME don't have a good reference for the position of this. Only doc illustrations. */}
          <text x={202} y={416} class="FontSmall Magenta">
            {this.locBcReminderText}
          </text>
        </g>

        <g id="ils-deviation-scale">
          <circle cx={236} cy={384} r={5} />
          <circle cx={310} cy={384} r={5} />
          <circle cx={458} cy={384} r={5} />
          <circle cx={532} cy={384} r={5} />
        </g>

        <g visibility={this.pointerVisibilitySub}>
          <path
            d="M352,256 L416,256 M384,134 L384,294 M384,474 L384,634"
            class="rounded shadow"
            id="ils-course-pointer-shadow"
            stroke-width={4.5}
          />
          <path
            d="M352,256 L416,256 M384,134 L384,294 M384,474 L384,634"
            class="Magenta rounded"
            id="ils-course-pointer"
            stroke-width={4}
          />
        </g>

        <g visibility={this.props.available.derive((a) => (a ? 'inherit' : 'hidden'))}>
          <path
            d="M384,304 L384,464"
            class="rounded shadow"
            transform={this.deviation.derive((cdiPx) => `translate(${cdiPx}, 0)`)}
            id="ils-deviation-shadow"
            stroke-width={4.5}
          />
          <path
            d="M384,304 L384,464"
            class="Magenta rounded"
            transform={this.deviation.derive((cdiPx) => `translate(${cdiPx}, 0)`)}
            id="ils-deviation"
            stroke-width={4}
          />
        </g>
      </g>
    );
  }
}
