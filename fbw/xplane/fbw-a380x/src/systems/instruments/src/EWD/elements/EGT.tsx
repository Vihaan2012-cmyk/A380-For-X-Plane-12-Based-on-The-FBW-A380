import {
  View,
  Readable,
  ViewNode,
  UI,
  Bus,
  BusSignal,
  Signal,
  Derived,
} from 'xplane-js/ui';
import { EwdSimvars } from '../shared/EwdSimvarPublisher';
import { GaugeComponent, GaugeMarkerComponent, GaugeMaxEGTComponent } from '../../MsfsAvionicsCommon/gauges';

interface EGTProps {
  bus: Bus;
  x: number;
  y: number;
  engine: number;
  active: Readable<boolean>;
}

export class EGT extends View<EGTProps> {
  private readonly sub = this.props.bus.inbox<EwdSimvars>();

  private readonly throttlePosition = BusSignal.from(
    this.sub.topic(`throttle_position_${this.props.engine}`).distinct(),
    0,
  );

  private readonly egt = BusSignal.from(
    this.sub.topic(`egt_${this.props.engine}`).rounded(1).distinct(),
    0,
  );

  private radius = 68;
  private startAngle = 270;
  private endAngle = 90;
  private min = 0;
  private max = 1000;

  private warningEGTColor = (EGTemperature: number, throttleMode: number) => {
    if (EGTemperature >= 900) {
      return 'Red';
    }
    if (EGTemperature > 850 && throttleMode < 3) {
      return 'Amber';
    }
    return 'Green';
  };

  private readonly amberVisible = this.throttlePosition.derive((tm) => tm < 33);

  private readonly egtColour = Derived.from(
    ([egt, tm]) => this.warningEGTColor(egt, tm),
    this.egt,
    this.throttlePosition,
  );

  // EEC trims EGT to a max value
  private readonly trimmedEGT = Derived.from(
    ([egt, throttleMode]) => Math.min([3, 4].includes(throttleMode) ? 900 : 850, egt),
    this.egt,
    this.throttlePosition,
  );

  public afterRender(node: ViewNode): void {
    super.afterRender(node);
  }

  render() {
    return (
      <>
        <g id={`EGT-indicator-${this.props.engine}`}>
          <g visibility={this.props.active.derive((it) => (!it ? 'inherit' : 'hidden'))}>
            <GaugeComponent
              x={this.props.x}
              y={this.props.y}
              radius={this.radius}
              startAngle={this.startAngle}
              endAngle={this.endAngle}
              visible={Signal.of(true)}
              class="GaugeComponent WhiteLine SW2"
            />
            <text class="F26 End Amber" x={this.props.x + 17} y={this.props.y + 11.7}>
              XX
            </text>
          </g>
          <g visibility={this.props.active.derive((it) => (it ? 'inherit' : 'hidden'))}>
            <text class={this.egtColour.derive((col) => `Large End ${col}`)} x={this.props.x + 33} y={this.props.y + 11.7}>
              {this.egt.derive((egt) =>
                Math.min([3, 4].includes(this.throttlePosition.get()) ? 900 : 850, Math.round(egt)),
              )}
            </text>
            <GaugeComponent
              x={this.props.x}
              y={this.props.y}
              radius={this.radius}
              startAngle={this.startAngle}
              endAngle={this.endAngle}
              visible={Signal.of(true)}
              class="GaugeComponent Gauge"
            >
              <GaugeComponent
                x={this.props.x}
                y={this.props.y}
                radius={this.radius - 2}
                startAngle={this.endAngle - 20}
                endAngle={this.endAngle}
                visible={Signal.of(true)}
                class="GaugeComponent Gauge ThickRedLine"
              />
              <GaugeMarkerComponent
                value={Signal.of(this.min)}
                x={this.props.x}
                y={this.props.y}
                min={this.min}
                max={this.max}
                radius={this.radius}
                startAngle={this.startAngle}
                endAngle={this.endAngle}
                class="GaugeText Gauge Medium"
              />
              <GaugeMarkerComponent
                value={Signal.of(500)}
                x={this.props.x}
                y={this.props.y}
                min={this.min}
                max={this.max}
                radius={this.radius}
                startAngle={this.startAngle}
                endAngle={this.endAngle}
                class="GaugeText Gauge"
              />
              <GaugeMarkerComponent
                value={Signal.of(this.max)}
                x={this.props.x}
                y={this.props.y}
                min={this.min}
                max={this.max}
                radius={this.radius}
                startAngle={this.startAngle}
                endAngle={this.endAngle}
                class="GaugeText Gauge RedLine"
              />
              <g visibility={this.amberVisible.derive((it) => (it ? 'inherit' : 'hidden'))}>
                <GaugeMaxEGTComponent
                  value={Signal.of(850)}
                  x={this.props.x}
                  y={this.props.y}
                  min={this.min}
                  max={this.max}
                  radius={this.radius}
                  startAngle={this.startAngle}
                  endAngle={this.endAngle}
                  class="GaugeThrustLimitIndicatorFill Gauge"
                />
              </g>
              <rect x={this.props.x - 36} y={this.props.y - 11} width={72} height={26} class="DarkGreyBox" />
              <GaugeMarkerComponent
                value={this.trimmedEGT}
                x={this.props.x}
                y={this.props.y}
                min={this.min}
                max={this.max}
                radius={this.radius}
                startAngle={this.startAngle}
                endAngle={this.endAngle}
                class={this.egtColour.derive((col) => `${col}GaugeIndicator Gauge`)}
                multiplierInner={0.75}
                indicator
                halfIndicator
              />
            </GaugeComponent>
          </g>
        </g>
      </>
    );
  }
}
