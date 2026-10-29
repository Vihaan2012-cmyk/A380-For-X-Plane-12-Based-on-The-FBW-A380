// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import {
  UI,
  View,
  ViewProps,
  Signal,
  Readable,
  ViewNode,
  Bus,
} from 'xplane-js/ui';

import { Arinc429WordData, MathUtils } from '@flybywiresim/fbw-sdk';

import { GenericFcuEvents } from '../../types/GenericFcuEvents';
import { GenericTcasEvents } from '../../types/GenericTcasEvents';

export interface RoseModeOverlayProps<T extends number> {
  bus: Bus;
  heading: Readable<Arinc429WordData>;
  visible: Readable<boolean>;
  rangeValues: T[];
}

export class RoseModeUnderlay<T extends number> extends View<RoseModeOverlayProps<T>> {
  private readonly headingValid = this.props.heading.derive((it) => it.isNormalOperation());

  private readonly rangeValue = Signal.of<number>(10);

  private readonly tcasMode = Signal.of<number>(0);

  private readonly middleRingNoTcasShown = UI.ref<SVGGElement>();

  private readonly middleRingTcasShown = UI.ref<SVGGElement>();

  private readonly smallRingNoTcasShown = UI.ref<SVGGElement>();

  private readonly ringColor = this.headingValid.derive((valid) => {
    if (valid) {
      return 'White';
    }

    return 'Red';
  });

  private readonly headingRingTransform = this.props.heading.derive((heading) => {
    if (heading.isNormalOperation()) {
      return `rotate(${MathUtils.diffAngle(heading.value, 0)} 384 384)`;
    }

    return 'rotate(0 384 384)';
  });

  afterRender(node: ViewNode) {
    super.afterRender(node);

    const sub = this.props.bus.inbox<GenericFcuEvents & GenericTcasEvents>();

    sub
      .topic('ndRangeSetting')
      .distinct()
      .listen((value) => {
        this.rangeValue.set(this.props.rangeValues[value]);

        this.handleRingVisibilities();
      });

    sub
      .topic('tcasMode')
      .distinct()
      .listen((value) => {
        this.tcasMode.set(value);

        this.handleRingVisibilities();
      });
  }

  private handleRingVisibilities() {
    this.middleRingNoTcasShown.target.style.visibility =
      !this.headingValid || this.tcasMode.get() === 0 || this.rangeValue.get() > 10 ? '' : 'hidden';
    this.middleRingTcasShown.target.style.visibility =
      this.headingValid && this.tcasMode.get() > 0 && this.rangeValue.get() <= 10 ? '' : 'hidden';
    this.smallRingNoTcasShown.target.style.visibility =
      this.tcasMode.get() > 0 && this.rangeValue.get() === 20 ? '' : 'hidden';
  }

  render(): ViewNode | null {
    return (
      <>
        <RoseMoveOverlayDefs />

        {/* C = 384,384 */}
        <g transform="rotateX(0deg)" stroke-width={2} fill="none">
          <g clip-path="url(#rose-mode-overlay-clip)">
            <g transform={this.headingRingTransform} class={this.ringColor}>
              <RoseModeOverlayHeadingRing headingValid={this.headingValid} />
            </g>
          </g>

          {/* R = 125, middle range ring */}
          <g ref={this.middleRingNoTcasShown}>
            <path
              class={this.ringColor}
              d="M 509 384 A 125 125 0 0 1 259 384 M 259 384 A 125 125 180 0 1 509 384"
              stroke-dasharray="15 10"
              stroke-dashoffset="-4.2"
            />
          </g>

          {/* middle range ring replaced with tcas range ticks */}
          <g ref={this.middleRingTcasShown} class={this.ringColor}>
            <line x1={384} x2={384} y1={264} y2={254} class="White rounded" transform="rotate(0 384 384)" />
            <line x1={384} x2={384} y1={264} y2={254} class="White rounded" transform="rotate(30 384 384)" />
            <line x1={384} x2={384} y1={264} y2={254} class="White rounded" transform="rotate(60 384 384)" />
            <line x1={384} x2={384} y1={264} y2={254} class="White rounded" transform="rotate(90 384 384)" />
            <line x1={384} x2={384} y1={264} y2={254} class="White rounded" transform="rotate(120 384 384)" />
            <line x1={384} x2={384} y1={264} y2={254} class="White rounded" transform="rotate(150 384 384)" />
            <line x1={384} x2={384} y1={264} y2={254} class="White rounded" transform="rotate(180 384 384)" />
            <line x1={384} x2={384} y1={264} y2={254} class="White rounded" transform="rotate(210 384 384)" />
            <line x1={384} x2={384} y1={264} y2={254} class="White rounded" transform="rotate(240 384 384)" />
            <line x1={384} x2={384} y1={264} y2={254} class="White rounded" transform="rotate(270 384 384)" />
            <line x1={384} x2={384} y1={264} y2={254} class="White rounded" transform="rotate(300 384 384)" />
            <line x1={384} x2={384} y1={264} y2={254} class="White rounded" transform="rotate(330 384 384)" />
          </g>

          {/* R = 62, tcas range ticks */}
          <g ref={this.smallRingNoTcasShown} class={this.ringColor}>
            <line x1={384} x2={384} y1={327} y2={317} class="White rounded" transform="rotate(0 384 384)" />
            <line x1={384} x2={384} y1={327} y2={317} class="White rounded" transform="rotate(30 384 384)" />
            <line x1={384} x2={384} y1={327} y2={317} class="White rounded" transform="rotate(60 384 384)" />
            <line x1={384} x2={384} y1={327} y2={317} class="White rounded" transform="rotate(90 384 384)" />
            <line x1={384} x2={384} y1={327} y2={317} class="White rounded" transform="rotate(120 384 384)" />
            <line x1={384} x2={384} y1={327} y2={317} class="White rounded" transform="rotate(150 384 384)" />
            <line x1={384} x2={384} y1={327} y2={317} class="White rounded" transform="rotate(180 384 384)" />
            <line x1={384} x2={384} y1={327} y2={317} class="White rounded" transform="rotate(210 384 384)" />
            <line x1={384} x2={384} y1={327} y2={317} class="White rounded" transform="rotate(240 384 384)" />
            <line x1={384} x2={384} y1={327} y2={317} class="White rounded" transform="rotate(270 384 384)" />
            <line x1={384} x2={384} y1={327} y2={317} class="White rounded" transform="rotate(300 384 384)" />
            <line x1={384} x2={384} y1={327} y2={317} class="White rounded" transform="rotate(330 384 384)" />
          </g>

          <text x={212} y={556} class="Cyan" font-size={22}>
            {this.rangeValue.derive((it) => (it / 2).toString())}
          </text>
          <text x={310} y={474} class="Cyan" font-size={22}>
            {this.rangeValue.derive((it) => (it / 4).toString())}
          </text>

          {/* fixed triangle markers every 45 deg except 12 o'clock */}
          <g class={this.ringColor} visibility={this.headingValid.derive((valid) => (valid ? 'inherit' : 'hidden'))}>
            <path d="M384,132 L379,123 L389,123 L384,132" transform="rotate(45 384 384)" fill="white" />
            <path d="M384,132 L379,123 L389,123 L384,132" transform="rotate(90 384 384)" fill="white" />
            <path d="M384,132 L379,123 L389,123 L384,132" transform="rotate(135 384 384)" fill="white" />
            <path d="M384,132 L379,123 L389,123 L384,132" transform="rotate(180 384 384)" fill="white" />
            <path d="M384,132 L379,123 L389,123 L384,132" transform="rotate(225 384 384)" fill="white" />
            <path d="M384,132 L379,123 L389,123 L384,132" transform="rotate(270 384 384)" fill="white" />
            <path d="M384,132 L379,123 L389,123 L384,132" transform="rotate(315 384 384)" fill="white" />
          </g>
        </g>
      </>
    );
  }
}

export class RoseMoveOverlayDefs extends View<{}> {
  render(): ViewNode | null {
    return (
      <>
        <clipPath id="rose-mode-map-clip">
          <path d="M45,155 L282,155 a250,250 0 0 1 204,0 L723,155 L723,562 L648,562 L591,625 L591,768 L174,768 L174,683 L122,625 L45,625 L45,155" />
        </clipPath>
        <clipPath id="rose-mode-wx-terr-clip">
          <path d="M45,155 L282,155 a250,250 0 0 1 204,0 L723,155 L723,384 L45,384 L45,155" />
        </clipPath>
        <clipPath id="rose-mode-tcas-clip">
          <path d="M45,155 L282,155 a250,250 0 0 1 204,0 L723,155 L723,562 L648,562 L591,625 L591,768 L174,768 L174,683 L122,625 L45,625 L45,155" />
        </clipPath>
      </>
    );
  }
}

interface RoseModeOverlayHeadingRingProps extends ViewProps {
  headingValid: Readable<boolean>;
}

export class RoseModeOverlayHeadingRing extends View<RoseModeOverlayHeadingRingProps> {
  render(): ViewNode | null {
    return (
      <>
        {/* R = 250 */}
        <circle cx={384} cy={384} r={250} />

        <g visibility={this.props.headingValid.derive((valid) => (valid ? 'inherit' : 'hidden'))}>
          <g transform="rotate(0 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
            <text x={384} y={112} text-anchor="middle" font-size={22} fill="white" stroke="none">
              0
            </text>
          </g>

          <g transform="rotate(5 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(10 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(15 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(20 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(25 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(30 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
            <text x={384} y={112} text-anchor="middle" font-size={22} fill="white" stroke="none">
              3
            </text>
          </g>

          <g transform="rotate(35 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(40 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(45 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(50 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(55 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(60 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
            <text x={384} y={112} text-anchor="middle" font-size={22} fill="white" stroke="none">
              6
            </text>
          </g>

          <g transform="rotate(65 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(70 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(75 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(80 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(85 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(90 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
            <text x={384} y={112} text-anchor="middle" font-size={22} fill="white" stroke="none">
              9
            </text>
          </g>

          <g transform="rotate(95 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(100 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(105 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(110 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(115 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(120 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
            <text x={384} y={112} text-anchor="middle" font-size={22} fill="white" stroke="none">
              12
            </text>
          </g>

          <g transform="rotate(125 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(130 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(135 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(140 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(145 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(150 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
            <text x={384} y={112} text-anchor="middle" font-size={22} fill="white" stroke="none">
              15
            </text>
          </g>

          <g transform="rotate(155 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(160 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(165 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(170 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(175 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(180 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
            <text x={384} y={112} text-anchor="middle" font-size={22} fill="white" stroke="none">
              18
            </text>
          </g>

          <g transform="rotate(185 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(190 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(195 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(200 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(205 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(210 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
            <text x={384} y={112} text-anchor="middle" font-size={22} fill="white" stroke="none">
              21
            </text>
          </g>

          <g transform="rotate(215 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(220 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(225 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(230 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(235 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(240 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
            <text x={384} y={112} text-anchor="middle" font-size={22} fill="white" stroke="none">
              24
            </text>
          </g>

          <g transform="rotate(245 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(250 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(255 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(260 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(265 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(270 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
            <text x={384} y={112} text-anchor="middle" font-size={22} fill="white" stroke="none">
              27
            </text>
          </g>

          <g transform="rotate(275 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(280 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(285 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(290 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(295 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(300 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
            <text x={384} y={112} text-anchor="middle" font-size={22} fill="white" stroke="none">
              30
            </text>
          </g>

          <g transform="rotate(305 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(310 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(315 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(320 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(325 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(330 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
            <text x={384} y={112} text-anchor="middle" font-size={22} fill="white" stroke="none">
              33
            </text>
          </g>

          <g transform="rotate(335 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(340 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(345 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>

          <g transform="rotate(350 384 384)">
            <line x1={384} y1={134} x2={384} y2={122} stroke-width={2.5} />
          </g>

          <g transform="rotate(355 384 384)">
            <line x1={384} y1={134} x2={384} y2={128} stroke-width={2.5} />
          </g>
        </g>
      </>
    );
  }
}
