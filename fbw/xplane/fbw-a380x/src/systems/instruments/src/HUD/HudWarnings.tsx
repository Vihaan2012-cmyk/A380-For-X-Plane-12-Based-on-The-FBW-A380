import { BusSignal, View, UI, Derived, ViewNode } from 'xplane-js/ui';
import { Arinc429ConsumerSubject, Arinc429LocalVarConsumerSubject, ArincEventBus } from '@flybywiresim/fbw-sdk';
import { HUDSimvars } from './shared/HUDSimvarPublisher';
import { HudElems, HudMode } from './HUDUtils';
import { Arinc429Values } from './shared/ArincValueProvider';
import { PrimFgBusEvents } from '@shared/publishers/PrimFgPublisher';
import { FcdcBusEvents } from '@shared/publishers/FcdcPublisher';
import { PrimFeBusBaseEvents } from '@shared/publishers/PrimFePublisher';
import { PanelInstrument } from 'xplane-js/data';

interface HudWarningsProps {
  bus: ArincEventBus;
  instrument: PanelInstrument;
}

export class HudWarnings extends View<HudWarningsProps> {
  private readonly warningGroupRef = UI.ref<SVGGElement>();
  private readonly sub = this.props.bus.inbox<
    HUDSimvars & HudElems & Arinc429Values & FcdcBusEvents & PrimFgBusEvents & PrimFeBusBaseEvents
  >();
  private readonly roll = Arinc429ConsumerSubject.create(this.sub.topic('rollAr').distinct());
  private readonly vStallWarn = Arinc429LocalVarConsumerSubject.create(this.sub.topic('prim_v_alpha_stall_warn'));
  private readonly airSpeed = Arinc429ConsumerSubject.create(this.sub.topic('speedAr').distinct());
  private readonly hudmode = BusSignal.from(this.sub.topic('hudMode').distinct(), 1);

  private readonly hudMode = BusSignal.from(this.sub.topic('hudFlightPhaseMode').distinct(), 0);
  private readonly autoBrakeMode = BusSignal.from(this.sub.topic('autoBrakeMode').distinct(), 0);
  private readonly brakePedalInputLeft = BusSignal.from(this.sub.topic('brakePedalInputLeft').distinct(), 0);
  private readonly brakePedalInputRight = BusSignal.from(this.sub.topic('brakePedalInputRight').distinct(), 0);
  private readonly throttle2Position = BusSignal.from(this.sub.topic('throttle2Position').distinct(), 0);
  private readonly throttle3Position = BusSignal.from(this.sub.topic('throttle3Position').distinct(), 0);

  private readonly fcdc1DiscreteWord1 = Arinc429LocalVarConsumerSubject.create(this.sub.topic('fcdc_discrete_word_1_1'));
  private readonly fcdc2DiscreteWord1 = Arinc429LocalVarConsumerSubject.create(this.sub.topic('fcdc_discrete_word_1_2'));

  private readonly isNormalLawActive = Derived.from(
    ([fcdc1DiscreteWord1, fcdc2DiscreteWord1]) =>
      fcdc1DiscreteWord1.bitValueOr(11, false) || fcdc2DiscreteWord1.bitValueOr(11, false),
    this.fcdc1DiscreteWord1,
    this.fcdc2DiscreteWord1,
  );
  afterRender(node: ViewNode): void {
    super.afterRender(node);
  }

  //   The following precedence of messages is implemented right now (first line is most important message):
  //
  //   BANK BANK
  //   STALL STALL  // already in AttitudeIndicatorWarnings , remove
  //   MAX REVERSE
  //   MAX BRAKING
  //

  render(): ViewNode {
    return (
      <g id="HudWarningGroup" ref={this.warningGroupRef} style="display: block;">
        <text
          x="640"
          y="596"
          class="FontLarge Green MiddleAlign"
          style={{
            display: Derived.from(([roll]) => {
              return Math.abs(roll.value) > 65;
            }, this.roll).derive((it) => (it ? 'block' : 'none')),
          }}
        >
          BANK BANK
        </text>
        <text
          x="640"
          y="669"
          class="FontLarge Green MiddleAlign"
          style={{
            display: Derived.from(
              ([vStallWarn, airSpeed, isNormalLawActive, hudmode]) => {
                if (hudmode === 0) {
                  if (
                    (airSpeed.value - vStallWarn.value < 0 ||
                      vStallWarn.isFailureWarning() ||
                      vStallWarn.isNoComputedData()) &&
                    !isNormalLawActive // is stall  warn only  with normal law ?
                  ) {
                    return true;
                  } else {
                    return false;
                  }
                } else {
                  return false;
                }
              },
              this.vStallWarn,
              this.airSpeed,
              this.isNormalLawActive,
              this.hudmode,
            ).derive((it) => (it ? 'block' : 'none')),
          }}
        >
          STALL STALL
        </text>

        <text
          x="640"
          y="595"
          class="FontLarge Green MiddleAlign"
          style={{
            display: Derived.from(
              ([throttle3Position, throttle2Position]) => {
                return throttle3Position < 0.1 && throttle2Position < 0.1;
              },
              this.throttle2Position,
              this.throttle3Position,
            ).derive((it) => (it ? 'block' : 'none')),
          }}
        >
          MAX REVERSE
        </text>

        <text
          x="640"
          y="565"
          class="FontLarge Green MiddleAlign"
          style={{
            display: Derived.from(
              ([brakePedalInputLeft, brakePedalInputRight, autoBrakeMode, hudMode]) => {
                return (
                  brakePedalInputLeft > 90 &&
                  brakePedalInputRight > 90 &&
                  autoBrakeMode === 0 &&
                  hudMode === HudMode.ROLLOUT_OR_RTO
                );
              },
              this.brakePedalInputLeft,
              this.brakePedalInputRight,
              this.autoBrakeMode,
              this.hudMode,
            ).derive((it) => (it ? 'block' : 'none')),
          }}
        >
          MAX BRAKING
        </text>
      </g>
    );
  }
}
