import { View, UI, Derived, Signal, ViewNode } from 'xplane-js/ui';

import { Arinc429RegisterSubject, ArincEventBus } from '@flybywiresim/fbw-sdk';
import { RopRowOansSimVars, FwcDataEvents, TawsDataEvents } from '../MsfsAvionicsCommon/providers';
import { PanelInstrument } from 'xplane-js/data';

interface AttitudeIndicatorWarningsProps {
  bus: ArincEventBus;
  instrument: PanelInstrument;
}

export class AttitudeIndicatorWarnings extends View<AttitudeIndicatorWarningsProps> {
  private readonly warningGroupRef = UI.ref<SVGGElement>();

  private readonly maxReverseActive = Signal.of(false);

  private readonly maxReverseMaxBrakingActive = Signal.of(false);

  private readonly ifWetRwyTooShortActive = Signal.of(false);

  private readonly rwyTooShortActive = Signal.of(false);

  private readonly rwyAheadActive = Signal.of(false);

  private readonly rowRopWord1 = Arinc429RegisterSubject.createEmpty();

  private readonly oansWord1 = Arinc429RegisterSubject.createEmpty();

  private readonly fwcWord126 = Arinc429RegisterSubject.createEmpty();

  private readonly gpwsWord2 = Arinc429RegisterSubject.createEmpty();

  // stall warning = stall bit && !on ground bit
  private readonly stallActive = this.fwcWord126.derive((v) => v.bitValueOr(17, false) && !v.bitValue(28));

  // FIXME no source yet
  private readonly stopRudderInputActive = Signal.of(false);

  private readonly windshearActive = this.gpwsWord2.derive((w) => w.bitValueOr(15, false));

  // FIXME no source yet
  private readonly wsAheadCaution = Signal.of(false);

  // FIXME no source yet
  private readonly wsAheadWarning = Signal.of(false);

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<FwcDataEvents & RopRowOansSimVars & TawsDataEvents>();

    sub
      .topic('rowRopWord1Raw')
      .distinct()
      .listen((raw) => {
        this.rowRopWord1.setWord(raw);

        this.maxReverseActive.set(this.rowRopWord1.get().bitValueOr(12, false));
        this.maxReverseMaxBrakingActive.set(this.rowRopWord1.get().bitValueOr(13, false));
        this.ifWetRwyTooShortActive.set(this.rowRopWord1.get().bitValueOr(14, false));
        this.rwyTooShortActive.set(this.rowRopWord1.get().bitValueOr(15, false));
      });

    sub
      .topic('oansWord1Raw')
      .distinct()
      .listen((raw) => {
        this.oansWord1.setWord(raw);
        this.rwyAheadActive.set(this.oansWord1.get().bitValueOr(11, false));
      });

    sub
      .topic('fwc_discrete_word_126_1')
      .distinct()
      .listen((v) => {
        this.fwcWord126.setWord(v);
      });

    sub
      .topic('egpws_alert_discrete_word_2_1')
      .distinct()
      .listen((v) => {
        this.gpwsWord2.setWord(v);
      });
  }

  render(): ViewNode {
    return (
      <g id="WarningGroup" ref={this.warningGroupRef} style="display: block;">
        <text
          x="640"
          y="70.25"
          class="FontMediumSmaller Green MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: Derived.from(
              ([ifWetTooShort, tooShort, stall]) => ifWetTooShort && !tooShort && !stall,
              this.ifWetRwyTooShortActive,
              this.rwyTooShortActive,
              this.stallActive,
            ).derive((it) => (it ? 'block' : 'none')),
          }}
        >
          IF WET:RWY TOO SHORT
        </text>
        <text
          x="640"
          y="70.25"
          class="FontLarge Green MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: Derived.from(
              ([rwyTooShort, stall]) => rwyTooShort && !stall,
              this.rwyTooShortActive,
              this.stallActive,
            ).derive((it) => (it ? 'block' : 'none')),
          }}
        >
          RWY TOO SHORT
        </text>
        <text
          x="640"
          y="78"
          class="FontLarge Green MiddleAlign Blink9Seconds TextOutline"
          style={{ display: this.stallActive.derive((it) => (it ? 'block' : 'none')) }}
        >
          STALL
          {'\xa0\xa0\xa0'}
          STALL
        </text>
        <text
          x="640"
          y="70.25"
          class="FontLarge Green MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: Derived.from(
              ([stopRudder, wetTooShort, tooShort, stall]) => stopRudder && !wetTooShort && !tooShort && !stall,
              this.stopRudderInputActive,
              this.ifWetRwyTooShortActive,
              this.rwyTooShortActive,
              this.stallActive,
            ).derive((it) => (it ? 'block' : 'none')),
          }}
        >
          STOP RUDDER INPUT
        </text>
        <text
          x="640"
          y="78"
          class="FontLarge Green MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: Derived.from(
              ([windshear, maxReverse, maxBraking]) => windshear && !maxReverse && !maxBraking,
              this.windshearActive,
              this.maxReverseActive,
              this.maxReverseMaxBrakingActive,
            ).derive((it) => (it ? 'block' : 'none')),
          }}
        >
          WINDSHEAR
        </text>
        <text
          x="640"
          y="78"
          class="FontLarge Green MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: Derived.from(
              ([wsCaution, wsWarning, maxReverse, maxBraking, windshear, stall]) =>
                wsCaution && !wsWarning && !maxReverse && !maxBraking && !windshear && !stall,
              this.wsAheadCaution,
              this.wsAheadWarning,
              this.maxReverseActive,
              this.maxReverseMaxBrakingActive,
              this.windshearActive,
              this.stallActive,
            ).derive((it) => (it ? 'block' : 'none')),
          }}
        >
          W/S AHEAD
        </text>
        <text
          x="640"
          y="78"
          class="FontLarge Green MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: Derived.from(
              ([wsAheadWarning, maxReverse, maxBraking, windshear, stall]) =>
                wsAheadWarning && !maxReverse && !maxBraking && !windshear && !stall,
              this.wsAheadWarning,
              this.maxReverseActive,
              this.maxReverseMaxBrakingActive,
              this.windshearActive,
              this.stallActive,
            ).derive((it) => (it ? 'block' : 'none')),
          }}
        >
          W/S AHEAD
        </text>
        <g style={{ display: this.rwyAheadActive.derive((it) => (it ? 'block' : 'none')) }}>
          <rect
            x="545"
            y="220"
            width="190"
            height="40"
            stroke="green"
            fill="black"
            style="stroke-color: green; stroke-width: 3px;"
          />
          <text x="640" y="252" class="FontLarge Green MiddleAlign RwyAheadAnimation TextOutline">
            RWY AHEAD
          </text>
        </g>
      </g>
    );
  }
}
