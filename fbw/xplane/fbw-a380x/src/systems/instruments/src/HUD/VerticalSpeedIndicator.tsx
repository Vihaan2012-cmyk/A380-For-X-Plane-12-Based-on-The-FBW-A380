import {
  ClockTopics,
  View,
  Bus,
  UI,
  Signal,
  Readable,
  ViewNode,
} from 'xplane-js/ui';
import { Arinc429Word, ArincEventBus } from '@flybywiresim/fbw-sdk';
import { Arinc429Values } from './shared/ArincValueProvider';
import { HUDSimvars } from './shared/HUDSimvarPublisher';
import { LagFilter } from './HUDUtils';
import { HudElems } from './HUDUtils';
import { PanelInstrument } from 'xplane-js/data';

interface VerticalSpeedIndicatorProps {
  bus: ArincEventBus;
  instrument: PanelInstrument;
  filteredRadioAltitude: Readable<number>;
}

export class VerticalSpeedIndicator extends View<VerticalSpeedIndicatorProps> {
  private crosswindMode = false;
  private declutterMode = 0;
  private VS = '';
  private VSRef = UI.ref<SVGGElement>();
  private yOffsetSub = Signal.of(0);

  private needleColour = Signal.of('Green');

  private radioAlt = new Arinc429Word(0);

  private vsFailed = UI.ref<SVGGElement>();

  private vsNormal = UI.ref<SVGGElement>();

  private lagFilter = new LagFilter(2);

  private needsUpdate = false;

  private filteredRadioAltitude = 0;

  private handlePos() {
    if (this.crosswindMode) {
      // transform="translate(475 135)
      this.VSRef.target.style.transform = 'translate3d(485px, -35px, 0px)';
    } else {
      this.VSRef.target.style.transform = 'translate3d(485px, 155px, 0px)';
    }
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.getArincSubscriber<HUDSimvars & Arinc429Values & ClockTopics & HudElems>();

    sub
      .on('decMode')
      .distinct()
      .listen((value) => {
        this.declutterMode = value;
        this.handlePos();
      });

    sub
      .on('cWndMode')
      .distinct()
      .listen((value) => {
        this.crosswindMode = value;
        this.handlePos();
      });
    sub
      .on('VSI')
      .distinct()
      .listen((v) => {
        this.VS = v;
        this.VSRef.target.style.display = `${this.VS}`;
        this.handlePos();
      });
    sub
      .on('vs')
      .withArinc429Precision(2)
      .listen((vs) => {
        const filteredVS = this.lagFilter.step(vs.value, this.props.instrument.frameDelta / 1000);

        const absVSpeed = Math.abs(filteredVS);
        if (!vs.isNormalOperation()) {
          this.vsFailed.target.style.visibility = 'visible';
          this.vsNormal.target.style.visibility = 'hidden';
        } else {
          this.vsFailed.target.style.visibility = 'hidden';
          absVSpeed > 50
            ? (this.vsNormal.target.style.visibility = 'visible')
            : (this.vsNormal.target.style.visibility = 'hidden');
        }

        const sign = Math.sign(filteredVS);
        const multiplier = this.crosswindMode ? 0.5 : 1;
        if (absVSpeed < 1000) {
          this.yOffsetSub.set((filteredVS / 1000) * -136.1 * multiplier);
        } else if (absVSpeed < 2000) {
          this.yOffsetSub.set(((filteredVS - sign * 1000) / 1000) * -50.5 - sign * 136.1 * multiplier);
        } else if (absVSpeed < 6000) {
          this.yOffsetSub.set(((filteredVS - sign * 2000) / 4000) * -50.5 - sign * 186.6 * multiplier);
        } else {
          this.yOffsetSub.set(sign * -237 * multiplier);
        }
      });

    sub.on('chosenRa').listen((ra) => {
      this.radioAlt = ra;
    });

    this.props.filteredRadioAltitude.watch((filteredRadioAltitude) => {
      this.filteredRadioAltitude = filteredRadioAltitude;
    });
  }

  render(): ViewNode {
    return (
      <g id="VerticalSpeedIndicator" ref={this.VSRef}>
        <g id="VSpeedFailText" ref={this.vsFailed}>
          <text class="Blink9Seconds FontLargest Green EndAlign" x="686.7972891000001" y="347.59410192">
            V
          </text>
          <text class="Blink9Seconds FontLargest Green EndAlign" x="686.8062591" y="373.20307518">
            /
          </text>
          <text class="Blink9Seconds FontLargest Green EndAlign" x="686.1769239" y="398.585623215">
            S
          </text>
        </g>

        <g id="VerticalSpeedGroup" ref={this.vsNormal}>
          <path class="Fill Green" d="m 665 361 h 20 v 4 h -20 z" />
          <VSpeedNeedle yOffset={this.yOffsetSub} needleColour={this.needleColour} />

          <VSpeedText bus={this.props.bus} yOffset={this.yOffsetSub} textColour={this.needleColour} />
        </g>
      </g>
    );
  }
}

class VSpeedNeedle extends View<{ yOffset: Readable<number>; needleColour: Readable<string> }> {
  private outLineRef = UI.ref<SVGPathElement>();

  private indicatorRef = UI.ref<SVGPathElement>();

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const dxFull = 50;
    const dxBorder = 13;
    const centerX = 733;
    const centerY = 363;

    this.props.yOffset.watch((yOffset) => {
      const path = `m${centerX - dxBorder} ${centerY + (dxBorder / dxFull) * yOffset} l ${dxBorder - dxFull} ${(1 - dxBorder / dxFull) * yOffset}`;

      this.outLineRef.target.setAttribute('d', path);
      this.indicatorRef.target.setAttribute('d', path);
    });

    this.props.needleColour.watch((colour) => {
      this.indicatorRef.target.setAttribute('class', `ThickStroke ${colour}`);
    }, true);
  }

  render(): ViewNode | null {
    return (
      <>
        <path ref={this.outLineRef} class="NormalOutline" />
        <path ref={this.indicatorRef} id="VSpeedIndicator" />
      </>
    );
  }
}

class VSpeedText extends View<{
  bus: Bus;
  yOffset: Readable<number>;
  textColour: Readable<string>;
}> {
  private vsTextRef = UI.ref<SVGTextElement>();

  private groupRef = UI.ref<SVGGElement>();

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<Arinc429Values>();

    sub.topic('vs').listen((vs) => {
      const absVSpeed = Math.abs(vs.value);

      if (absVSpeed < 200) {
        this.groupRef.target.setAttribute('visibility', 'hidden');
        return;
      }
      this.groupRef.target.setAttribute('visibility', 'visible');

      const sign = Math.sign(vs.value);

      const textOffset = this.props.yOffset.get() - sign * 12;
      const textOffsetX = sign > 0 ? 0 : -5;

      const minusSign = sign > 0 ? '' : '-';

      const text = minusSign + (Math.round(absVSpeed / 100) < 10 ? '0' : '') + Math.round(absVSpeed / 100).toString();
      this.vsTextRef.target.textContent = text;
      this.groupRef.target.setAttribute('transform', `translate(${textOffsetX} ${textOffset})`);
    });

    this.props.textColour.watch((colour) => {
      const className = `FontSmallest MiddleAlign ${colour}`;
      this.vsTextRef.target.setAttribute('class', className);
    }, true);
  }

  render(): ViewNode {
    return (
      <g ref={this.groupRef} id="VSpeedTextGroup">
        <text ref={this.vsTextRef} id="VSpeedText" x="695.8029" y="370.0125" />
      </g>
    );
  }
}
