import {
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  Signal,
  Readable,
  ViewNode,
  ClockTopics,
  Ref,
} from 'xplane-js/ui';
import { getDisplayIndex } from './HUD';
import { Arinc429ConsumerSubject, Arinc429LocalVarConsumerSubject, ArincEventBus } from '@flybywiresim/fbw-sdk';
import { Arinc429Values } from './shared/ArincValueProvider';
import { HUDSimvars } from './shared/HUDSimvarPublisher';
import { HudElems, LagFilter } from './HUDUtils';
import { FIVE_DEG } from './HUDUtils';
import { calculateHorizonOffsetFromPitch } from './HUDUtils';
import { FcuEfisCpBusEvents } from '@shared/publishers/EfisCpBusPublisher';
import { FlashOneHertz } from '../MsfsAvionicsCommon/FlashingElementUtils';
import { PrimFgBusBaseEvents } from '@shared/publishers/PrimFgPublisher';
import { PanelInstrument } from 'xplane-js/data';

export class LandingSystem extends View<{ bus: ArincEventBus; instrument: PanelInstrument }> {
  private readonly sub = this.props.bus.getArincSubscriber<
    Arinc429Values & FcuEfisCpBusEvents & HudElems & HUDSimvars & ClockTopics
  >();
  private lsButtonPressedVisibility = false;

  private readonly altitude = Arinc429ConsumerSubject.create(this.sub.on('altitudeAr'));

  private readonly fcuEisDiscreteWord2 = Arinc429LocalVarConsumerSubject.create(null);

  private readonly lsButtonPressed = this.fcuEisDiscreteWord2.derive((word) => word.bitValueOr(14, true));

  private xtkValid = Signal.of(false);

  private isDecluttered = Signal.of(false);

  private ldevRequest = false;

  private lsGroupRef = UI.ref<SVGGElement>();

  private deviationGroup = UI.ref<SVGGElement>();

  private ldevRef = UI.ref<SVGGElement>();

  private groupVis = false;
  private hudFlightPhaseMode = 0;

  private readonly declutterModeL = BusSignal.from(this.sub.on('declutterModeL'), 0);
  private readonly declutterModeR = BusSignal.from(this.sub.on('declutterModeR'), 0);
  private gsVis = '';

  private handleGsReferenceLine() {
    if (this.lsButtonPressedVisibility || this.altitude.get().isNormalOperation()) {
      this.deviationGroup.target.style.display = 'inline';
    } else if (!this.lsButtonPressedVisibility) {
      this.deviationGroup.target.style.display = 'none';
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
      this.lsGroupRef.target.style.display = this.lsButtonPressedVisibility && this.groupVis ? 'inline' : 'none';
      this.deviationGroup.target.style.display = this.lsButtonPressedVisibility && this.groupVis ? 'none' : 'inline';
      this.handleGsReferenceLine();
    }, true);

    const sub = this.props.bus.inbox<HUDSimvars & Arinc429Values & HudElems>();

    this.altitude.watch(this.handleGsReferenceLine.bind(this), true);

    sub
      .topic('IlsGS')
      .distinct()
      .listen((value) => {
        this.gsVis = value;
        value === 'block' ? (this.groupVis = true) : (this.groupVis = false);
      });

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
        <g id="LSGroup" ref={this.lsGroupRef} transform=" translate(0 0)">
          <LandingSystemInfo bus={this.props.bus} isVisible={this.lsButtonPressed} />

          <g id="LSGroup">
            <LocalizerIndicator bus={this.props.bus} instrument={this.props.instrument} />
            <GlideSlopeIndicator bus={this.props.bus} instrument={this.props.instrument} />
            <MarkerBeaconIndicator bus={this.props.bus} />
            <LsTitle bus={this.props.bus} />
          </g>
        </g>

        <g>
          <LsReminderIndicator bus={this.props.bus} />
        </g>
        <g id="DeviationGroup" ref={this.deviationGroup} style="display: none">
          <g id="LateralDeviationGroup" ref={this.ldevRef} style="display: none">
            {/* ////TODO rnav use 3.0 scaling */}
            <LDevIndicator bus={this.props.bus} />
          </g>
          <g id="VerticalDeviationGroup" style="display: none">
            <VDevIndicator bus={this.props.bus} />
          </g>
        </g>
        {/* <path ref={this.gsReferenceLine} class="Green Fill" d="m404.32 280.234v5.292h-31.397v-5.292z" /> */}
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

  private elemVis = false;
  private hasLoc = false;
  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<HUDSimvars & HudElems>();

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
      this.destRef.target.innerHTML = `<tspan id="ILSDistLeading" class="FontSmallest  StartAlign">${distLeading}</tspan><tspan id="ILSDistTrailing" class="FontSmallest StartAlign">${distTrailing}</tspan>`;
    } else {
      this.dmeVisibilitySub.set('display: none');
    }
  }

  render(): ViewNode {
    return (
      <g
        id="LSInfoGroup"
        class={{ HiddenElement: this.props.isVisible.derive((v) => !v) }}
        transform=" translate(115 320)"
      >
        <text id="ILSIdent" class="Green FontSmallest  AlignLeft" x="15" y="490">
          {this.identText}
        </text>
        <text id="ILSFreqLeading" class="Green FontSmallest  AlignLeft" x="15" y="511.9">
          {this.freqTextLeading}
        </text>
        <text id="ILSFreqTrailing" class="Green FontSmallest  AlignLeft" x="60" y="512">
          {this.freqTextTrailing}
        </text>

        <g id="ILSDistGroup" style={this.dmeVisibilitySub}>
          <text ref={this.destRef} class="Green AlignLeft" x="15" y="535" />
          <text class="Green FontTiny AlignLeft" x="65" y="535">
            NM
          </text>
        </g>
      </g>
    );
  }
}

class LocalizerIndicator extends View<{ bus: ArincEventBus; instrument: PanelInstrument }> {
  private readonly sub = this.props.bus.getArincSubscriber<
    Arinc429Values & HudElems & HUDSimvars & ClockTopics & FcuEfisCpBusEvents
  >();
  private flightPhase = -1;
  private fmgcFlightPhase = -1;
  private declutterMode = 0;
  private onGround = true;
  private LsState = false;
  private LSLocRef = UI.ref<SVGGElement>();
  private LSLocGroupVerticalOffset = 0;
  private pitch = 0;
  private locVis = '';
  private locVisBool = false;
  private lsBtnState = false;
  private lagFilter = new LagFilter(1.5);

  private readonly fcuEisDiscreteWord2 = Arinc429LocalVarConsumerSubject.create(null);
  private readonly lsButtonPressed = this.fcuEisDiscreteWord2.derive((word) => word.bitValueOr(14, true));

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
      this.locDiamond.target.style.transform = `translate3d(${(dots * 90.6) / 2}px, 0px, 0px)`;
    }
  }
  private setLocGroupPos() {
    this.LSLocRef.target.style.transform = `translate3d(433.5px, 400px, 0px)`;
  }
  afterRender(node: ViewNode): void {
    super.afterRender(node);
    const isFo = getDisplayIndex() === 2;

    this.fcuEisDiscreteWord2.setConsumer(
      this.sub.on(isFo ? 'fcu_efis_r_discrete_word_2' : 'fcu_efis_l_discrete_word_2'),
    );
    const navRadialSub = this.sub.on('navRadialError').listen(this.handleNavRadialError.bind(this), true);

    this.sub
      .on('fwcFlightPhase')
      .distinct()
      .listen((fp) => {
        this.flightPhase = fp;
      });
    this.sub
      .on('fmgcFlightPhase')
      .distinct()
      .listen((fp) => {
        this.fmgcFlightPhase = fp;
      });

    this.sub
      .on('IlsLoc')
      .distinct()
      .listen((value) => {
        this.locVis = value;
        this.LSLocRef.target.style.display = `${this.locVis}`;
      });
    this.sub
      .on('decMode')
      .distinct()
      .listen((value) => {
        this.declutterMode = value;
        this.setLocGroupPos();
      });

    this.sub
      .on('hasLoc')
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

    this.lsButtonPressed.watch((lsButton) => {
      this.LsState = lsButton;
      this.setLocGroupPos();
    }, true);

    this.sub.on('pitchAr').listen((p) => {
      this.pitch = p.value;
    });
  }

  render(): ViewNode {
    return (
      <g ref={this.LSLocRef} id="LocalizerSymbolsGroup">
        <path class="NormalStroke Green" d="m164.412 391.53a3.022 3.024 0 1 0 -6.044 0 3.022 3.024 0 1 0 6.044 0z" />
        <path class="NormalStroke Green" d="m119.079 391.53a3.022 3.024 0 1 0 -6.044 0 3.022 3.024 0 1 0 6.044 0z" />
        <path class="NormalStroke Green" d="m255.072 391.53a3.022 3.024 0 1 0 -6.044 0 3.022 3.024 0 1 0 6.044 0z" />
        <path class="NormalStroke Green" d="m300.39 391.53a3.022 3.024 0 1 0 -6.044 0 3.022 3.024 0 1 0 6.044 0z" />
        <g class="HiddenElement" ref={this.diamondGroup}>
          <path
            id="LocDiamondRight"
            ref={this.rightDiamond}
            class="NormalStroke Green HiddenElement"
            d="m297.381 399.09 11.333 -7.559 -11.333 -7.559"
          />
          <path
            id="LocDiamondLeft"
            ref={this.leftDiamond}
            class="NormalStroke Green HiddenElement"
            d="m116.058 399.09 -11.333 -7.559 11.333 -7.559"
          />
          <path
            id="LocDiamond"
            ref={this.locDiamond}
            class="NormalStroke Green HiddenElement"
            d="m195.387 391.53 11.333 7.559 11.333 -7.559 -11.333 -7.559z"
          />
        </g>
        <path id="LocalizerNeutralLine" class="Green Fill" d="m 204 406.5v -30 h 5 v 30" />
      </g>
    );
  }
}

class GlideSlopeIndicator extends View<{ bus: Bus; instrument: PanelInstrument }> {
  private readonly sub = this.props.bus.inbox<FcuEfisCpBusEvents & HUDSimvars>();
  private lagFilter = new LagFilter(1.5);

  private upperDiamond = UI.ref<SVGPathElement>();

  private lowerDiamond = UI.ref<SVGPathElement>();

  private glideSlopeDiamond = UI.ref<SVGPathElement>();

  private diamondGroup = UI.ref<SVGGElement>();

  private hasGlideSlope = false;

  private LSGsRef = new Ref<SVGGElement>();

  private readonly fcuEisDiscreteWord2 = Arinc429LocalVarConsumerSubject.create(null);

  private readonly lsButtonPressed = this.fcuEisDiscreteWord2.derive((word) => word.bitValueOr(14, true));

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
      this.glideSlopeDiamond.target.style.transform = `translate3d(0px, ${(dots * 90.6) / 2}px, 0px)`;
    }
  }

  private crosswindMode = false;
  private pitch = 0;

  private gsReferenceLine = UI.ref<SVGPathElement>();

  private deviationGroup = UI.ref<SVGGElement>();

  private GsVis = '';

  private MoveGlideSlopeGroup() {
    if (this.crosswindMode == false) {
      this.LSGsRef.target.style.transform = `translate3d(665px, ${FIVE_DEG + 13 + calculateHorizonOffsetFromPitch(this.pitch)}px, 0px)`;
    } else {
      this.LSGsRef.target.style.transform = `translate3d(665px, 84px, 0px)`;
    }
  }
  afterRender(node: ViewNode): void {
    super.afterRender(node);
    const isFo = getDisplayIndex() === 2;

    this.fcuEisDiscreteWord2.setConsumer(
      this.sub.topic(isFo ? 'fcu_efis_r_discrete_word_2' : 'fcu_efis_l_discrete_word_2'),
    );

    this.lsButtonPressed.watch(this.MoveGlideSlopeGroup.bind(this), true);

    const sub = this.props.bus.inbox<HUDSimvars & Arinc429Values & ClockTopics & HudElems>();

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

    sub
      .topic('pitchAr')
      .distinct()
      .listen((p) => {
        this.pitch = p.value;
        this.MoveGlideSlopeGroup();
      });

    sub
      .topic('IlsGS')
      .distinct()
      .listen((value) => {
        this.GsVis = value;
        this.LSGsRef.target.style.display = `${this.GsVis}`;
      });
    sub
      .topic('cWndMode')
      .distinct()
      .listen((value) => {
        this.crosswindMode = value;
        this.MoveGlideSlopeGroup();
      });
  }

  render(): ViewNode {
    return (
      <g id="GlideSlopeSymbolsGroup" ref={this.LSGsRef}>
        <path id="GsGroupMask" d="m 314 149 h 30 v 184 h-30 z" class="BlackkFill" />
        <path class="NormalStroke Green" d="m332.13 151.755a3.022 3.024 0 1 0 -6.044 0 3.022 3.024 0 1 0 6.044 0z" />
        <path class="NormalStroke Green" d="m332.13 197.112a3.022 3.024 0 1 0 -6.044 0 3.022 3.024 0 1 0 6.044 0z" />
        <path class="NormalStroke Green" d="m332.13 287.826a3.022 3.024 0 1 0 -6.044 0 3.022 3.024 0 1 0 6.044 0z" />
        <path class="NormalStroke Green" d="m332.13 333.18a3.022 3.024 0 1 0 -6.044 0 3.022 3.024 0 1 0 6.044 0z" />
        <g class="HideGSDiamond" ref={this.diamondGroup}>
          <path
            id="GlideSlopeDiamondLower"
            ref={this.upperDiamond}
            class="NormalStroke Green HiddenElement"
            d="m321.57 333.18 7.555 11.339 7.555 -11.339"
          />
          <path
            id="GlideSlopeDiamondUpper"
            ref={this.lowerDiamond}
            class="NormalStroke Green HiddenElement"
            d="m321.57 151.755 7.555 -11.339 7.555 11.339"
          />
          <path
            id="GlideSlopeDiamond"
            ref={this.glideSlopeDiamond}
            class="NormalStroke Green HiddenElement"
            d="m329.1 231.129 -7.555 11.339 7.555 11.339 7.555 -11.339z"
          />
        </g>
        <path ref={this.gsReferenceLine} class="Green Fill" d="m 314 243 h 30 v 5 h -30 z" />
      </g>
    );
  }
}

class VDevIndicator extends View<{ bus: Bus }> {
  private VDevSymbolLower = UI.ref<SVGPathElement>();

  private VDevSymbolUpper = UI.ref<SVGPathElement>();

  private VDevSymbol = UI.ref<SVGPathElement>();
  private VdevRef = UI.ref<SVGGElement>();
  private pitch = 0;
  private needsUpdate = false;
  private crosswindMode = false;
  afterRender(node: ViewNode): void {
    super.afterRender(node);
    const sub = this.props.bus.inbox<Arinc429Values & HudElems & ClockTopics>();
    sub
      .topic('pitchAr')
      .distinct()
      .listen((p) => {
        this.pitch = p.value;
        this.needsUpdate = true;
      });
    sub
      .topic('cWndMode')
      .distinct()
      .listen((p) => {
        this.crosswindMode = p;
        this.needsUpdate = true;
      });

    sub.topic('realTime').listen((_t) => {
      if (this.needsUpdate) {
        this.needsUpdate = false;
        this.MoveGlideSlopeGroup();
      }
    });

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

  private MoveGlideSlopeGroup() {
    if (this.crosswindMode == false) {
      this.VdevRef.target.style.transform = `translate3d(665px, ${FIVE_DEG + 13 + calculateHorizonOffsetFromPitch(this.pitch)}px, 0px)`;
    } else {
      this.VdevRef.target.style.transform = `translate3d(665px, 84px, 0px)`;
    }
    //DistanceSpacing
  }

  render(): ViewNode {
    return (
      <g id="VertDevSymbolsGroup" ref={this.VdevRef} style="display: none">
        <text class="FontSmall AlignRight Green" x="285.06600000000003" y="129.378">
          V/DEV
        </text>
        <path id="vDevGroupMask" d="m 314 149 h 30 v 184 h-30 z" class="BlackkFill" />
        <path class="NormalStroke Green" d="m326.1 197.112h6.044" />
        <path class="NormalStroke Green" d="m326.1 151.755h6.044" />
        <path class="NormalStroke Green" d="m326.1 333.18h6.044" />
        <path class="NormalStroke Green" d="m326.1 287.826h6.044" />
        <path
          id="VDevSymbolLower"
          ref={this.VDevSymbolLower}
          class="NormalStroke Green"
          d="m321.57 333.18v6.048h15.11v-6.048"
        />
        <path
          id="VDevSymbolUpper"
          ref={this.VDevSymbolUpper}
          class="NormalStroke Green"
          d="m321.57 151.755v-6.048h15.11v6.048"
        />
        <path
          id="VDevSymbol"
          ref={this.VDevSymbol}
          class="NormalStroke Green"
          d="m336.66 236.421h-15.11v12.095h15.11v-6.048z"
        />
        <path class="Green Fill" d="m 314 243 h 30 v 5 h -30 z" />
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

    const sub = this.props.bus.inbox<HUDSimvars>();

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
      <g id="LatDeviationSymbolsGroup" transform="translate(433.5 400)">
        <text class="FontSmall AlignRight Green" x="92.664" y="367.917">
          L/DEV
        </text>
        <path class="NormalStroke Green" d="m116.058 388.53v6.047" />
        <path class="NormalStroke Green" d="m161.388 388.53v6.047" />
        <path class="NormalStroke Green" d="m252.051 388.53v6.047" />
        <path class="NormalStroke Green" d="m297.381 388.53v6.047" />
        <path
          id="LDevSymbolLeft"
          ref={this.LDevSymbolLeft}
          class="NormalStroke Green"
          d="m116.058 383.97h-6.044v15.119h6.044"
        />
        <path
          id="LDevSymbolRight"
          ref={this.LDevSymbolRight}
          class="NormalStroke Green"
          d="m297.381 383.97h6.044v15.119h-6.044"
        />
        <path
          id="LDevSymbol"
          ref={this.LDevSymbol}
          class="NormalStroke Green"
          d="m200.676 383.97v15.119h12.088v-15.119h-6.044z"
        />
        <path id="LDevNeutralLine" class="Green Fill" d="m 204 406.5v -30 h 5 v 30" />
      </g>
    );
  }
}

class MarkerBeaconIndicator extends View<{ bus: Bus }> {
  private classNames = Signal.of('HiddenElement');

  private markerText = Signal.of('');

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<HUDSimvars>();

    const baseClass = 'FontMedium  StartAlign';

    sub
      .topic('markerBeacon')
      .distinct()
      .listen((markerState) => {
        if (markerState === 0) {
          this.classNames.set(`${baseClass} HiddenElement`);
        } else if (markerState === 1) {
          this.classNames.set(`${baseClass} Green OuterMarkerBlink`);
          this.markerText.set('OM');
        } else if (markerState === 2) {
          this.classNames.set(`${baseClass} Green MiddleMarkerBlink`);
          this.markerText.set('MM');
        } else {
          this.classNames.set(`${baseClass} Green InnerMarkerBlink`);
          this.markerText.set('IM');
        }
      });
  }

  render(): ViewNode {
    return (
      <text id="ILSMarkerText" class={this.classNames} x="760" y="800">
        {this.markerText}
      </text>
    );
  }
}

class LsTitle extends View<{ bus: Bus }> {
  private readonly sub = this.props.bus.inbox<FcuEfisCpBusEvents & HUDSimvars>();

  private readonly lsTitle = UI.ref<SVGTextElement>();

  private readonly fcuEisDiscreteWord2 = Arinc429LocalVarConsumerSubject.create(null);

  private readonly lsButton = this.fcuEisDiscreteWord2.derive((word) => word.bitValueOr(14, true));

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
      <text class="FontSmallest Green StartAlign" ref={this.lsTitle} x="130" y="788">
        ILS
      </text>
    );
  }
}

class LsReminderIndicator extends View<{ bus: Bus }> {
  private readonly sub = this.props.bus.inbox<
    HUDSimvars & HudElems & PrimFgBusBaseEvents & FcuEfisCpBusEvents
  >();
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

  private readonly decMode = BusSignal.from(this.sub.topic('decMode'), 0);

  render(): ViewNode {
    return (
      <FlashOneHertz bus={this.props.bus} flashDuration={9} visible={this.lsReminderVisible}>
        <text class="FontLargest Green" x="905" y="800">
          LS
        </text>
      </FlashOneHertz>
    );
  }
}
