// @ts-strict-ignore
// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import {
  UI,
  View,
  ViewProps,
  Derived,
  Signal,
  Readable,
  ViewNode,
} from 'xplane-js/ui';

import { Arinc429WordData, Arinc429ConsumerSubject, GenericAdirsEvents } from '@flybywiresim/fbw-sdk';

import { RoseMode, RoseModeProps } from './RoseMode';
import { RoseModeUnderlay } from './RoseModeUnderlay';
import { Flag } from '../../shared/Flag';
import { NDControlEvents } from '../../NDControlEvents';
import { VorInfoIndicator } from './VorInfoIndicator';
import { GenericDisplayManagementEvents } from '../../types/GenericDisplayManagementEvents';
import { GenericVorEvents } from '../../types/GenericVorEvents';
import { NavMath } from 'xplane-js/data';

export interface RoseVorProps<T extends number> extends RoseModeProps<T> {
  index: 1 | 2;
}

export class RoseVorPage<T extends number> extends RoseMode<T, RoseVorProps<T>> {
  isVisible = Signal.of(false);

  private readonly headingWord = Arinc429ConsumerSubject.create(null);

  private readonly courseSub = Signal.of(0);

  private readonly courseDeviationSub = Signal.of(0);

  private readonly vorAvailableSub = Signal.of(false);

  private readonly vorFrequencySub = Signal.of(0);

  onShow() {
    super.onShow();

    const publisher = this.props.bus.outbox<NDControlEvents>();

    publisher.post('set_show_map', false);
  }

  afterRender(node: ViewNode) {
    super.afterRender(node);

    const sub = this.props.bus.inbox<GenericAdirsEvents & GenericDisplayManagementEvents & GenericVorEvents>();

    const index = this.props.index;

    this.headingWord.setConsumer(sub.topic('heading'));

    sub
      .topic(`nav${index}Obs`)
      .distinct()
      .listen((v) => this.courseSub.set(v));

    sub
      .topic(`nav${index}RadialError`)
      .distinct()
      .listen((v) => this.courseDeviationSub.set(v));

    sub
      .topic(`nav${index}Available`)
      .distinct()
      .listen((v) => this.vorAvailableSub.set(v));

    sub
      .topic(`nav${index}Frequency`)
      .distinct()
      .listen((v) => this.vorFrequencySub.set(v));
  }

  private readonly hdgFlagShown = Derived.from(
    ([headingWord]) => !headingWord.isNormalOperation(),
    this.headingWord,
  );

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

        <VorInfoIndicator bus={this.props.bus} index={this.props.index} />

        <RoseModeUnderlay
          bus={this.props.bus}
          heading={this.props.headingWord}
          visible={this.isVisible}
          rangeValues={this.props.rangeValues}
        />

        <VorCaptureOverlay
          index={this.props.index}
          heading={this.props.headingWord}
          course={this.courseSub}
          courseDeviation={this.courseDeviationSub}
          vorAvailable={this.vorAvailableSub}
          vorFrequency={this.vorFrequencySub}
        />

        <Flag visible={this.hdgFlagShown} x={384} y={241} class="Red FontLarge">
          HDG
        </Flag>
      </g>
    );
  }
}

interface VorCaptureOverlayProps extends ViewProps {
  index: 1 | 2;
  heading: Readable<Arinc429WordData>;
  course: Readable<number>;
  courseDeviation: Readable<number>;
  vorAvailable: Readable<boolean>;
  vorFrequency: Readable<number>;
}

class VorCaptureOverlay extends View<VorCaptureOverlayProps> {
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

  /*     useEffect(() => {
        let cdiDegrees: number;
        if (Math.abs(courseDeviation) <= 90) {
            cdiDegrees = courseDeviation;
            setToward(true);
        } else {
            cdiDegrees = Math.sign(courseDeviation) * -Avionics.Utils.diffAngle(180, Math.abs(courseDeviation));
            setToward(false);
        }
        setCdiPx(Math.min(12, Math.max(-12, cdiDegrees)) * 74 / 5);
    }, [courseDeviation.toFixed(2)]);
 */

  private readonly cdiPx = Signal.of(12);

  private readonly toward = Signal.of(true);

  private readonly directionTransform = Derived.from(
    ([cdiPx, toward]) => {
      return `translate(${cdiPx}, ${toward ? 0 : 160}) rotate(${toward ? 0 : 180} 384 304)`;
    },
    this.cdiPx,
    this.toward,
  );

  private readonly deviationTransform = Derived.from(([cdiPx]) => {
    return `translate(${cdiPx}, 0)`;
  }, this.cdiPx);

  afterRender(node: ViewNode): void {
    super.afterRender(node);
    this.props.courseDeviation.watch((courseDeviation) => {
      let cdiDegrees: number;
      if (Math.abs(courseDeviation) <= 90) {
        cdiDegrees = courseDeviation;
        this.toward.set(true);
      } else {
        cdiDegrees = Math.sign(courseDeviation) * -NavMath.Geo.angleDelta(180, Math.abs(courseDeviation));
        this.toward.set(false);
      }
      this.cdiPx.set((Math.min(12, Math.max(-12, cdiDegrees)) * 74) / 5);
    });
  }

  render(): ViewNode | null {
    return (
      <g
        visibility={this.visible.derive((visible) => (visible ? 'inherit' : 'hidden'))}
        transform={this.rotation.derive((deg) => `rotate(${deg} 384 384)`)}
        stroke="white"
        stroke-width={3}
        fill="none"
      >
        <g id="vor-deviation-scale">
          <circle cx={236} cy={384} r={5} />
          <circle cx={310} cy={384} r={5} />
          <circle cx={458} cy={384} r={5} />
          <circle cx={532} cy={384} r={5} />
        </g>

        <g visibility={this.props.vorFrequency.derive((v) => (v > 0 ? 'inherit' : 'hidden'))}>
          <path
            d="M352,256 L416,256 M384,134 L384,294 M384,474 L384,634"
            class="rounded shadow"
            id="vor-course-pointer-shadow"
            stroke-width={4.5}
          />
          <path
            d="M352,256 L416,256 M384,134 L384,294 M384,474 L384,634"
            class={this.pointerColor.derive((color) => `rounded ${color}`)}
            id="vor-course-pointer"
            stroke-width={4}
          />
        </g>

        <g visibility={this.props.vorAvailable.derive((available) => (available ? 'inherit' : 'hidden'))}>
          <path
            d="M372,322 L384,304 L396,322"
            class="rounded shadow"
            transform={this.directionTransform}
            id="vor-deviation-direction-shadow"
            stroke-width={4.5}
          />
          <path
            d="M384,304 L384,464"
            class="rounded shadow"
            transform={this.deviationTransform}
            id="vor-deviation-shadow"
            stroke-width={4.5}
          />
          <path
            d="M372,322 L384,304 L396,322"
            class={this.pointerColor.derive((color) => `rounded ${color}`)}
            transform={this.directionTransform}
            id="vor-deviation-direction"
            stroke-width={4}
          />
          <path
            d="M384,304 L384,464"
            class={this.pointerColor.derive((color) => `rounded ${color}`)}
            transform={this.deviationTransform}
            id="vor-deviation"
            stroke-width={4}
          />
        </g>
      </g>
    );
  }
}
