// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import {
  View,
  UI,
  Derived,
  Ref,
  Signal,
  Readable,
  ViewNode,
  PanelEventTopics,
  ClockTopics,
} from 'xplane-js/ui';
import { ArincEventBus, Arinc429RegisterSubject, Arinc429Word } from '@flybywiresim/fbw-sdk';
import { Arinc429Values } from './shared/ArincValueProvider';
import { HUDSimvars } from './shared/HUDSimvarPublisher';
import { HudElems, XWIND_TO_AIR_REF_OFFSET, XWIND_FULL_OFFSET } from './HUDUtils';
const UnitDigits = (value: number) => {
  let text: string;
  if (value < 0) {
    text = (0).toString();
  } else if (value >= 10) {
    text = (value - 10).toString();
  } else {
    text = value.toString();
  }

  return text;
};

const TensDigits = (value: number) => {
  let text: string;
  if (value < 0) {
    text = (value + 100).toString();
  } else if (value >= 100) {
    text = (value - 100).toString();
  } else {
    text = value.toString();
  }

  return text;
};

const HundredsDigit = (value: number) => {
  let text: string;
  if (value < 0) {
    text = (value + 1).toString();
  } else if (value >= 10) {
    text = (value - 10).toString();
  } else {
    text = value.toString();
  }

  return text;
};

interface CrosswindDigitalSpeedReadoutProps {
  bus: ArincEventBus;
}

export class CrosswindDigitalSpeedReadout extends View<CrosswindDigitalSpeedReadoutProps> {
  private speed = Signal.of(0);
  //private readonly speed = Arinc429RegisterSubject.createEmpty();
  private cwDrRef = UI.ref<SVGGElement>();
  private readonly mda = Arinc429RegisterSubject.createEmpty();

  private baroMode = new Arinc429Word(0);

  private isNegativeSub = Signal.of('hidden');

  private showThousandsZeroSub = Signal.of(false);

  private unitDigitsSub = Signal.of(0);

  private unitsValue = Signal.of(0);

  private tensValue = Signal.of(0);

  private hundredsValue = Signal.of(0);

  private unitsPosition = Signal.of(0);

  private tensPosition = Signal.of(0);

  private hundredsPosition = Signal.of(0);

  private thousandsValue = Signal.of(0);

  private thousandsPosition = Signal.of(0);

  private tenThousandsValue = Signal.of(0);

  private tenThousandsPosition = Signal.of(0);

  private readonly color = Derived.from(([speed]) => (speed > -1 ? 'Green' : 'Green'), this.speed);
  private crosswindMode = false;
  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<HUDSimvars & PanelEventTopics & Arinc429Values & ClockTopics & HudElems>();

    sub
      .topic('cWndMode')
      .distinct()
      .listen((value) => {
        this.crosswindMode = value;
        value === true
          ? (this.cwDrRef.target.style.transform = `translate3d(0px, ${-XWIND_FULL_OFFSET + XWIND_TO_AIR_REF_OFFSET}px, 0px)`)
          : (this.cwDrRef.target.style.transform = `translate3d(0px, 0px, 0px)`);
      });

    this.speed.watch((speed) => {
      const isNegative = speed < 0;
      this.isNegativeSub.set(isNegative ? 'visible' : 'hidden');

      const absSpeed = Math.abs(Math.max(Math.min(speed, 50000), -1500));

      const unitDigits = absSpeed % 10;
      this.unitsValue.set(unitDigits);

      const TensValue = Math.floor((absSpeed / 10) % 10);
      this.tensValue.set(TensValue);
      let TensPosition = 0;
      if (unitDigits >= 9) {
        TensPosition = unitDigits / 2 - 4;
        this.tensPosition.set(TensPosition);
      } else {
        this.tensPosition.set(0);
      }

      const HundredsValue = Math.floor((absSpeed / 100) % 10);
      this.hundredsValue.set(HundredsValue);
      let HundredsPosition = 0;
      if (TensValue >= 9) {
        HundredsPosition = TensPosition;
        this.hundredsPosition.set(HundredsPosition);
      } else {
        this.hundredsPosition.set(0);
      }

      const ThousandsValue = Math.floor((absSpeed / 1000) % 10);
      this.thousandsValue.set(ThousandsValue);
      let ThousandsPosition = 0;
      if (HundredsValue >= 9) {
        ThousandsPosition = HundredsPosition;
        this.thousandsPosition.set(ThousandsPosition);
      } else {
        this.thousandsPosition.set(0);
      }

      const TenThousandsValue = Math.floor((absSpeed / 10000) % 10);
      this.tenThousandsValue.set(TenThousandsValue);
      let TenThousandsPosition = 0;
      if (ThousandsValue >= 9) {
        TenThousandsPosition = ThousandsPosition;
      }

      this.tenThousandsPosition.set(TenThousandsPosition);
      const showThousandsZero = TenThousandsValue !== 0;

      this.showThousandsZeroSub.set(showThousandsZero);
    });
    //sub.on('speed').handle(this.speed.setWord.bind(this.speed));
    sub.topic('speedAr').listen((speed) => {
      this.speed.set(speed.value);
    });
  }

  render(): ViewNode {
    return (
      //   change lvar when btns added to model.
      <g id="CrosswindSpeedReadoutGroup" ref={this.cwDrRef}>
        <path
          id="CrosswindSpeedReadoutOutline"
          class="NormalStroke ReadoutBackgroundFill"
          d="m5 33.5 h 65 v 35 h -65z"
        />
        <g id="cwDrum" transform="translate(0 -20)">
          <svg id="cwdrHundreds" x="-40" y="53" width="67.5" height="35" viewBox="0 0 67.5 35">
            <Drum
              type="hundreds"
              position={this.hundredsPosition}
              value={this.hundredsValue}
              color={this.color}
              getText={HundredsDigit}
              valueSpacing={1}
              distanceSpacing={35}
              displayRange={1}
              amount={10}
            />
          </svg>
          <svg id="cwdrTens" x="16.5" y="53" width="67.5" height="35" viewBox="0 0 67.5 35">
            <Drum
              type="tens"
              position={this.tensPosition}
              value={this.tensValue}
              color={this.color}
              getText={TensDigits}
              valueSpacing={1}
              distanceSpacing={35}
              displayRange={1}
              amount={10}
            />
          </svg>
          <svg id="cwdrUnit" x="35" y="53" width="67.5" height="35" viewBox="0 0 67.5 35">
            <Drum
              type="unit"
              position={this.unitsPosition}
              value={this.unitsValue}
              color={this.color}
              getText={UnitDigits}
              valueSpacing={1}
              distanceSpacing={35}
              displayRange={1}
              amount={10}
            />
          </svg>
        </g>

        {/* <path id="cwTape"
            class="NormalStroke  Green"
            d="m128.75 76.337  v -6 "
          />
          <path id="cwTape"
            class="NormalStroke  Green"
            d="m128.75 85.337  v 6 "
          />
          <path id="cwTape"
            class="NormalStroke  Green"
            d="m117.75 91.337  h 22 "
          />
          <path id="cwTape"
            class="NormalStroke  Green"
            d="m117.75 70.337  h 22 "
          /> */}
      </g>
    );
  }
}

interface DrumProperties {
  type: string;
  displayRange: number;
  amount: number;
  valueSpacing: number;
  distanceSpacing: number;
  position: Readable<number>;
  value: Readable<number>;
  color: Readable<string>;
  getText: any;
  showZero?: Readable<boolean>;
}
class Drum extends View<DrumProperties> {
  private digitRefElements: Ref<SVGTextElement>[] = [];

  private buildElements(amount: number) {
    const highestValue =
      Math.round((this.value + this.props.displayRange) / this.props.valueSpacing) * this.props.valueSpacing;

    const graduationElements: SVGTextElement[] = [];

    for (let i = 0; i < amount; i++) {
      let elementVal = highestValue - i * this.props.valueSpacing;
      if (!this.showZero && elementVal === 0) {
        elementVal = NaN;
      }

      const digitRef = UI.ref<SVGTextElement>();

      if (this.props.type === 'hundreds') {
        graduationElements.push(
          <text
            ref={digitRef}
            style="transform: rotate3d(0px, 0px, 0px)"
            class={`FontLarge MiddleAlign ${this.color}`}
            x="58.155"
            y="30.5"
          />,
        );
      } else if (this.props.type === 'thousands') {
        graduationElements.push(
          <text
            ref={digitRef}
            style="transform: rotate3d(0px, 0px, 0px)"
            class={`FontLarge MiddleAlign ${this.color}`}
            x="7.18"
            y="7.1"
          />,
        );
      } else if (this.props.type === 'ten-thousands') {
        graduationElements.push(
          <text
            ref={digitRef}
            style="transform: rotate3d(0px, 0px, 0px)"
            class={`FontLarge MiddleAlign ${this.color}`}
            x="2.498"
            y="7.1"
          />,
        );
      } else if (this.props.type === 'tens') {
        graduationElements.push(
          <text
            ref={digitRef}
            style="transform: rotate3d(0px, 0px, 0px)"
            class={`FontLarge MiddleAlign ${this.color}`}
            x="20"
            y="30.5"
          />,
        );
      } else if (this.props.type === 'unit') {
        graduationElements.push(
          <text
            ref={digitRef}
            style="transform: rotate3d(0px, 0px, 0px)"
            class={`FontLarge MiddleAlign ${this.color}`}
            x="20"
            y="30.5"
          />,
        );
      }

      this.digitRefElements.push(digitRef);
    }

    return graduationElements;
  }

  private getOffset(position: number) {
    //// smooth drum
    //this.gRef.instance.style.transform = `translate3d(0px, ${(position * this.props.distanceSpacing) / this.props.valueSpacing}px, 0px)`;

    //// snap on digit
    this.gRef.target.style.transform = `translate3d(0px, ${
      (position * this.props.distanceSpacing) / this.props.valueSpacing -
      (((position * this.props.distanceSpacing) / this.props.valueSpacing) % this.props.distanceSpacing)
    }px, 0px)`;
  }

  private updateValue() {
    let highestPosition =
      Math.round((this.position + this.props.displayRange) / this.props.valueSpacing) * this.props.valueSpacing;
    if (highestPosition > this.position + this.props.displayRange) {
      highestPosition -= this.props.valueSpacing;
    }

    let highestValue =
      Math.round((this.value + this.props.displayRange) / this.props.valueSpacing) * this.props.valueSpacing;
    if (highestValue > this.value + this.props.displayRange) {
      highestValue -= this.props.valueSpacing;
    }

    for (let i = 0; i < this.props.amount; i++) {
      let elementVal = highestValue - i * this.props.valueSpacing;
      const elementPosition = highestPosition - i * this.props.valueSpacing;
      const offset = (-elementPosition * this.props.distanceSpacing) / this.props.valueSpacing;
      if (!this.showZero && elementVal === 0) {
        elementVal = NaN;
      }

      const text = this.props.getText(elementVal);

      this.digitRefElements[i].target.setAttribute('transform', `translate(0 ${offset})`);
      if (this.digitRefElements[i].target.textContent !== text) {
        this.digitRefElements[i].target.textContent = text;
      }
      this.digitRefElements[i].target.classList.replace('Green', this.color);
      this.digitRefElements[i].target.classList.replace('Amber', this.color);
    }
  }

  private position = 0;

  private value = 0;

  private color = 'Green';

  private showZero = true;

  private gRef = UI.ref<SVGGElement>();

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.props.position.watch((p) => {
      this.position = p;
      this.getOffset(p);
    }, true);
    this.props.value.watch((p) => {
      this.value = p;
      this.updateValue();
    }, true);
    this.props.color.watch((p) => {
      this.color = p;
      this.updateValue();
    });
    this.props.showZero?.watch((p) => {
      this.showZero = p;
      this.updateValue();
    }, true);
  }

  render(): ViewNode {
    return <g ref={this.gRef}>{this.buildElements(this.props.amount)}</g>;
  }
}
