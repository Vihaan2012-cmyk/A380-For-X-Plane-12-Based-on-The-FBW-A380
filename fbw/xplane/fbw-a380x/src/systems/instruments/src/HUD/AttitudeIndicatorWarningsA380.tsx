import { View, UI, Derived, ViewNode } from 'xplane-js/ui';
import { Arinc429RegisterSubject, ArincEventBus } from '@flybywiresim/fbw-sdk';
import { TawsDataEvents } from '@flybywiresim/msfs-avionics-common';
import { PanelInstrument } from 'xplane-js/data';

interface AttitudeIndicatorWarningsA380Props {
  bus: ArincEventBus;
  instrument: PanelInstrument;
}

export class AttitudeIndicatorWarningsA380 extends View<AttitudeIndicatorWarningsA380Props> {
  private readonly warningGroupRef = UI.ref<SVGGElement>();

  private readonly gpwsWord1 = Arinc429RegisterSubject.createEmpty();

  private readonly gpwsPullUpActive = this.gpwsWord1.derive((w) => w.bitValueOr(12, false));

  private readonly gpwsSinkRateActive = this.gpwsWord1.derive((w) => w.bitValueOr(11, false));

  private readonly gpwsTerrainActive = this.gpwsWord1.derive((w) => w.bitValueOr(13, false));

  private readonly gpwsDontSinkActive = this.gpwsWord1.derive((w) => w.bitValueOr(14, false));

  private readonly gpwsTooLowGearActive = this.gpwsWord1.derive((w) => w.bitValueOr(15, false));

  private readonly gpwsTooLowTerrainActive = this.gpwsWord1.derive((w) => w.bitValueOr(17, false));

  private readonly gpwsTooLowFlapsActive = this.gpwsWord1.derive((w) => w.bitValueOr(16, false));

  private readonly gpwsGlideSlopeActive = this.gpwsWord1.derive((w) => w.bitValueOr(18, false));

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<TawsDataEvents>();

    sub
      .topic('egpws_alert_discrete_word_1_1')
      .distinct()
      .listen((v) => {
        this.gpwsWord1.setWord(v);
      });
  }

  /** The following precedence of messages is implemented right now (first line is most important message):
   * PULL UP
   * TOO LOW TERRAIN
   * TOO LOW GEAR
   * TOO LOW FLAPS
   * TERRAIN
   * GLIDESLOPE
   * DONT SINK
   * SINK RATE
   * No references for this precedence, educated guess.
   */

  render(): ViewNode {
    return (
      <g id="WarningGroupA380" ref={this.warningGroupRef} style="display: block;">
        <text
          x="640"
          y="512"
          class="FontLargest Green MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: Derived.from(([pullUp]) => pullUp, this.gpwsPullUpActive).derive((it) =>
              it ? 'block' : 'none',
            ),
          }}
        >
          PULL UP
        </text>
        <text
          x="640"
          y="512"
          class="FontLargest Green MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: Derived.from(
              ([sr, ds, gs, t, tlf, tlg, tlt, pu]) => sr && !ds && !gs && !t && !tlf && !tlg && !tlt && !pu,
              this.gpwsSinkRateActive,
              this.gpwsDontSinkActive,
              this.gpwsGlideSlopeActive,
              this.gpwsTerrainActive,
              this.gpwsTooLowFlapsActive,
              this.gpwsTooLowGearActive,
              this.gpwsTooLowTerrainActive,
              this.gpwsPullUpActive,
            ).derive((it) => (it ? 'block' : 'none')),
          }}
        >
          SINK RATE
        </text>
        <text
          x="640"
          y="512"
          class="FontLargest Green MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: Derived.from(
              ([ds, gs, t, tlf, tlg, tlt, pu]) => ds && !gs && !t && !tlf && !tlg && !tlt && !pu,
              this.gpwsDontSinkActive,
              this.gpwsGlideSlopeActive,
              this.gpwsTerrainActive,
              this.gpwsTooLowFlapsActive,
              this.gpwsTooLowGearActive,
              this.gpwsTooLowTerrainActive,
              this.gpwsPullUpActive,
            ).derive((it) => (it ? 'block' : 'none')),
          }}
        >
          DONT SINK
        </text>
        <text
          x="640"
          y="512"
          class="FontLargest Green MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: Derived.from(
              ([tlg, tlt, pu]) => tlg && !tlt && !pu,
              this.gpwsTooLowGearActive,
              this.gpwsTooLowTerrainActive,
              this.gpwsPullUpActive,
            ).derive((it) => (it ? 'block' : 'none')),
          }}
        >
          TOO LOW GEAR
        </text>
        <text
          x="640"
          y="512"
          class="FontLargest Green MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: Derived.from(
              ([tlt, pu]) => tlt && !pu,
              this.gpwsTooLowTerrainActive,
              this.gpwsPullUpActive,
            ).derive((it) => (it ? 'block' : 'none')),
          }}
        >
          TOO LOW TERRAIN
        </text>
        <text
          x="640"
          y="512"
          class="FontLargest Green MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: Derived.from(
              ([tlf, tlg, tlt, pu]) => tlf && !tlg && !tlt && !pu,
              this.gpwsTooLowFlapsActive,
              this.gpwsTooLowGearActive,
              this.gpwsTooLowTerrainActive,
              this.gpwsPullUpActive,
            ).derive((it) => (it ? 'block' : 'none')),
          }}
        >
          TOO LOW FLAPS
        </text>
        <text
          x="640"
          y="512"
          class="FontLargest Green MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: Derived.from(
              ([gs, t, tlf, tlg, tlt, pu]) => gs && !t && !tlf && !tlg && !tlt && !pu,
              this.gpwsGlideSlopeActive,
              this.gpwsTerrainActive,
              this.gpwsTooLowFlapsActive,
              this.gpwsTooLowGearActive,
              this.gpwsTooLowTerrainActive,
              this.gpwsPullUpActive,
            ).derive((it) => (it ? 'block' : 'none')),
          }}
        >
          GLIDE SLOPE
        </text>
        <text
          x="640"
          y="512"
          class="FontLargest Green MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: Derived.from(
              ([t, tlf, tlg, tlt, pu]) => t && !tlf && !tlg && !tlt && !pu,
              this.gpwsTerrainActive,
              this.gpwsTooLowFlapsActive,
              this.gpwsTooLowGearActive,
              this.gpwsTooLowTerrainActive,
              this.gpwsPullUpActive,
            ).derive((it) => (it ? 'block' : 'none')),
          }}
        >
          TERRAIN
        </text>
      </g>
    );
  }
}
