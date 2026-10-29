// Copyright (c) 2023-2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0
import { UI, ScopedView, Derived, Signal, ViewNode } from 'xplane-js/ui';

import { Arinc429RegisterSubject, ArincEventBus } from '@flybywiresim/fbw-sdk';
import { RopRowOansSimVars, FwcDataEvents, TawsDataEvents } from '../MsfsAvionicsCommon/providers';
import { PanelInstrument } from 'xplane-js/data';

interface AttitudeIndicatorWarningsProps {
  bus: ArincEventBus;
  instrument: PanelInstrument;
}

export class AttitudeIndicatorWarnings extends ScopedView<AttitudeIndicatorWarningsProps> {
  //FIXME: Allow split logic per aircraft.
  private readonly warningGroupRef = UI.ref<SVGGElement>();
  private readonly maxBrakingRequested = Signal.of(false);

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

  // FIXME: We should recieve the condition as from  dmc/cds so we can split it per aircraft
  private readonly maxReverseActive = Derived.from(
    ([maxRmB, wetTooShort, tooShort, stall]) => maxRmB && !wetTooShort && !tooShort && !stall,
    this.maxBrakingRequested,
    this.ifWetRwyTooShortActive,
    this.rwyTooShortActive,
    this.stallActive,
  ).scopedTo(this.scope);

  private readonly maxBrakingActive = Derived.from(
    ([maxBraking, wetTooShort, tooShort, stall, stopRudder]) =>
      maxBraking && !wetTooShort && !tooShort && !stall && !stopRudder,
    this.maxBrakingRequested,
    this.ifWetRwyTooShortActive,
    this.rwyTooShortActive,
    this.stallActive,
    this.stopRudderInputActive,
  ).scopedTo(this.scope);

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<FwcDataEvents & RopRowOansSimVars & TawsDataEvents>();

    sub
      .topic('rowRopWord1Raw')
      .distinct()
      .listen((raw) => {
        this.rowRopWord1.setWord(raw);
        const rowRopWord1 = this.rowRopWord1.get();
        this.maxBrakingRequested.set(rowRopWord1.bitValueOr(11, false));
        this.ifWetRwyTooShortActive.set(rowRopWord1.bitValueOr(14, false));
        this.rwyTooShortActive.set(rowRopWord1.bitValueOr(15, false));
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
          x="69"
          y="78"
          class="FontLarge Red MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: this.maxReverseActive.derive((it) => (it ? 'block' : 'none')).scopedTo(this.scope),
          }}
        >
          MAX REVERSE
        </text>
        <text
          x="69"
          y="70.25"
          class="FontLarge Red MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: this.maxBrakingActive.derive((it) => (it ? 'block' : 'none')).scopedTo(this.scope),
          }}
        >
          MAX BRAKING
        </text>
        <text
          x="69"
          y="70.25"
          class="FontMediumSmaller Amber MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: Derived.from(
              ([ifWetTooShort, tooShort, stall]) => ifWetTooShort && !tooShort && !stall,
              this.ifWetRwyTooShortActive,
              this.rwyTooShortActive,
              this.stallActive,
            )
              .derive((it) => (it ? 'block' : 'none'))
              .scopedTo(this.scope),
          }}
        >
          IF WET:RWY TOO SHORT
        </text>
        <text
          x="69"
          y="70.25"
          class="FontLarge Red MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: Derived.from(
              ([rwyTooShort, stall]) => rwyTooShort && !stall,
              this.rwyTooShortActive,
              this.stallActive,
            )
              .derive((it) => (it ? 'block' : 'none'))
              .scopedTo(this.scope),
          }}
        >
          RWY TOO SHORT
        </text>
        <text
          x="69"
          y="78"
          class="FontLarge Red MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: this.stallActive.derive((it) => (it ? 'block' : 'none')).scopedTo(this.scope),
          }}
        >
          STALL
          {'\xa0\xa0\xa0'}
          STALL
        </text>
        <text
          x="69"
          y="70.25"
          class="FontLarge Red MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: Derived.from(
              ([stopRudder, wetTooShort, tooShort, stall]) => stopRudder && !wetTooShort && !tooShort && !stall,
              this.stopRudderInputActive,
              this.ifWetRwyTooShortActive,
              this.rwyTooShortActive,
              this.stallActive,
            )
              .derive((it) => (it ? 'block' : 'none'))
              .scopedTo(this.scope),
          }}
        >
          STOP RUDDER INPUT
        </text>
        <text
          x="69"
          y="78"
          class="FontLarge Red MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: Derived.from(
              ([windshear, maxReverse, maxBraking]) => windshear && !maxReverse && !maxBraking,
              this.windshearActive,
              this.maxReverseActive,
              this.maxBrakingActive,
            )
              .derive((it) => (it ? 'block' : 'none'))
              .scopedTo(this.scope),
          }}
        >
          WINDSHEAR
        </text>
        <text
          x="69"
          y="78"
          class="FontLarge Amber MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: Derived.from(
              ([wsCaution, wsWarning, maxReverse, maxBraking, windshear, stall]) =>
                wsCaution && !wsWarning && !maxReverse && !maxBraking && !windshear && !stall,
              this.wsAheadCaution,
              this.wsAheadWarning,
              this.maxReverseActive,
              this.maxBrakingActive,
              this.windshearActive,
              this.stallActive,
            )
              .derive((it) => (it ? 'block' : 'none'))
              .scopedTo(this.scope),
          }}
        >
          W/S AHEAD
        </text>
        <text
          x="69"
          y="78"
          class="FontLarge Red MiddleAlign Blink9Seconds TextOutline"
          style={{
            display: Derived.from(
              ([wsAheadWarning, maxReverse, maxBraking, windshear, stall]) =>
                wsAheadWarning && !maxReverse && !maxBraking && !windshear && !stall,
              this.wsAheadWarning,
              this.maxReverseActive,
              this.maxBrakingActive,
              this.windshearActive,
              this.stallActive,
            )
              .derive((it) => (it ? 'block' : 'none'))
              .scopedTo(this.scope),
          }}
        >
          W/S AHEAD
        </text>
        <g
          style={{
            display: this.rwyAheadActive.derive((it) => (it ? 'block' : 'none')).scopedTo(this.scope),
          }}
        >
          <rect x="50" y="69" width="38" height="8" stroke="yellow" fill="black" />
          <text x="69" y="75.5" class="FontLarge Yellow MiddleAlign RwyAheadAnimation TextOutline">
            RWY AHEAD
          </text>
        </g>
      </g>
    );
  }
}
