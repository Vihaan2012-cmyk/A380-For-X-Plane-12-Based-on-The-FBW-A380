// @ts-strict-ignore
import {
  View,
  UI,
  Readable,
  Signal,
  ViewNode,
  Derived,
  Readables,
} from 'xplane-js/ui';

import './gauges.scss';

export const polarToCartesian = (centerX: number, centerY: number, radius: number, angleInDegrees: number) => {
  const angleInRadians = ((angleInDegrees - 90) * Math.PI) / 180.0;

  return {
    x: centerX + radius * Math.cos(angleInRadians),
    y: centerY + radius * Math.sin(angleInRadians),
  };
};

/**
 * Draws an arc between startAngle and endAngle. This can start and finish anywhere on a circle
 * Note all arcs are drawn in a clockwise fashion
 *
 * @param x             x coordinate of arc centre
 * @param y             y coordinate of arc centre
 * @param radius        radius of arc
 * @param startAngle    value between 0 and 360 degrees where arc starts
 * @param endAngle      value between 0 and 360 degrees where arc finishes
 * @param sweep         value between 0 and 1 for sweep flag
 * @param largeArc      value between 0 and 1 for large arc flag
 */
export function describeArc(
  x: number,
  y: number,
  radius: number,
  startAngle: number,
  endAngle: number,
  sweep: number = 0,
  largeArc: number = 2,
) {
  const start = polarToCartesian(x, y, radius, endAngle);
  const end = polarToCartesian(x, y, radius, startAngle);

  const arcSize = startAngle > endAngle ? 360 - endAngle + startAngle : endAngle - startAngle;
  if (largeArc === 2) {
    largeArc = arcSize <= 180 ? 0 : 1;
  }

  return ['M', start.x, start.y, 'A', radius, radius, 0, largeArc, sweep, end.x, end.y].join(' ');
}

export const splitDecimals = (value: number) => value.toFixed(1).split('.', 2);

export const valueRadianAngleConverter = (
  value: number,
  min: number,
  max: number,
  endAngle: number,
  startAngle: number,
  perpendicular: boolean = false,
) => {
  const valuePercentage = (value - min) / (max - min);
  const angle = perpendicular ? 0 : 90;
  const angleInDegress =
    startAngle > endAngle
      ? startAngle + valuePercentage * (360 - startAngle + endAngle) - angle
      : startAngle + valuePercentage * (endAngle - startAngle) - angle;
  const angleInRadians = angleInDegress * (Math.PI / 180.0);
  return {
    x: Math.cos(angleInRadians),
    y: Math.sin(angleInRadians),
    angle: angleInDegress,
  };
};

interface GaugeMarkerComponentProps {
  value: Readable<number>;
  x: number;
  y: number;
  min: number;
  max: number;
  radius: number;
  startAngle: number;
  endAngle: number;
  class: string | Readable<string>;
  textClass?: string;
  showValue?: boolean;
  indicator?: boolean;
  outer?: boolean;
  multiplierOuter?: number;
  multiplierInner?: number;
  textNudgeX?: number;
  textNudgeY?: number;
  halfIndicator?: boolean;
  bold?: boolean;
  roundLinecap?: boolean;
  reverse?: boolean;
  overrideText?: string;
}
export class GaugeMarkerComponent extends View<GaugeMarkerComponentProps> {
  private startX = Signal.of(0);

  private startY = Signal.of(0);

  private endX = Signal.of(0);

  private endY = Signal.of(0);

  private textX = Signal.of(0);

  private textY = Signal.of(0);

  private text = Signal.of('');

  private value: number = 0;

  constructor(props: GaugeMarkerComponentProps) {
    super(props);

    this.props.textClass ??= 'GaugeText';
    this.props.multiplierOuter ??= 1.15;
    this.props.multiplierInner ??= 0.85;
    this.props.showValue ??= false;
    this.props.textNudgeX ??= 0;
    this.props.textNudgeY ??= 0;

    this.value = this.props.value.get();
    this.update();
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.props.value.watch((v) => {
      this.value = v;
      this.update();
    });
  }

  update(): void {
    const dir = valueRadianAngleConverter(
      this.value,
      this.props.min,
      this.props.max,
      this.props.endAngle,
      this.props.startAngle,
      this.props.reverse ?? false,
    );

    let start = {
      x: this.props.x + dir.x * this.props.radius * this.props.multiplierInner,
      y: this.props.y + dir.y * this.props.radius * this.props.multiplierInner,
    };
    let end = {
      x: this.props.x + dir.x * this.props.radius,
      y: this.props.y + dir.y * this.props.radius,
    };

    if (this.props.outer) {
      start = {
        x: this.props.x + dir.x * this.props.radius,
        y: this.props.y + dir.y * this.props.radius,
      };
      end = {
        x: this.props.x + dir.x * this.props.radius * this.props.multiplierOuter,
        y: this.props.y + dir.y * this.props.radius * this.props.multiplierOuter,
      };
    }

    if (this.props.indicator) {
      // Need case for EGT and other gauges which do not originate from the centre of the arc
      // In this case use original start definition
      if (!this.props.halfIndicator) {
        start = { x: this.props.x, y: this.props.y };
      }

      end = {
        x: this.props.x + dir.x * this.props.radius * this.props.multiplierOuter,
        y: this.props.y + dir.y * this.props.radius * this.props.multiplierOuter,
      };
    }

    // Text
    const pos = {
      x: this.props.x + dir.x * this.props.radius * this.props.multiplierInner + this.props.textNudgeX,
      y: this.props.y + dir.y * this.props.radius * this.props.multiplierInner + this.props.textNudgeY,
    };

    if (Number.isNaN(start.x)) {
      console.log(dir);
      console.log(this);
    }

    this.startX.set(start.x);
    this.startY.set(start.y);
    this.endX.set(end.x);
    this.endY.set(end.y);
    this.textX.set(pos.x);
    this.textY.set(pos.y);
    this.text.set(
      !this.props.showValue ? '' : this.props.overrideText ? this.props.overrideText : Math.abs(this.value).toString(),
    );
  }

  render(): ViewNode {
    return (
      <>
        <line
          x1={this.startX}
          y1={this.startY}
          x2={this.endX}
          y2={this.endY}
          strokeWidth={this.props.bold ? 3 : 2}
          class={this.props.class}
          strokeLinecap={this.props.roundLinecap ? 'round' : 'square'}
        />
        <text
          x={this.textX}
          y={this.textY}
          class={this.props.textClass}
          alignmentBaseline="central"
          textAnchor="middle"
        >
          {this.text}
        </text>
      </>
    );
  }
}

interface GaugeMaxComponentProps {
  value: Readable<number>;
  x: number;
  y: number;
  min: number;
  max: number;
  radius: number;
  startAngle: number;
  endAngle: number;
  class: string;
}
export class GaugeMaxComponent extends View<GaugeMaxComponentProps> {
  private rectX = Signal.of(0);

  private rectY = Signal.of(0);

  private transform = Signal.of('');

  private value: number = 0;

  constructor(props: GaugeMaxComponentProps) {
    super(props);

    this.value = this.props.value.get();
    this.update();
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.props.value.watch((v) => {
      this.value = v;
      this.update();
    });
  }

  update(): void {
    const dir = valueRadianAngleConverter(
      this.value,
      this.props.min,
      this.props.max,
      this.props.endAngle,
      this.props.startAngle,
    );

    const xy = { x: this.props.x + dir.x * this.props.radius, y: this.props.y + dir.y * this.props.radius };

    this.rectX.set(xy.x);
    this.rectY.set(xy.y);
    this.transform.set(`rotate(${dir.angle} ${xy.x} ${xy.y})`);
  }

  render(): ViewNode {
    return (
      <rect x={this.rectX} y={this.rectY} width={6} height={4} class={this.props.class} transform={this.transform} />
    );
  }
}

interface ThrottlePositionDonutComponentProps {
  value: Readable<number>;
  x: number;
  y: number;
  min: number;
  max: number;
  radius: number;
  startAngle: number;
  endAngle: number;
  class: string;
  reverse?: boolean;
  outerMultiplier?: number;
  donutRadius?: number;
}
export class ThrottlePositionDonutComponent extends View<ThrottlePositionDonutComponentProps> {
  private circleX = Signal.of(0);

  private circleY = Signal.of(0);

  private value: number = 0;

  constructor(props: ThrottlePositionDonutComponentProps) {
    super(props);

    this.value = this.props.value.get();
    this.update();
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.props.value.watch((v) => {
      this.value = v;
      this.update();
    });
  }

  update(): void {
    const dir = valueRadianAngleConverter(
      this.value,
      this.props.min,
      this.props.max,
      this.props.endAngle,
      this.props.startAngle,
      this.props.reverse,
    );

    this.circleX.set(this.props.x + dir.x * this.props.radius * (this.props.outerMultiplier ?? 1.12));
    this.circleY.set(this.props.y + dir.y * this.props.radius * (this.props.outerMultiplier ?? 1.12));
  }

  render(): ViewNode {
    return <circle cx={this.circleX} cy={this.circleY} r={this.props.donutRadius ?? 4} class={this.props.class} />;
  }
}

interface GaugeComponentProps {
  x: number;
  y: number;
  radius: number;
  startAngle: number;
  endAngle: number;
  class: string;
  visible?: Readable<boolean>;
  sweep?: number;
  largeArc?: number;
}
export class GaugeComponent extends View<GaugeComponentProps> {
  private arc = Signal.of('');

  constructor(props: GaugeComponentProps) {
    super(props);

    this.updateArc();
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);
  }

  updateArc(): void {
    this.arc.set(
      describeArc(
        this.props.x,
        this.props.y,
        this.props.radius,
        this.props.startAngle,
        this.props.endAngle,
        this.props.sweep,
        this.props.largeArc,
      ),
    );
  }

  render(): ViewNode {
    return (
      <g class="GaugeComponent" visibility={this.props.visible?.derive((v) => (v ? 'inherit' : 'hidden')) ?? 'inherit'}>
        <path d={this.arc} class={this.props.class} />
        {this.props.children}
      </g>
    );
  }
}

interface GaugeMaxEGTComponentProps {
  value: Readable<number>;
  x: number;
  y: number;
  min: number;
  max: number;
  radius: number;
  startAngle: number;
  endAngle: number;
  class: string;
}
export class GaugeMaxEGTComponent extends View<GaugeMaxEGTComponentProps> {
  private angle = Signal.of(0);

  private value: number = 0;

  private path = Signal.of('');

  private pathTransform = Signal.of('');

  constructor(props: GaugeMaxEGTComponentProps) {
    super(props);
    this.value = this.props.value.get();
    this.update();
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.props.value.watch((v) => {
      this.value = v;
      this.update();
    });
  }

  update(): void {
    const dir = valueRadianAngleConverter(
      this.value,
      this.props.min,
      this.props.max,
      this.props.endAngle,
      this.props.startAngle,
    );
    const x = this.props.x + dir.x * this.props.radius;
    const y = this.props.y + dir.y * this.props.radius;

    this.angle.set(dir.angle);
    this.path.set(`M ${x - 6},${y} l 12,0 l 0,4 l -8,0 Z`);
    this.pathTransform.set(`rotate(${this.angle.get()} ${x} ${y})`);
  }

  render(): ViewNode {
    return <path d={this.path} class={this.props.class} transform={this.pathTransform} />;
  }
}

interface GaugeThrustComponentProps {
  x: number;
  y: number;
  min: number;
  max: number;
  radius: number;
  startAngle: number;
  endAngle: number;
  class: string;
  visible: Readable<boolean>;
  valueIdle: number | Readable<number>;
  valueMax: number | Readable<number>;
  reverse?: boolean;
}
export class GaugeThrustComponent extends View<GaugeThrustComponentProps> {
  constructor(props: GaugeThrustComponentProps) {
    super(props);
  }

  private readonly valueIdleSub = Readables.toReadable(this.props.valueIdle, true);

  private readonly valueMaxSub = Readables.toReadable(this.props.valueMax, true);

  private readonly thrustPath = Derived.from(
    ([valueIdle, valueMax]) => {
      const valueIdleDir = valueRadianAngleConverter(
        valueIdle,
        this.props.min,
        this.props.max,
        this.props.endAngle,
        this.props.startAngle,
      );
      const valueIdleEnd = {
        x: this.props.x + valueIdleDir.x * this.props.radius,
        y: this.props.y + valueIdleDir.y * this.props.radius,
      };
      const valueMaxDir = valueRadianAngleConverter(
        valueMax,
        this.props.min,
        this.props.max,
        this.props.endAngle,
        this.props.startAngle,
      );
      const valueMaxEnd = {
        x: this.props.x + valueMaxDir.x * this.props.radius,
        y: this.props.y + valueMaxDir.y * this.props.radius,
      };

      const diffAngle = valueMaxDir.angle - valueIdleDir.angle;

      return [
        `M ${this.props.x},${this.props.y} L ${valueIdleEnd.x.toFixed(2)},${valueIdleEnd.y.toFixed(2)}`,
        `A ${this.props.radius} ${this.props.radius} 0 ${Math.abs(diffAngle) < 180 || this.props.reverse ? '0' : '1'} 1 ${valueMaxEnd.x.toFixed(2)} ${valueMaxEnd.y.toFixed(2)}`,
        `M ${valueMaxEnd.x.toFixed(2)} ${valueMaxEnd.y.toFixed(2)} L ${this.props.x},${this.props.y}`,
      ].join(' ');
    },
    this.valueIdleSub,
    this.valueMaxSub,
  );

  render(): ViewNode {
    return (
      <g class="GaugeComponent">
        <g class={this.props.visible.derive((it) => (it ? 'Show' : 'Hide'))}>
          <path d={this.thrustPath} class={this.props.class} />
        </g>
      </g>
    );
  }
}

interface ThrustTransientComponentProps {
  x: number;
  y: number;
  min: number;
  max: number;
  radius: number;
  startAngle: number;
  endAngle: number;
  class: string;
  visible: Readable<boolean>;
  thrustActual: Readable<number>;
  thrustTarget: Readable<number>;
}
export class ThrustTransientComponent extends View<ThrustTransientComponentProps> {
  private thrustActual: number = 0;

  private thrustTarget: number = 0;

  private sweep1 = Signal.of('');
  private sweep2 = Signal.of('');
  private sweep3 = Signal.of('');
  private sweep4 = Signal.of('');

  private endPath = Signal.of('');

  constructor(props: ThrustTransientComponentProps) {
    super(props);

    this.update();
  }

  private sweepHorizontal(radius: number) {
    const valueIdleDir = valueRadianAngleConverter(
      this.thrustActual,
      this.props.min,
      this.props.max,
      this.props.endAngle,
      this.props.startAngle,
    );
    const valueIdleEnd = {
      x: this.props.x + valueIdleDir.x * radius,
      y: this.props.y + valueIdleDir.y * radius,
    };
    const valueMaxDir = valueRadianAngleConverter(
      this.thrustTarget,
      this.props.min,
      this.props.max,
      this.props.endAngle,
      this.props.startAngle,
    );
    const valueMaxEnd = {
      x: this.props.x + valueMaxDir.x * radius,
      y: this.props.y + valueMaxDir.y * radius,
    };

    const diffAngle = valueMaxDir.angle - valueIdleDir.angle;

    return [
      `M ${valueIdleEnd.x},${valueIdleEnd.y}`,
      `A ${radius} ${radius} 0 ${Math.abs(diffAngle) > 180 ? '1' : '0'} ${diffAngle < 0 ? '0' : '1'} ${valueMaxEnd.x} ${valueMaxEnd.y}`,
    ].join(' ');
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.props.thrustActual.watch((a) => {
      this.thrustActual = a;
      this.update();
    });

    this.props.thrustTarget.watch((a) => {
      this.thrustTarget = a;
      this.update();
    });
  }

  update(): void {
    const thrustEnd = valueRadianAngleConverter(
      this.thrustTarget,
      this.props.min,
      this.props.max,
      this.props.endAngle,
      this.props.startAngle,
    );
    this.endPath.set(
      `M ${this.props.x} ${this.props.y} L ${this.props.x + thrustEnd.x * 0.8 * this.props.radius} ${this.props.y + thrustEnd.y * 0.8 * this.props.radius}`,
    );

    this.sweep1.set(this.sweepHorizontal(0.8 * this.props.radius));
    this.sweep2.set(this.sweepHorizontal(0.65 * this.props.radius));
    this.sweep3.set(this.sweepHorizontal(0.5 * this.props.radius));
    this.sweep4.set(this.sweepHorizontal(0.35 * this.props.radius));
  }

  render(): ViewNode {
    return (
      <>
        <g class="GaugeComponent">
          <g class={this.props.visible.derive((it) => (it ? 'Show' : 'Hide'))}>
            <path d={this.sweep1} class={this.props.class} />
            <path d={this.sweep2} class={this.props.class} />
            <path d={this.sweep3} class={this.props.class} />
            <path d={this.sweep4} class={this.props.class} />
            <path d={this.endPath} class={this.props.class} />
          </g>
        </g>
      </>
    );
  }
}
