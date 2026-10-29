import {
  ClockTopics,
  ViewProps,
  View,
  Bus,
  UI,
  Signal,
  Readable,
  ViewNode,
} from 'xplane-js/ui';
import { Arinc429Word, ArincEventBus } from '@flybywiresim/fbw-sdk';
import { Arinc429Values } from './shared/ArincValueProvider';
import { PFDSimvars } from './shared/PFDSimvarPublisher';
import { LagFilter } from './PFDUtils';
import { PanelInstrument } from 'xplane-js/data';

interface VerticalSpeedIndicatorProps {
  bus: ArincEventBus;
  instrument: PanelInstrument;
  filteredRadioAltitude: Readable<number>;
}

interface TcasState {
  tcasState: number;
  isTcasCorrective: boolean;
  tcasRedZoneL: number;
  tcasRedZoneH: number;
  tcasGreenZoneL: number;
  tcasGreenZoneH: number;
}

export class VerticalSpeedIndicator extends View<VerticalSpeedIndicatorProps> {
  private yOffsetSub = Signal.of(0);

  private needleColour = Signal.of('Green');

  private radioAlt = new Arinc429Word(0);

  private vsFailed = UI.ref<SVGGElement>();

  private vsNormal = UI.ref<SVGGElement>();

  private lagFilter = new LagFilter(2);

  private needsUpdate = false;

  private vspeedTcas = UI.ref<VSpeedTcas>();

  private filteredRadioAltitude = 0;

  private tcasState: TcasState = {
    tcasState: 0,
    isTcasCorrective: false,
    tcasRedZoneL: 0,
    tcasRedZoneH: 0,
    tcasGreenZoneL: 0,
    tcasGreenZoneH: 0,
  };

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.getArincSubscriber<PFDSimvars & Arinc429Values & ClockTopics>();

    sub
      .on('tcasState')
      .distinct()
      .listen((s) => {
        this.tcasState.tcasState = s;
        this.needsUpdate = true;
      });

    sub
      .on('tcasCorrective')
      .distinct()
      .listen((s) => {
        this.tcasState.isTcasCorrective = s;
        this.needsUpdate = true;
      });
    sub
      .on('tcasRedZoneL')
      .distinct()
      .listen((s) => {
        this.tcasState.tcasRedZoneL = s;
        this.needsUpdate = true;
      });
    sub
      .on('tcasRedZoneH')
      .distinct()
      .listen((s) => {
        this.tcasState.tcasRedZoneH = s;
        this.needsUpdate = true;
      });
    sub
      .on('tcasGreenZoneL')
      .distinct()
      .listen((s) => {
        this.tcasState.tcasGreenZoneL = s;
        this.needsUpdate = true;
      });
    sub
      .on('tcasGreenZoneH')
      .distinct()
      .listen((s) => {
        this.tcasState.tcasGreenZoneH = s;
        this.needsUpdate = true;
      });

    sub.on('realTime').listen((_r) => {
      if (this.needsUpdate) {
        if (this.tcasState.tcasState === 2) {
          this.needleColour.set('White');
        }
        this.vspeedTcas.target.update(this.tcasState);
      }
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
          this.vsNormal.target.style.visibility = 'visible';
        }

        const radioAltitudeValid = !this.radioAlt.isNoComputedData() && !this.radioAlt.isFailureWarning();
        if (this.tcasState.tcasState !== 2) {
          if (
            absVSpeed >= 6000 ||
            (vs.value <= -2000 &&
              radioAltitudeValid &&
              this.filteredRadioAltitude <= 2500 &&
              this.filteredRadioAltitude >= 1000) ||
            (vs.value <= -1200 && radioAltitudeValid && this.filteredRadioAltitude <= 1000)
          ) {
            this.needleColour.set('Amber');
          } else {
            this.needleColour.set('Green');
          }
        }

        const sign = Math.sign(filteredVS);

        if (absVSpeed < 1000) {
          this.yOffsetSub.set((filteredVS / 1000) * -27.22);
        } else if (absVSpeed < 2000) {
          this.yOffsetSub.set(((filteredVS - sign * 1000) / 1000) * -10.1 - sign * 27.22);
        } else if (absVSpeed < 6000) {
          this.yOffsetSub.set(((filteredVS - sign * 2000) / 4000) * -10.1 - sign * 37.32);
        } else {
          this.yOffsetSub.set(sign * -47.37);
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
      <g>
        <path class="TapeBackground" d="m151.84 131.72 4.1301-15.623v-70.556l-4.1301-15.623h-5.5404v101.8z" />

        <g id="VSpeedFailText" ref={this.vsFailed}>
          <text class="Blink9Seconds FontLargest Red EndAlign" x="153.13206" y="77.501472">
            V
          </text>
          <text class="Blink9Seconds FontLargest Red EndAlign" x="153.13406" y="83.211388">
            /
          </text>
          <text class="Blink9Seconds FontLargest Red EndAlign" x="152.99374" y="88.870819">
            S
          </text>
        </g>

        <VSpeedTcas ref={this.vspeedTcas} bus={this.props.bus} />

        <g id="VerticalSpeedGroup" ref={this.vsNormal}>
          <g class="Fill White">
            <path d="m149.92 54.339v-1.4615h1.9151v1.4615z" />
            <path d="m149.92 44.26v-1.4615h1.9151v1.4615z" />
            <path d="m149.92 34.054v-1.2095h1.9151v1.2095z" />
            <path d="m151.84 107.31h-1.9151v1.4615h1.9151z" />
            <path d="m151.84 117.39h-1.9151v1.4615h1.9151z" />
            <path d="m151.84 127.59h-1.9151v1.2095h1.9151z" />
          </g>
          <g class="SmallStroke  White">
            <path d="m149.92 67.216h1.7135h0" />
            <path d="m151.84 48.569h-1.9151h0" />
            <path d="m151.84 38.489h-1.9151h0" />
            <path d="m149.92 94.43h1.7135h0" />
            <path d="m151.84 113.08h-1.9151h0" />
            <path d="m151.84 123.16h-1.9151h0" />
          </g>
          <g class="FontSmallest MiddleAlign Fill White">
            <text x="148.47067" y="109.72845">
              1
            </text>
            <text x="148.24495" y="119.8801">
              2
            </text>
            <text x="148.27068" y="129.90607">
              6
            </text>
            <text x="148.49667" y="55.316456">
              1
            </text>
            <text x="148.26495" y="45.356102">
              2
            </text>
            <text x="148.21367" y="35.195072">
              6
            </text>
          </g>
          <path class="Fill Yellow" d="m145.79 80.067h6.0476v1.5119h-6.0476z" />
          <VSpeedNeedle yOffset={this.yOffsetSub} needleColour={this.needleColour} />

          <VSpeedText
            bus={this.props.bus}
            yOffset={this.yOffsetSub}
            textColour={this.needleColour.derive((c) => (c === 'White' ? 'Green' : c))}
          />
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

    const dxFull = 12;
    const dxBorder = 5;
    const centerX = 162.74;
    const centerY = 80.822;

    this.props.yOffset.watch((yOffset) => {
      const path = `m${centerX - dxBorder} ${centerY + (dxBorder / dxFull) * yOffset} l ${dxBorder - dxFull} ${(1 - dxBorder / dxFull) * yOffset}`;

      this.outLineRef.target.setAttribute('d', path);
      this.indicatorRef.target.setAttribute('d', path);
    });

    this.props.needleColour.watch((colour) => {
      this.indicatorRef.target.setAttribute('class', `HugeStroke ${colour}`);
    }, true);
  }

  render(): ViewNode | null {
    return (
      <>
        <path ref={this.outLineRef} class="HugeOutline" />
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

      const textOffset = this.props.yOffset.get() - sign * 2.4;

      const text = (Math.round(absVSpeed / 100) < 10 ? '0' : '') + Math.round(absVSpeed / 100).toString();
      this.vsTextRef.target.textContent = text;
      this.groupRef.target.setAttribute('transform', `translate(0 ${textOffset})`);
    });

    this.props.textColour.watch((colour) => {
      const className = `FontSmallest MiddleAlign ${colour}`;
      this.vsTextRef.target.setAttribute('class', className);
    }, true);
  }

  render(): ViewNode {
    return (
      <g ref={this.groupRef} id="VSpeedTextGroup">
        <path class="BackgroundFill" d="m158.4 83.011h-7.0514v-4.3989h7.0514z" />
        <text ref={this.vsTextRef} id="VSpeedText" x="155.14055" y="82.554756" />
      </g>
    );
  }
}

interface VSpeedTcasProps extends ViewProps {
  bus: Bus;
}
class VSpeedTcas extends View<VSpeedTcasProps> {
  private tcasGroup = UI.ref<SVGGElement>();

  private nonCorrective = UI.ref<SVGGElement>();

  private background = UI.ref<SVGRectElement>();

  private redZoneElement = UI.ref<VSpeedTcasZone>();

  private greenZoneElement = UI.ref<VSpeedTcasZone>();

  private redZone = Signal.of(-1);

  private redZoneHigh = Signal.of(-1);

  private greenZone = Signal.of(-1);

  private greenZoneHigh = Signal.of(-1);

  private extended = Signal.of(false);

  private isCorrective = Signal.of(false);

  public update(state: TcasState) {
    if (state.tcasState !== 2) {
      this.tcasGroup.target.classList.add('HiddenElement');
      this.nonCorrective.target.classList.add('HiddenElement');
    } else if (state.isTcasCorrective) {
      this.tcasGroup.target.classList.remove('HiddenElement');
      this.background.target.classList.remove('HiddenElement');
      this.redZone.set(state.tcasRedZoneL);
      this.redZoneHigh.set(state.tcasRedZoneH);
      this.greenZone.set(state.tcasGreenZoneL);
      this.greenZoneHigh.set(state.tcasGreenZoneH);
      this.nonCorrective.target.classList.add('HiddenElement');
    } else {
      this.background.target.classList.add('HiddenElement');
      this.nonCorrective.target.classList.add('HiddenElement');

      this.isCorrective.set(false);
      this.extended.set(false);
      this.redZone.set(state.tcasRedZoneL);
      this.redZoneHigh.set(state.tcasRedZoneH);
    }
  }

  render(): ViewNode {
    return (
      <>
        <g id="VerticalSpeedTCASGroup" ref={this.tcasGroup}>
          <rect ref={this.background} class="TapeBackground" height="101.8" width="5.5404" y="29.92" x="151.84" />
          <VSpeedTcasZone
            ref={this.redZoneElement}
            zoneBoundLow={this.redZone}
            zoneBoundHigh={this.redZoneHigh}
            zoneClass="Fill Red"
            isCorrective={this.isCorrective}
            extended={Signal.of(false)}
          />
          <VSpeedTcasZone
            ref={this.greenZoneElement}
            zoneBoundLow={this.greenZone}
            zoneBoundHigh={this.greenZoneHigh}
            zoneClass="Fill Green"
            extended={this.extended}
            isCorrective={this.isCorrective}
          />
        </g>
        <g id="VerticalSpeedTCASGroupNonCorrective" ref={this.nonCorrective}>
          <VSpeedTcasZone
            zoneBoundLow={this.redZone}
            zoneBoundHigh={this.redZoneHigh}
            zoneClass="Fill Red"
            extended={Signal.of(false)}
            isCorrective={Signal.of(false)}
          />
        </g>
      </>
    );
  }
}

interface VSpeedTcasZoneProps extends ViewProps {
  zoneBoundLow: Readable<number>;
  zoneBoundHigh: Readable<number>;
  zoneClass: string;
  isCorrective: Readable<boolean>;
  extended: Readable<boolean>;
}
class VSpeedTcasZone extends View<VSpeedTcasZoneProps> {
  private zoneUpper = 0;

  private zoneLower = 0;

  private extended = false;

  private isCorrective = false;

  private path = UI.ref<SVGPathElement>();

  private getYoffset = (VSpeed: number) => {
    const absVSpeed = Math.abs(VSpeed);
    const sign = Math.sign(VSpeed);

    if (absVSpeed < 1000) {
      return (VSpeed / 1000) * -27.22;
    }
    if (absVSpeed < 2000) {
      return ((VSpeed - sign * 1000) / 1000) * -10.1 - sign * 27.22;
    }
    if (absVSpeed < 6000) {
      return ((VSpeed - sign * 2000) / 4000) * -10.1 - sign * 37.32;
    }
    return sign * -47.37;
  };

  private drawTcasZone() {
    if (this.zoneLower !== -1 && this.zoneUpper !== -1) {
      let y1;
      let y2;
      let y3;
      let y4;

      if (this.zoneLower >= 6000) {
        y1 = 29.92;
      } else if (this.zoneLower <= -6000) {
        y1 = 131.72;
      } else {
        y1 = 80.822 + this.getYoffset(this.zoneLower);
      }

      if (this.zoneUpper >= 6000) {
        y2 = 29.92;
      } else if (this.zoneUpper <= -6000) {
        y2 = 131.72;
      } else {
        y2 = 80.822 + this.getYoffset(this.zoneUpper);
      }

      if (
        (Math.abs(this.zoneUpper) > 1750 && Math.abs(this.zoneUpper) > Math.abs(this.zoneLower)) ||
        (this.isCorrective && this.props.zoneClass === 'Fill Red')
      ) {
        y3 = y2;
      } else {
        // y3 = 80.822 + getYoffset(zoneBounds[1] / 2);
        y3 = 80.822;
      }

      if (
        (Math.abs(this.zoneLower) > 1750 && Math.abs(this.zoneLower) > Math.abs(this.zoneUpper)) ||
        (this.isCorrective && this.props.zoneClass === 'Fill Red')
      ) {
        y4 = y1;
      } else {
        // y4 = 80.822 + getYoffset(zoneBounds[0] / 2);
        y4 = 80.822;
      }

      const x1 = 151.84;
      const x2 = this.extended ? 162.74 : 157.3804;

      this.path.target.setAttribute('d', `m${x1},${y1} L${x1},${y2} L${x2},${y3} L${x2},${y4} L${x1},${y1}z`);
    }
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.props.zoneBoundLow.watch((z) => {
      this.zoneLower = z;
      this.drawTcasZone();
    });

    this.props.zoneBoundHigh.watch((z) => {
      this.zoneUpper = z;
      this.drawTcasZone();
    });

    this.props.extended.watch((z) => {
      this.extended = z;
      this.drawTcasZone();
    });

    this.props.isCorrective.watch((z) => {
      this.isCorrective = z;
      this.drawTcasZone();
    });
  }

  render(): ViewNode {
    return <path ref={this.path} class={this.props.zoneClass} />;
  }
}
