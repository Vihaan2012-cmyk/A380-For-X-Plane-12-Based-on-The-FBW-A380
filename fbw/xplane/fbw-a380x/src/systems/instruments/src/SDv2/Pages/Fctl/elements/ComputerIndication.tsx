import {
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  Signal,
  Readable,
  Logic,
} from 'xplane-js/ui';
import { SDSimvars } from '../../../SDSimvarPublisher';

interface FctlComputerShapeProps {
  x: number;
  y: number;
  num: 1 | 2 | 3;
  infoAvailable: Readable<boolean>;
  computerFailed: Readable<boolean>;
}

class FctlComputerShape extends View<FctlComputerShapeProps> {
  private readonly computerNumberGreen = Derived.from(
    ([infoAvailable, computerFailed]) => !computerFailed && infoAvailable,
    this.props.infoAvailable,
    this.props.computerFailed,
  );

  private readonly indicationBorderGrey = Derived.from(
    ([infoAvailable, computerFailed]) => !computerFailed || !infoAvailable,
    this.props.infoAvailable,
    this.props.computerFailed,
  );

  render() {
    let path: string;
    let textX: number;
    let textY: number;
    if (this.props.num === 1) {
      path = 'M0 0 l98,0 l0,-35 l-21,0';
      textX = 81;
      textY = -8;
    } else if (this.props.num === 2) {
      path = 'M0 0 l94,0 l0,-35 l-23,0';
      textX = 75;
      textY = -10;
    } else {
      path = 'M0 0 l93,0 l0,-35 l-23,0';
      textX = 74;
      textY = -8;
    }

    return (
      <g transform={`translate(${this.props.x} ${this.props.y})`}>
        <path
          class={{
            SW3: true,
            NoFill: true,
            LineRound: true,
            Grey: this.indicationBorderGrey,
            Amber: this.indicationBorderGrey.derive(Logic.not()),
          }}
          d={path}
        />
        <text
          x={textX}
          y={textY}
          class={{
            F26: true,
            Green: this.computerNumberGreen,
            Amber: this.computerNumberGreen.derive(Logic.not()),
          }}
        >
          {this.props.infoAvailable.derive((infoAvailable) => (infoAvailable ? this.props.num : 'X'))}
        </text>
      </g>
    );
  }
}

interface PrimSecFlapsSlatsProps {
  x: number;
  y: number;
  bus: Bus;
}

export class Prims extends View<PrimSecFlapsSlatsProps> {
  private infoAvailable = Signal.of(true);

  private readonly prim1Healthy = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic('prim1Healthy'),
    false,
  );
  private readonly prim2Healthy = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic('prim2Healthy'),
    false,
  );
  private readonly prim3Healthy = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic('prim3Healthy'),
    false,
  );

  render() {
    return (
      <g id="prim-computers" transform={`translate(${this.props.x} ${this.props.y})`}>
        <text class="F22 White MiddleAlign LS1" x={45} y={85}>
          PRIM
        </text>
        <FctlComputerShape
          x={8}
          y={100}
          num={1}
          infoAvailable={this.infoAvailable}
          computerFailed={this.prim1Healthy.derive(Logic.not())}
        />
        <FctlComputerShape
          x={36}
          y={112}
          num={2}
          infoAvailable={this.infoAvailable}
          computerFailed={this.prim2Healthy.derive(Logic.not())}
        />
        <FctlComputerShape
          x={62}
          y={124}
          num={3}
          infoAvailable={this.infoAvailable}
          computerFailed={this.prim3Healthy.derive(Logic.not())}
        />
      </g>
    );
  }
}

export class Secs extends View<PrimSecFlapsSlatsProps> {
  private infoAvailable = Signal.of(true);

  private readonly sec1Healthy = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic('sec1Healthy'),
    false,
  );
  private readonly sec2Healthy = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic('sec2Healthy'),
    false,
  );
  private readonly sec3Healthy = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic('sec3Healthy'),
    false,
  );

  render() {
    return (
      <g id="sec-computers" transform={`translate(${this.props.x} ${this.props.y})`}>
        <text class="F22 White MiddleAlign LS1" x={48} y={84}>
          SEC
        </text>
        <FctlComputerShape
          x={8}
          y={100}
          num={1}
          infoAvailable={this.infoAvailable}
          computerFailed={this.sec1Healthy.derive(Logic.not())}
        />
        <FctlComputerShape
          x={36}
          y={112}
          num={2}
          infoAvailable={this.infoAvailable}
          computerFailed={this.sec2Healthy.derive(Logic.not())}
        />
        <FctlComputerShape
          x={62}
          y={124}
          num={3}
          infoAvailable={this.infoAvailable}
          computerFailed={this.sec3Healthy.derive(Logic.not())}
        />
      </g>
    );
  }
}

export class Slats extends View<PrimSecFlapsSlatsProps> {
  private infoAvailable = Signal.of(true);
  private computerFailed = Signal.of(false);

  render() {
    return (
      <g id="slat-computers" transform={`translate(${this.props.x} ${this.props.y})`}>
        <text class="F22 White MiddleAlign LS1" x={46} y={85}>
          SLATS
        </text>
        <FctlComputerShape
          x={8}
          y={100}
          num={1}
          infoAvailable={this.infoAvailable}
          computerFailed={this.computerFailed}
        />
        <FctlComputerShape
          x={36}
          y={112}
          num={2}
          infoAvailable={this.infoAvailable}
          computerFailed={this.computerFailed}
        />
      </g>
    );
  }
}

export class Flaps extends View<PrimSecFlapsSlatsProps> {
  private infoAvailable = Signal.of(true);
  private computerFailed = Signal.of(false);

  render() {
    return (
      <g id="flap-computers" transform={`translate(${this.props.x} ${this.props.y})`}>
        <text class="F22 White MiddleAlign LS1" x={46} y={84}>
          FLAPS
        </text>
        <FctlComputerShape
          x={8}
          y={100}
          num={1}
          infoAvailable={this.infoAvailable}
          computerFailed={this.computerFailed}
        />
        <FctlComputerShape
          x={36}
          y={112}
          num={2}
          infoAvailable={this.infoAvailable}
          computerFailed={this.computerFailed}
        />
      </g>
    );
  }
}
