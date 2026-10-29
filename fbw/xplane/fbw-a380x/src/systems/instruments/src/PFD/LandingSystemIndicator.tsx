// @ts-strict-ignore
import {
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  Signal,
  Readable,
  ViewNode,
} from 'xplane-js/ui';
import { getDisplayIndex } from './PFD';
import { Arinc429ConsumerSubject, Arinc429LocalVarConsumerSubject, ArincEventBus } from '@flybywiresim/fbw-sdk';
import { Arinc429Values } from './shared/ArincValueProvider';
import { PFDSimvars } from './shared/PFDSimvarPublisher';
import { LagFilter } from './PFDUtils';
import { FcuEfisCpBusEvents } from '@shared/publishers/EfisCpBusPublisher';
import { FlashOneHertz } from '../MsfsAvionicsCommon/FlashingElementUtils';
import { PrimFgBusBaseEvents } from '@shared/publishers/PrimFgPublisher';
import { PanelInstrument } from 'xplane-js/data';

export class LandingSystem extends View<{ bus: ArincEventBus; instrument: PanelInstrument }> {
  private readonly sub = this.props.bus.getArincSubscriber<Arinc429Values & FcuEfisCpBusEvents>();

  private readonly altitude = Arinc429ConsumerSubject.create(this.sub.on('altitudeAr'));

  private readonly fcuEisDiscreteWord2 = Arinc429LocalVarConsumerSubject.create(null);

  private readonly lsButtonPressed = this.fcuEisDiscreteWord2.derive((word) => word.bitValueOr(14, true));

  private lsButtonPressedVisibility = false;

  private xtkValid = Signal.of(false);

  private ldevRequest = false;

  private lsGroupRef = UI.ref<SVGGElement>();

  private gsReferenceLine = UI.ref<SVGPathElement>();

  private deviationGroup = UI.ref<SVGGElement>();

  private ldevRef = UI.ref<SVGGElement>();

  private vdevRef = UI.ref<SVGGElement>();

  private handleGsReferenceLine() {
    if (this.lsButtonPressedVisibility || this.altitude.get().isNormalOperation()) {
      this.gsReferenceLine.target.style.display = 'inline';
    } else if (!this.lsButtonPressedVisibility) {
      this.gsReferenceLine.target.style.display = 'none';
    }
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const isFo = getDisplayIndex() === 2;

    this.fcuEisDiscreteWord2.setConsumer(
      this.sub.on(isFo ? 'fcu_efis_r_discrete_word_2' : 'fcu_efis_l_discrete_word_2'),
    );

    this.lsButtonPressed.watch((lsButton) => {
      this.lsButtonPressedVisibility = lsButton;
      this.lsGroupRef.target.style.display = this.lsButtonPressedVisibility ? 'inline' : 'none';
      this.deviationGroup.target.style.display = this.lsButtonPressedVisibility ? 'none' : 'inline';
      this.handleGsReferenceLine();
    }, true);

    const sub = this.props.bus.inbox<PFDSimvars & Arinc429Values>();

    this.altitude.watch(this.handleGsReferenceLine.bind(this), true);

    sub
      .topic(getDisplayIndex() === 1 ? 'ldevRequestLeft' : 'ldevRequestRight')
      .distinct()
      .listen((ldevRequest) => {
        this.ldevRequest = ldevRequest;
        this.updateLdevVisibility();
      });

    sub
      .topic('xtk')
      .distinct()
      .listen((xtk) => {
        this.xtkValid.set(Math.abs(xtk) > 0);
      });

    this.xtkValid.watch(() => {
      this.updateLdevVisibility();
    });
  }

  updateLdevVisibility() {
    this.ldevRef.target.style.display = this.ldevRequest && this.xtkValid ? 'inline' : 'none';
  }

  render(): ViewNode {
    return (
      <>
        <g id="LSGroup" ref={this.lsGroupRef} style="display: none">
          <LandingSystemInfo bus={this.props.bus} isVisible={this.lsButtonPressed} />

          <g id="LSGroup">
            <LocalizerIndicator bus={this.props.bus} instrument={this.props.instrument} />
            <GlideSlopeIndicator bus={this.props.bus} instrument={this.props.instrument} />
            <MarkerBeaconIndicator bus={this.props.bus} />
            <LsTitle bus={this.props.bus} />
          </g>
          <path ref={this.gsReferenceLine} class="Yellow Fill" d="m115.52 80.067v1.5119h-8.9706v-1.5119z" />
        </g>

        <g>
          <LsReminderIndicator bus={this.props.bus} />
        </g>
        <g id="DeviationGroup" ref={this.deviationGroup} style="display: none">
          <g id="LateralDeviationGroup" ref={this.ldevRef} style="display: none">
            <LDevIndicator bus={this.props.bus} />
          </g>
          <g id="VerticalDeviationGroup" ref={this.vdevRef} style="display: none">
            <VDevIndicator bus={this.props.bus} />
          </g>
        </g>
        <path ref={this.gsReferenceLine} class="Yellow Fill" d="m115.52 80.067v1.5119h-8.9706v-1.5119z" />
      </>
    );
  }
}

class LandingSystemInfo extends View<{ bus: Bus; isVisible: Readable<boolean> }> {
  private hasDme = false;

  private identText = Signal.of('');

  private freqTextLeading = Signal.of('');

  private freqTextTrailing = Signal.of('');

  private navFreq = 0;

  private dme = 0;

  private dmeVisibilitySub = Signal.of('hidden');

  private destRef = UI.ref<SVGTextElement>();

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<PFDSimvars>();

    sub
      .topic('hasDme')
      .distinct()
      .listen((hasDme) => {
        this.hasDme = hasDme;
        this.updateContents();
      });

    sub
      .topic('navIdent')
      .distinct()
      .listen((navIdent) => {
        this.identText.set(navIdent);
        this.updateContents();
      });

    sub
      .topic('navFreq')
      .distinct()
      .listen((navFreq) => {
        this.navFreq = navFreq;
        this.updateContents();
      });

    sub
      .topic('dme')
      .distinct()
      .listen((dme) => {
        this.dme = dme;
        this.updateContents();
      });
  }

  private updateContents() {
    const freqTextSplit = (Math.round(this.navFreq * 1000) / 1000).toString().split('.');
    this.freqTextLeading.set(freqTextSplit[0] === '0' ? '' : freqTextSplit[0]);
    if (freqTextSplit[1]) {
      this.freqTextTrailing.set(`.${freqTextSplit[1].padEnd(2, '0')}`);
    } else {
      this.freqTextTrailing.set('');
    }

    let distLeading = '';
    let distTrailing = '';
    if (this.hasDme) {
      this.dmeVisibilitySub.set('display: inline');
      const dist = Math.round(this.dme * 10) / 10;

      if (dist < 20) {
        const distSplit = dist.toString().split('.');

        distLeading = distSplit[0];
        distTrailing = `.${distSplit.length > 1 ? distSplit[1] : '0'}`;
      } else {
        distLeading = Math.round(dist).toString();
        distTrailing = '';
      }
      // eslint-disable-next-line max-len
      this.destRef.target.innerHTML = `<tspan id="ILSDistLeading" class="FontLarge StartAlign">${distLeading}</tspan><tspan id="ILSDistTrailing" class="FontSmallest StartAlign">${distTrailing}</tspan>`;
    } else {
      this.dmeVisibilitySub.set('display: none');
    }
  }

  render(): ViewNode {
    return (
      <g id="LSInfoGroup" class={{ HiddenElement: this.props.isVisible.derive((v) => !v) }}>
        <text id="ILSIdent" class="Magenta FontLarge AlignLeft" x="1.184" y="143.11522">
          {this.identText}
        </text>
        <text id="ILSFreqLeading" class="Magenta FontLarge AlignLeft" x="1.3610243" y="149.11575">
          {this.freqTextLeading}
        </text>
        <text id="ILSFreqTrailing" class="Magenta FontLarge AlignLeft" x="12.964463" y="149.24084">
          {this.freqTextTrailing}
        </text>

        <g id="ILSDistGroup" style={this.dmeVisibilitySub}>
          <text ref={this.destRef} class="Magenta AlignLeft" x="1.3685881" y="155.26602" />
          <text class="Cyan FontSmallest AlignLeft" x="17.159119" y="155.22606">
            NM
          </text>
        </g>
      </g>
    );
  }
}

class LocalizerIndicator extends View<{ bus: Bus; instrument: PanelInstrument }> {
  private lagFilter = new LagFilter(1.5);

  private rightDiamond = UI.ref<SVGPathElement>();

  private leftDiamond = UI.ref<SVGPathElement>();

  private locDiamond = UI.ref<SVGPathElement>();

  private diamondGroup = UI.ref<SVGGElement>();

  private handleNavRadialError(radialError: number): void {
    const deviation = this.lagFilter.step(radialError, this.props.instrument.frameDelta / 1000);
    const dots = deviation / 0.8;

    if (dots > 2) {
      this.rightDiamond.target.classList.remove('HiddenElement');
      this.leftDiamond.target.classList.add('HiddenElement');
      this.locDiamond.target.classList.add('HiddenElement');
    } else if (dots < -2) {
      this.rightDiamond.target.classList.add('HiddenElement');
      this.leftDiamond.target.classList.remove('HiddenElement');
      this.locDiamond.target.classList.add('HiddenElement');
    } else {
      this.locDiamond.target.classList.remove('HiddenElement');
      this.rightDiamond.target.classList.add('HiddenElement');
      this.leftDiamond.target.classList.add('HiddenElement');
      this.locDiamond.target.style.transform = `translate3d(${(dots * 30.221) / 2}px, 0px, 0px)`;
    }
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<PFDSimvars>();

    const navRadialSub = sub.topic('navRadialError').listen(this.handleNavRadialError.bind(this), true);

    sub
      .topic('hasLoc')
      .distinct()
      .listen((hasLoc) => {
        if (hasLoc) {
          this.diamondGroup.target.classList.remove('HiddenElement');
          navRadialSub.release(true);
        } else {
          this.diamondGroup.target.classList.add('HiddenElement');
          this.lagFilter.reset();
          navRadialSub.hold();
        }
      });
  }

  render(): ViewNode {
    return (
      <g id="LocalizerSymbolsGroup">
        <path
          class="NormalStroke White"
          d="m54.804 130.51a1.0073 1.0079 0 1 0-2.0147 0 1.0073 1.0079 0 1 0 2.0147 0z"
        />
        <path
          class="NormalStroke White"
          d="m39.693 130.51a1.0074 1.0079 0 1 0-2.0147 0 1.0074 1.0079 0 1 0 2.0147 0z"
        />
        <path
          class="NormalStroke White"
          d="m85.024 130.51a1.0073 1.0079 0 1 0-2.0147 0 1.0073 1.0079 0 1 0 2.0147 0z"
        />
        <path
          class="NormalStroke White"
          d="m100.13 130.51a1.0074 1.0079 0 1 0-2.0147 0 1.0074 1.0079 0 1 0 2.0147 0z"
        />
        <g class="HiddenElement" ref={this.diamondGroup}>
          <path
            id="LocDiamondRight"
            ref={this.rightDiamond}
            class="NormalStroke Magenta HiddenElement"
            d="m99.127 133.03 3.7776-2.5198-3.7776-2.5198"
          />
          <path
            id="LocDiamondLeft"
            ref={this.leftDiamond}
            class="NormalStroke Magenta HiddenElement"
            d="m38.686 133.03-3.7776-2.5198 3.7776-2.5198"
          />
          <path
            id="LocDiamond"
            ref={this.locDiamond}
            class="NormalStroke Magenta HiddenElement"
            d="m65.129 130.51 3.7776 2.5198 3.7776-2.5198-3.7776-2.5198z"
          />
        </g>
        <path id="LocalizerNeutralLine" class="Yellow Fill" d="m68.098 134.5v-8.0635h1.5119v8.0635z" />
      </g>
    );
  }
}

class GlideSlopeIndicator extends View<{ bus: Bus; instrument: PanelInstrument }> {
  private lagFilter = new LagFilter(1.5);

  private upperDiamond = UI.ref<SVGPathElement>();

  private lowerDiamond = UI.ref<SVGPathElement>();

  private glideSlopeDiamond = UI.ref<SVGPathElement>();

  private diamondGroup = UI.ref<SVGGElement>();

  private hasGlideSlope = false;

  private handleGlideSlopeError(glideSlopeError: number): void {
    const deviation = this.lagFilter.step(glideSlopeError, this.props.instrument.frameDelta / 1000);
    const dots = deviation / 0.4;

    if (dots > 2) {
      this.upperDiamond.target.classList.remove('HiddenElement');
      this.lowerDiamond.target.classList.add('HiddenElement');
      this.glideSlopeDiamond.target.classList.add('HiddenElement');
    } else if (dots < -2) {
      this.upperDiamond.target.classList.add('HiddenElement');
      this.lowerDiamond.target.classList.remove('HiddenElement');
      this.glideSlopeDiamond.target.classList.add('HiddenElement');
    } else {
      this.upperDiamond.target.classList.add('HiddenElement');
      this.lowerDiamond.target.classList.add('HiddenElement');
      this.glideSlopeDiamond.target.classList.remove('HiddenElement');
      this.glideSlopeDiamond.target.style.transform = `translate3d(0px, ${(dots * 30.238) / 2}px, 0px)`;
    }
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<PFDSimvars>();

    sub
      .topic('hasGlideslope')
      .distinct()
      .listen((hasGlideSlope) => {
        this.hasGlideSlope = hasGlideSlope;
        if (hasGlideSlope) {
          this.diamondGroup.target.classList.remove('HiddenElement');
        } else {
          this.diamondGroup.target.classList.add('HiddenElement');
          this.lagFilter.reset();
        }
      });

    sub.topic('glideSlopeError').listen((gs) => {
      if (this.hasGlideSlope) {
        this.handleGlideSlopeError(gs);
      }
    });
  }

  render(): ViewNode {
    return (
      <g id="LocalizerSymbolsGroup">
        <path
          class="NormalStroke White"
          d="m110.71 50.585a1.0074 1.0079 0 1 0-2.0147 0 1.0074 1.0079 0 1 0 2.0147 0z"
        />
        <path
          class="NormalStroke White"
          d="m110.71 65.704a1.0074 1.0079 0 1 0-2.0147 0 1.0074 1.0079 0 1 0 2.0147 0z"
        />
        <path
          class="NormalStroke White"
          d="m110.71 95.942a1.0074 1.0079 0 1 0-2.0147 0 1.0074 1.0079 0 1 0 2.0147 0z"
        />
        <path
          class="NormalStroke White"
          d="m110.71 111.06a1.0074 1.0079 0 1 0-2.0147 0 1.0074 1.0079 0 1 0 2.0147 0z"
        />
        <g class="HideGSDiamond" ref={this.diamondGroup}>
          <path
            id="GlideSlopeDiamondLower"
            ref={this.upperDiamond}
            class="NormalStroke Magenta HiddenElement"
            d="m107.19 111.06 2.5184 3.7798 2.5184-3.7798"
          />
          <path
            id="GlideSlopeDiamondUpper"
            ref={this.lowerDiamond}
            class="NormalStroke Magenta HiddenElement"
            d="m107.19 50.585 2.5184-3.7798 2.5184 3.7798"
          />
          <path
            id="GlideSlopeDiamond"
            ref={this.glideSlopeDiamond}
            class="NormalStroke Magenta HiddenElement"
            d="m109.7 77.043-2.5184 3.7798 2.5184 3.7798 2.5184-3.7798z"
          />
        </g>
      </g>
    );
  }
}

class VDevIndicator extends View<{ bus: Bus }> {
  private VDevSymbolLower = UI.ref<SVGPathElement>();

  private VDevSymbolUpper = UI.ref<SVGPathElement>();

  private VDevSymbol = UI.ref<SVGPathElement>();

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    // TODO use correct simvar once RNAV is implemented
    const deviation = 0;
    const dots = deviation / 100;

    if (dots > 2) {
      this.VDevSymbolLower.target.style.visibility = 'visible';
      this.VDevSymbolUpper.target.style.visibility = 'hidden';
      this.VDevSymbol.target.style.visibility = 'hidden';
    } else if (dots < -2) {
      this.VDevSymbolLower.target.style.visibility = 'hidden';
      this.VDevSymbolUpper.target.style.visibility = 'visible';
      this.VDevSymbol.target.style.visibility = 'hidden';
    } else {
      this.VDevSymbolLower.target.style.visibility = 'hidden';
      this.VDevSymbolUpper.target.style.visibility = 'hidden';
      this.VDevSymbol.target.style.visibility = 'visible';
      this.VDevSymbol.target.style.transform = `translate3d(0px, ${(dots * 30.238) / 2}px, 0px)`;
    }
  }

  render(): ViewNode {
    return (
      <g id="VertDevSymbolsGroup" style="display: none">
        <text class="FontSmall AlignRight Green" x="95.022" y="43.126">
          V/DEV
        </text>
        <path class="NormalStroke White" d="m108.7 65.704h2.0147" />
        <path class="NormalStroke White" d="m108.7 50.585h2.0147" />
        <path class="NormalStroke White" d="m108.7 111.06h2.0147" />
        <path class="NormalStroke White" d="m108.7 95.942h2.0147" />
        <path
          id="VDevSymbolLower"
          ref={this.VDevSymbolLower}
          class="NormalStroke Green"
          d="m107.19 111.06v2.0159h5.0368v-2.0159"
        />
        <path
          id="VDevSymbolUpper"
          ref={this.VDevSymbolUpper}
          class="NormalStroke Green"
          d="m107.19 50.585v-2.0159h5.0368v2.0159"
        />
        <path
          id="VDevSymbol"
          ref={this.VDevSymbol}
          class="NormalStroke Green"
          d="m112.22 78.807h-5.0368v4.0318h5.0368v-2.0159z"
        />
      </g>
    );
  }
}

class LDevIndicator extends View<{ bus: Bus }> {
  private LDevSymbolLeft = UI.ref<SVGPathElement>();

  private LDevSymbolRight = UI.ref<SVGPathElement>();

  private LDevSymbol = UI.ref<SVGPathElement>();

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<PFDSimvars>();

    sub
      .topic('xtk')
      .distinct()
      .rounded(3)
      .listen((xtk) => {
        const dots = xtk / 0.1;

        if (dots > 2) {
          this.LDevSymbolRight.target.style.visibility = 'visible';
          this.LDevSymbolLeft.target.style.visibility = 'hidden';
          this.LDevSymbol.target.style.visibility = 'hidden';
        } else if (dots < -2) {
          this.LDevSymbolRight.target.style.visibility = 'hidden';
          this.LDevSymbolLeft.target.style.visibility = 'visible';
          this.LDevSymbol.target.style.visibility = 'hidden';
        } else {
          this.LDevSymbolRight.target.style.visibility = 'hidden';
          this.LDevSymbolLeft.target.style.visibility = 'hidden';
          this.LDevSymbol.target.style.visibility = 'visible';
          this.LDevSymbol.target.style.transform = `translate3d(${(dots * 30.238) / 2}px, 0px, 0px)`;
        }
      });
  }

  render(): ViewNode {
    return (
      <g id="LatDeviationSymbolsGroup">
        <text class="FontSmall AlignRight Green" x="30.888" y="122.639">
          L/DEV
        </text>
        <path class="NormalStroke White" d="m38.686 129.51v2.0158" />
        <path class="NormalStroke White" d="m53.796 129.51v2.0158" />
        <path class="NormalStroke White" d="m84.017 129.51v2.0158" />
        <path class="NormalStroke White" d="m99.127 129.51v2.0158" />
        <path
          id="LDevSymbolLeft"
          ref={this.LDevSymbolLeft}
          class="NormalStroke Green"
          d="m38.686 127.99h-2.0147v5.0397h2.0147"
        />
        <path
          id="LDevSymbolRight"
          ref={this.LDevSymbolRight}
          class="NormalStroke Green"
          d="m99.127 127.99h2.0147v5.0397h-2.0147"
        />
        <path
          id="LDevSymbol"
          ref={this.LDevSymbol}
          class="NormalStroke Green"
          d="m66.892 127.99v5.0397h4.0294v-5.0397h-2.0147z"
        />
        <path id="LDevNeutralLine" class="Yellow Fill" d="m68.098 134.5v-8.0635h1.5119v8.0635z" />
      </g>
    );
  }
}

class MarkerBeaconIndicator extends View<{ bus: Bus }> {
  private classNames = Signal.of('HiddenElement');

  private markerText = Signal.of('');

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<PFDSimvars>();

    const baseClass = 'FontLarge StartAlign';

    sub
      .topic('markerBeacon')
      .distinct()
      .listen((markerState) => {
        if (markerState === 0) {
          this.classNames.set(`${baseClass} HiddenElement`);
        } else if (markerState === 1) {
          this.classNames.set(`${baseClass} Cyan OuterMarkerBlink`);
          this.markerText.set('OM');
        } else if (markerState === 2) {
          this.classNames.set(`${baseClass} Amber MiddleMarkerBlink`);
          this.markerText.set('MM');
        } else {
          this.classNames.set(`${baseClass} White InnerMarkerBlink`);
          this.markerText.set('IM');
        }
      });
  }

  render(): ViewNode {
    return (
      <text id="ILSMarkerText" class={this.classNames} x="107" y="133">
        {this.markerText}
      </text>
    );
  }
}

class LsTitle extends View<{ bus: Bus }> {
  private readonly sub = this.props.bus.inbox<FcuEfisCpBusEvents & PFDSimvars>();

  private readonly fcuEisDiscreteWord2 = Arinc429LocalVarConsumerSubject.create(null);

  private readonly lsButton = this.fcuEisDiscreteWord2.derive((word) => word.bitValueOr(14, true));

  private readonly lsTitle = UI.ref<SVGTextElement>();

  private readonly hasLoc = BusSignal.from(this.sub.topic('hasLoc'), false);

  private readonly ilsTitleShown = Derived.from(
    ([hasLoc, lsButton]) => hasLoc && lsButton,
    this.hasLoc,
    this.lsButton,
  );

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const isFo = getDisplayIndex() === 2;

    this.fcuEisDiscreteWord2.setConsumer(
      this.sub.topic(isFo ? 'fcu_efis_r_discrete_word_2' : 'fcu_efis_l_discrete_word_2'),
    );

    // normally the ident and freq should be always displayed when an ILS freq is set, but currently it only show when we have a signal
    this.ilsTitleShown.watch((it) => {
      if (it) {
        this.lsTitle.target.style.display = 'inline';
      } else {
        this.lsTitle.target.style.display = 'none';
      }
    });
  }

  render(): ViewNode {
    return (
      <text class="FontLargest Magenta MiddleAlign" ref={this.lsTitle} x="104" y="126">
        ILS
      </text>
    );
  }
}

class LsReminderIndicator extends View<{ bus: Bus }> {
  private readonly sub = this.props.bus.inbox<PrimFgBusBaseEvents & FcuEfisCpBusEvents>();

  private readonly fcuEisDiscreteWord2 = Arinc429LocalVarConsumerSubject.create(null);

  private primFgDiscreteWord1 = Arinc429LocalVarConsumerSubject.create(this.sub.topic('prim_fg_discrete_word_1'));

  private primFgDiscreteWord2 = Arinc429LocalVarConsumerSubject.create(this.sub.topic('prim_fg_discrete_word_2'));

  private primFgDiscreteWord4 = Arinc429LocalVarConsumerSubject.create(this.sub.topic('prim_fg_discrete_word_4'));

  private readonly lsButton = this.fcuEisDiscreteWord2.derive((word) => word.bitValueOr(14, true));

  private readonly approachModeSelected = Derived.from(
    ([primFgDiscreteWord1, primFgDiscreteWord2, primFgDiscreteWord4]) => {
      return (
        primFgDiscreteWord1.bitValueOr(23, false) || // LAND active
        primFgDiscreteWord2.bitValueOr(28, false) || // LAND armed
        primFgDiscreteWord2.bitValueOr(23, false) || // (F)LOC Armed
        primFgDiscreteWord4.bitValueOr(13, false) || // (F)LOC* active
        primFgDiscreteWord4.bitValueOr(14, false) // (F)LOC Active
      ); // TODO Check if LOC or G/S scales are invalid (MMR words)
    },
    this.primFgDiscreteWord1,
    this.primFgDiscreteWord2,
    this.primFgDiscreteWord4,
  );

  private readonly lsReminderVisible = Derived.from(
    ([approachModeSelected, lsPushed]) => {
      return approachModeSelected && !lsPushed; // TODO Check if LOC or G/S scales are invalid (MMR words)
    },
    this.approachModeSelected,
    this.lsButton,
  );

  render(): ViewNode {
    return (
      <FlashOneHertz bus={this.props.bus} flashDuration={9} visible={this.lsReminderVisible}>
        <text class="FontLargest Amber" x="104.33" y="124.8">
          LS
        </text>
      </FlashOneHertz>
    );
  }
}
