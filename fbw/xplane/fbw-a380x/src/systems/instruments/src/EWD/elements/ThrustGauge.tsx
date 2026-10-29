import {
  View,
  Readable,
  ViewNode,
  UI,
  Bus,
  Derived,
  Signal,
  BusSignal,
  Logic,
} from 'xplane-js/ui';
import { EwdSimvars } from '../shared/EwdSimvarPublisher';
import { Arinc429Values } from '../shared/ArincValueProvider';

import {
  GaugeComponent,
  GaugeMarkerComponent,
  GaugeThrustComponent,
  splitDecimals,
  ThrottlePositionDonutComponent,
  ThrustTransientComponent,
} from '../../MsfsAvionicsCommon/gauges';
import { AdrBusEvents, Arinc429ConsumerSubject, Arinc429LocalVarConsumerSubject } from '@flybywiresim/fbw-sdk';

interface ThrustGaugeProps {
  bus: Bus;
  x: number;
  y: number;
  engine: number;
  active: Readable<boolean>;
  n1Degraded: Readable<boolean>;
}

const METOTS_N1_LIMIT = 76.5;

export class ThrustGauge extends View<ThrustGaugeProps> {
  private readonly sub = this.props.bus.inbox<Arinc429Values & EwdSimvars & AdrBusEvents>();

  private readonly n1 = BusSignal.from(
    this.sub.topic(`n1_${this.props.engine}`).rounded(2).distinct(),
    0,
  );

  private readonly engineState = BusSignal.from(
    this.sub.topic(`engine_state_${this.props.engine}`).distinct(),
    0,
  );

  private readonly throttlePositionN1 = BusSignal.from(
    this.sub.topic(`throttle_position_n1_${this.props.engine}`).rounded(2).distinct(),
    0,
  );

  private readonly n1Commanded = BusSignal.from(
    this.sub.topic(`n1_commanded_${this.props.engine}`).rounded(2).distinct(),
    0,
  );

  private readonly athrEngaged = BusSignal.from(
    this.sub.topic('autothrustStatus').rounded(2).distinct(),
    0,
  ).derive((it) => it !== 0);

  private readonly thrustLimitIdle = BusSignal.from(this.sub.topic('thrust_limit_idle').distinct(), 0);
  private readonly thrustLimitToga = BusSignal.from(this.sub.topic('thrust_limit_toga').distinct(), 0);
  private readonly thrustLimitRev = BusSignal.from(this.sub.topic('thrust_limit_rev').distinct(), 0);

  private readonly cpiomBAgsDiscrete = Arinc429ConsumerSubject.create(undefined);

  private readonly revDeploying = BusSignal.from(
    this.sub.topic(`reverser_deploying_${this.props.engine}`).distinct(),
    false,
  );
  private readonly revDeployed = BusSignal.from(
    this.sub.topic(`reverser_deployed_${this.props.engine}`).distinct(),
    false,
  );
  private readonly revSelected = BusSignal.from(
    this.sub.topic(`thrust_reverse_${this.props.engine}`).distinct(),
    false,
  );

  private readonly revVisible = Derived.from(
    Logic.or(),
    this.revDeployed,
    this.revDeploying,
    this.revSelected,
  );

  private readonly thrustLimitMax = Derived.from(
    ([cpiomB, thrustLimitToga]) =>
      !cpiomB.bitValueOr(13, false) && !cpiomB.bitValueOr(14, false) ? thrustLimitToga : thrustLimitToga + 0.6,
    this.cpiomBAgsDiscrete,
    this.thrustLimitToga,
  );

  private readonly thrIdleOffset = Derived.from(
    ([es, n1, thrustLimitIdle]) =>
      es === 1
        ? 0.042
        : (es === 2 || es === 3) && thrustLimitIdle > 0
          ? 0.042 * Math.min(1, Math.max(0, n1 / thrustLimitIdle))
          : 0,
    this.engineState,
    this.n1,
    this.thrustLimitIdle,
  );

  private readonly throttleTarget = Derived.from(
    ([throttlePositionN1, thrustLimitIdle, thrustLimitMax, thrIdleOffset]) =>
      ((throttlePositionN1 - thrustLimitIdle) / (thrustLimitMax - thrustLimitIdle)) * (1 - thrIdleOffset) +
      thrIdleOffset,
    this.throttlePositionN1,
    this.thrustLimitIdle,
    this.thrustLimitMax,
    this.thrIdleOffset,
  );

  private readonly autoThrottleTarget = Derived.from(
    ([n1Commanded, thrustLimitIdle, thrustLimitMax, thrIdleOffset]) =>
      ((n1Commanded - thrustLimitIdle) / (thrustLimitMax - thrustLimitIdle)) * (1 - thrIdleOffset) + thrIdleOffset,
    this.n1Commanded,
    this.thrustLimitIdle,
    this.thrustLimitMax,
    this.thrIdleOffset,
  );

  private readonly throttleTargetReverse = Derived.from(
    ([throttlePositionN1, thrustLimitIdle, thrustLimitRev]) =>
      (throttlePositionN1 - thrustLimitIdle) / (-thrustLimitRev - thrustLimitIdle),
    this.throttlePositionN1,
    this.thrustLimitIdle,
    this.thrustLimitRev,
  );

  private readonly thrustPercent = Derived.from(
    ([n1, thrustLimitIdle, thrustLimitMax, thrIdleOffset]) =>
      ThrustGauge.thrustPercentFromN1(n1, thrustLimitIdle, thrustLimitMax, thrIdleOffset),
    this.n1,
    this.thrustLimitIdle,
    this.thrustLimitMax,
    this.thrIdleOffset,
  );

  /** Expects values in percent 0-100, returns 0-100 */
  public static thrustPercentFromN1(
    n1: number,
    thrustLimitIdle: number,
    thrustLimitMax: number,
    thrIdleOffset: number,
  ) {
    return (
      Math.min(
        1,
        Math.max(0, (n1 - thrustLimitIdle) / (thrustLimitMax - thrustLimitIdle)) * (1 - thrIdleOffset) + thrIdleOffset,
      ) * 100
    );
  }

  private readonly thrustPercentReverse = Derived.from(
    ([n1, thrustLimitIdle, thrustLimitRev]) =>
      Math.min(1, Math.max(0, (n1 - thrustLimitIdle) / (-thrustLimitRev - thrustLimitIdle))) * 100,
    this.n1,
    this.thrustLimitIdle,
    this.thrustLimitRev,
  );

  private thrustPercentSplit1 = this.thrustPercent.derive((thr) => splitDecimals(thr)[0]);
  private thrustPercentSplit2 = this.thrustPercent.derive((thr) => splitDecimals(thr)[1]);

  private readonly availVisible = Derived.from(
    ([n1, thrustLimitIdle, engineState]) => n1 > Math.floor(thrustLimitIdle) && engineState === 2,
    this.n1,
    this.thrustLimitIdle,
    this.engineState,
  );

  private readonly availRevVisible = Derived.from(
    Logic.or(),
    this.availVisible,
    this.revDeployed,
    this.revDeploying,
  );

  private readonly availRevText = this.availVisible.derive((it) => (it ? 'AVAIL' : 'REV'));

  private readonly airspeed1 = Arinc429LocalVarConsumerSubject.create(this.sub.topic('adr_computed_airspeed_1'));
  private readonly airspeed2 = Arinc429LocalVarConsumerSubject.create(this.sub.topic('adr_computed_airspeed_2'));
  private readonly airspeed3 = Arinc429LocalVarConsumerSubject.create(this.sub.topic('adr_computed_airspeed_3'));
  private readonly airspeed = Derived.from(
    ([airspeed1, airspeed2, airspeed3]) =>
      !airspeed1.isFailureWarning()
        ? airspeed1.valueOr(0)
        : !airspeed2.isFailureWarning()
          ? airspeed2.valueOr(0)
          : airspeed3.valueOr(0),
    this.airspeed1,
    this.airspeed2,
    this.airspeed3,
  );

  // FIXME replace with actual available thrust when ACUTE is implemented
  private readonly maxThrustAvail = Derived.from(
    ([cas, thrustLimitIdle, thrustLimitMax, thrIdleOffset]) =>
      cas < 35
        ? Math.max(
            thrIdleOffset * 100,
            ThrustGauge.thrustPercentFromN1(METOTS_N1_LIMIT, thrustLimitIdle, thrustLimitMax, thrIdleOffset),
          ) / 10
        : 10,
    this.airspeed,
    this.thrustLimitIdle,
    this.thrustLimitMax,
    this.thrIdleOffset,
  );

  private radius = 64;
  private startAngle = 230;
  private endAngle = 90;
  private min = 0;
  private max = 10;
  private revStartAngle = 130;
  private revEndAngle = 230;
  private revRadius = 58;
  private revMin = 0;
  private revMax = 3;

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.cpiomBAgsDiscrete.setConsumer(this.sub.topic('cpiomBAgsDiscrete'));
  }

  render() {
    return (
      <>
        <g id={`Thrust-indicator-${this.props.engine}`}>
          <g
            visibility={Derived.from(
              ([a, n1d]) => (!a || n1d ? 'inherit' : 'hidden'),
              this.props.active,
              this.props.n1Degraded,
            )}
          >
            <GaugeComponent
              x={this.props.x}
              y={this.props.y + 20}
              radius={this.radius}
              startAngle={320}
              endAngle={40}
              visible={Signal.of(true)}
              largeArc={0}
              sweep={0}
              class="GaugeComponent SW2 AmberLine"
            />
            <text class="F26 End Amber Spread" x={this.props.x + 55} y={this.props.y - 48}>
              THR XX
            </text>
          </g>
          <g
            visibility={Derived.from(
              ([a, n1d]) => (a && !n1d ? 'inherit' : 'hidden'),
              this.props.active,
              this.props.n1Degraded,
            )}
          >
            <g visibility={this.revVisible.derive((it) => (!it ? 'inherit' : 'hidden'))}>
              <text class="F26 End Green" x={this.props.x + 48} y={this.props.y + 47}>
                {this.thrustPercentSplit1}
              </text>
              <text class="F26 End Green" x={this.props.x + 62} y={this.props.y + 47}>
                .
              </text>
              <text class="F20 End Green" x={this.props.x + 78} y={this.props.y + 47}>
                {this.thrustPercentSplit2}
              </text>
              <GaugeThrustComponent
                x={this.props.x}
                y={this.props.y}
                valueIdle={Signal.of(0.42)}
                valueMax={this.maxThrustAvail}
                radius={this.radius}
                startAngle={this.startAngle}
                endAngle={this.endAngle}
                min={this.min}
                max={this.max}
                visible={Derived.from(
                  ([availVis, es]) => availVis || es === 1,
                  this.availVisible,
                  this.engineState,
                )}
                class="GaugeComponent GaugeThrustFill"
              />
              <AvailRev
                x={this.props.x - 18}
                y={this.props.y - 14}
                mesg={this.availRevText}
                engineNumber={this.props.engine}
                visible={this.availRevVisible}
                revDoorOpen={this.revDeployed}
              />
              <ThrustTransientComponent
                x={this.props.x}
                y={this.props.y}
                min={this.min / 10}
                max={this.max / 10}
                thrustActual={this.thrustPercent.derive((it) => it / 100)}
                thrustTarget={this.autoThrottleTarget}
                radius={this.radius}
                startAngle={this.startAngle}
                endAngle={this.endAngle}
                visible={this.athrEngaged}
                class="TransientIndicator"
              />
            </g>
            <GaugeComponent
              x={this.props.x}
              y={this.props.y}
              radius={this.radius}
              startAngle={this.startAngle}
              endAngle={this.endAngle}
              visible={Signal.of(true)}
              class="GaugeComponent Gauge"
            >
              <GaugeMarkerComponent
                value={Signal.of(0)}
                x={this.props.x}
                y={this.props.y}
                min={this.min}
                max={this.max}
                radius={this.radius}
                startAngle={this.startAngle}
                endAngle={this.endAngle}
                class="GaugeText Gauge"
                showValue
                textNudgeX={10}
                textNudgeY={-2}
                multiplierInner={0.9}
              />
              <GaugeMarkerComponent
                value={Signal.of(2.5)}
                x={this.props.x}
                y={this.props.y}
                min={this.min}
                max={this.max}
                radius={this.radius}
                startAngle={this.startAngle}
                endAngle={this.endAngle}
                class="GaugeText Gauge"
                textNudgeY={6}
                textNudgeX={13}
                multiplierInner={0.9}
              />
              <GaugeMarkerComponent
                value={Signal.of(5)}
                x={this.props.x}
                y={this.props.y}
                min={this.min}
                max={this.max}
                radius={this.radius}
                startAngle={this.startAngle}
                endAngle={this.endAngle}
                class="GaugeText Gauge"
                showValue
                textNudgeX={5}
                textNudgeY={15}
                multiplierInner={0.9}
              />
              <GaugeMarkerComponent
                value={Signal.of(7.5)}
                x={this.props.x}
                y={this.props.y}
                min={this.min}
                max={this.max}
                radius={this.radius}
                startAngle={this.startAngle}
                endAngle={this.endAngle}
                class="GaugeText Gauge"
                multiplierInner={0.9}
              />
              <GaugeMarkerComponent
                value={Signal.of(10)}
                x={this.props.x}
                y={this.props.y}
                min={this.min}
                max={this.max}
                radius={this.radius}
                startAngle={this.startAngle}
                endAngle={this.endAngle}
                class="GaugeText Gauge"
                showValue
                textNudgeY={7}
                textNudgeX={-27}
                multiplierInner={0.9}
              />
              <g visibility={this.revVisible.derive((it) => (!it ? 'inherit' : 'hidden'))}>
                <rect x={this.props.x - 11} y={this.props.y + 21} width={96} height={30} class="DarkGreyBox" />
                <GaugeMarkerComponent
                  value={this.thrustPercent.derive((it) => it / 10)}
                  x={this.props.x}
                  y={this.props.y}
                  min={this.min}
                  max={this.max}
                  radius={this.radius}
                  startAngle={this.startAngle}
                  endAngle={this.endAngle}
                  class="GaugeIndicator Gauge"
                  multiplierOuter={1.1}
                  indicator
                />
              </g>
            </GaugeComponent>
            <g visibility={this.revSelected.derive((it) => (!it ? 'inherit' : 'hidden'))}>
              <ThrottlePositionDonutComponent
                value={this.throttleTarget.derive((it) => it * 10)}
                x={this.props.x}
                y={this.props.y}
                min={this.min}
                max={this.max}
                radius={this.radius}
                startAngle={this.startAngle}
                endAngle={this.endAngle}
                class="DonutThrottleIndicator"
              />
            </g>
            <g visibility={this.revSelected.derive((it) => (it ? 'inherit' : 'hidden'))}>
              <ThrottlePositionDonutComponent
                value={this.throttleTargetReverse.derive((it) => this.revMax - it * 3)}
                x={this.props.x}
                y={this.props.y}
                min={this.revMin}
                max={this.revMax}
                radius={this.revRadius}
                startAngle={this.revStartAngle}
                endAngle={this.revEndAngle}
                class="DonutThrottleIndicator"
              />
            </g>
          </g>
          <g
            visibility={Derived.from(
              ([active, revVisible, n1Degraded]) => active && revVisible && !n1Degraded,
              this.props.active,
              this.revVisible,
              this.props.n1Degraded,
            ).derive((it) => (it ? 'inherit' : 'hidden'))}
          >
            <GaugeThrustComponent
              x={this.props.x}
              y={this.props.y}
              valueIdle={Signal.of(0.04)}
              valueMax={Signal.of(2.6)}
              radius={this.revRadius}
              startAngle={this.revStartAngle}
              endAngle={this.revEndAngle}
              min={this.revMin}
              max={this.revMax}
              visible={Derived.from(
                ([revDeploying, revDeployed, engineState]) => revDeploying || revDeployed || engineState === 1,
                this.revDeploying,
                this.revDeploying,
                this.engineState,
              )}
              class="GaugeComponent GaugeThrustFill"
              reverse
            />
            {/* reverse */}
            <GaugeComponent
              x={this.props.x}
              y={this.props.y}
              radius={this.revRadius}
              startAngle={this.revStartAngle}
              endAngle={this.revEndAngle}
              visible={Signal.of(true)}
              class="GaugeComponent Gauge"
            >
              <AvailRev
                x={this.props.x - 18}
                y={this.props.y - 14}
                mesg={this.availRevText}
                engineNumber={this.props.engine}
                visible={this.availRevVisible}
                revDoorOpen={this.revDeployed}
              />
              <GaugeMarkerComponent
                value={Signal.of(0)}
                x={this.props.x}
                y={this.props.y}
                min={this.revMin}
                max={this.revMax}
                radius={this.revRadius}
                startAngle={this.revStartAngle}
                endAngle={this.revEndAngle}
                class="GaugeText Gauge"
                textNudgeX={-24}
                textNudgeY={-6}
                multiplierInner={1.1}
                showValue
                overrideText="MAX"
              />
              <GaugeMarkerComponent
                value={Signal.of(this.revMax / 2)}
                x={this.props.x}
                y={this.props.y}
                min={this.revMin}
                max={this.revMax}
                radius={this.revRadius}
                startAngle={this.revStartAngle}
                endAngle={this.revEndAngle}
                class="GaugeText Gauge"
                multiplierInner={1.1}
              />
              <GaugeMarkerComponent
                value={this.thrustPercentReverse.derive((thr) => this.revMax - thr * 0.03)}
                x={this.props.x}
                y={this.props.y}
                min={this.revMin}
                max={this.revMax}
                radius={this.revRadius}
                startAngle={this.revStartAngle}
                endAngle={this.revEndAngle}
                class="GaugeIndicator Gauge"
                multiplierOuter={1.1}
                indicator
              />
            </GaugeComponent>
          </g>
        </g>
      </>
    );
  }
}

interface AvailRevProps {
  x: number;
  y: number;
  mesg: Readable<'AVAIL' | 'REV'>;
  engineNumber: number;
  visible: Readable<boolean>;
  revDoorOpen: Readable<boolean>;
}

class AvailRev extends View<AvailRevProps> {
  public afterRender(node: ViewNode): void {
    super.afterRender(node);
  }

  render() {
    return (
      <g visibility={this.props.visible.derive((it) => (it ? 'inherit' : 'hidden'))}>
        <rect x={this.props.x - 28} y={this.props.y - 13} width={90} height={24} class="DarkGreyBox BackgroundFill" />
        <g visibility={this.props.mesg.derive((mesg) => (mesg === 'REV' ? 'inherit' : 'hidden'))}>
          <text
            class={this.props.revDoorOpen.derive((it) => `F26 Spread Centre ${it ? 'Green' : 'Amber'}`)}
            x={this.props.x - 25}
            y={this.props.y + 9}
          >
            {`REV ${this.props.engineNumber}`}
          </text>
        </g>
        <g visibility={this.props.mesg.derive((mesg) => (mesg === 'AVAIL' ? 'inherit' : 'hidden'))}>
          <text class="F26 Spread Centre Green" x={this.props.x - 26} y={this.props.y + 9}>
            AVAIL
          </text>
        </g>
      </g>
    );
  }
}
