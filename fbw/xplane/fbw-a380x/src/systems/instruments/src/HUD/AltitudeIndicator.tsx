import {
  ClockTopics,
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  Signal,
  Readable,
  ViewNode,
} from 'xplane-js/ui';
import {
  Arinc429ConsumerSubject,
  Arinc429LocalVarConsumerSubject,
  Arinc429Register,
  Arinc429RegisterSubject,
  Arinc429Word,
  ArincEventBus,
} from '@flybywiresim/fbw-sdk';
import { HUDSimvars } from './shared/HUDSimvarPublisher';
import { DigitalAltitudeReadout } from './DigitalAltitudeReadout';
import { VerticalTape } from './VerticalTape';
import { Arinc429Values } from './shared/ArincValueProvider';
import { FmgcFlightPhase } from '@shared/flightphase';
import { getDisplayIndex } from './HUD';
import { ALT_TAPE_XPOS, ALT_TAPE_YPOS, XWIND_TO_AIR_REF_OFFSET } from './HUDUtils';
import { HudElems, WindMode, MdaMode } from './HUDUtils';
import { CrosswindDigitalAltitudeReadout } from './CrosswindDigitalAltitudeReadout';
import { FcuEfisCpBusEvents } from '@shared/publishers/EfisCpBusPublisher';
import { PrimFgBusBaseEvents } from '@shared/publishers/PrimFgPublisher';
import { FlashOneHertz } from '../MsfsAvionicsCommon/FlashingElementUtils';

let DisplayRange = 600;
const ValueSpacing = 100;
const DistanceSpacing = 33.3;

class LandingElevationIndicator extends View<{ bus: ArincEventBus }> {
  private readonly altitude = Arinc429ConsumerSubject.create(
    this.props.bus.getArincSubscriber<Arinc429Values>().on('altitudeAr'),
  );

  private landingElevationIndicator = UI.ref<SVGPathElement>();

  private crosswindMode = false;
  private xWindOffset = 0;
  private landingElevation = new Arinc429Word(0);

  private flightPhase = 0;

  private delta = 0;

  private handleLandingElevation() {
    const landingElevationValid =
      !this.landingElevation.isFailureWarning() && !this.landingElevation.isNoComputedData();
    const delta = this.altitude.get().value - this.landingElevation.value;
    const offset = ((delta - DisplayRange) * DistanceSpacing) / ValueSpacing + this.xWindOffset;
    this.delta = delta;
    if (delta > DisplayRange || (this.flightPhase !== 9 && this.flightPhase !== 10) || !landingElevationValid) {
      this.landingElevationIndicator.target.classList.add('HiddenElement');
    } else {
      this.landingElevationIndicator.target.classList.remove('HiddenElement');
    }
    this.landingElevationIndicator.target.setAttribute('d', `m586.862 554.167h-58.736v${offset}h58.736z`);
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<HUDSimvars & Arinc429Values & HudElems>();

    sub
      .topic('fwcFlightPhase')
      .distinct()
      .listen((fp) => {
        this.flightPhase = fp;

        if ((fp !== 9 && fp !== 10) || this.delta > DisplayRange) {
          this.landingElevationIndicator.target.classList.add('HiddenElement');
        } else {
          this.landingElevationIndicator.target.classList.remove('HiddenElement');
        }
      });

    sub
      .topic('cWndMode')
      .distinct()
      .listen((value) => {
        if (this.crosswindMode != value) {
          this.crosswindMode = value;
          if (this.crosswindMode) {
            DisplayRange = 200;
            this.xWindOffset = -150;
            this.landingElevationIndicator.target.style.transform = `translate3d(0px, ${XWIND_TO_AIR_REF_OFFSET}px, 0px)`;
          } else {
            DisplayRange = 600;
            this.xWindOffset = 0;
            this.landingElevationIndicator.target.style.transform = 'translate3d(0px, 0px, 0px)';
          }
        }
      });

    sub
      .topic('landingElevation')
      .distinct()
      .listen((le) => {
        this.landingElevation = le;
        this.handleLandingElevation();
      });

    this.altitude.watch(() => this.handleLandingElevation.bind(this), true);
  }

  render(): ViewNode {
    return <path ref={this.landingElevationIndicator} id="AltTapeLandingElevation" class="GreenFill2" />;
  }
}

class RadioAltIndicator extends View<{ bus: Bus; filteredRadioAltitude: Readable<number> }> {
  private altTapeGndRef = UI.ref<SVGPathElement>();
  private crosswindMode = false;
  private xWindOffset = 0;
  private hudMode = -1;
  private visibilitySub = Signal.of('hidden');

  private offsetSub = Signal.of('');

  private radioAltitude = new Arinc429Word(0);

  private setOffset() {
    if (
      this.props.filteredRadioAltitude.get() > DisplayRange ||
      this.radioAltitude.isFailureWarning() ||
      this.radioAltitude.isNoComputedData()
    ) {
      this.visibilitySub.set('hidden');
    } else {
      this.visibilitySub.set('visible');
      const offset = ((this.props.filteredRadioAltitude.get() - DisplayRange) * DistanceSpacing) / ValueSpacing;
      this.crosswindMode
        ? this.offsetSub.set(`m588.208 437 h12.876v${offset}h-12.876z`)
        : this.offsetSub.set(`m588.208 556 h12.876v${offset}h-12.876z`);
    }
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<Arinc429Values & HudElems>();

    this.props.filteredRadioAltitude.watch((_filteredRadioAltitude) => {
      this.setOffset();
    }, true);

    sub
      .topic('cWndMode')
      .distinct()
      .listen((value) => {
        this.crosswindMode = value;
        if (this.crosswindMode) {
          DisplayRange = 200;
          this.xWindOffset = -137;
        } else {
          DisplayRange = 600;
          this.xWindOffset = 0;
        }
        this.setOffset();
      });

    sub.topic('chosenRa').listen((ra) => {
      this.radioAltitude = ra;
      this.setOffset();
    });
  }

  render(): ViewNode {
    return <path visibility={this.visibilitySub} id="AltTapeGroundReference" class="BarGreen" d={this.offsetSub} />;
  }
}

class MinimumDescentAltitudeIndicator extends View<{ bus: ArincEventBus }> {
  private readonly sub = this.props.bus.inbox<FcuEfisCpBusEvents>();

  private readonly altitude = Arinc429ConsumerSubject.create(
    this.props.bus.getArincSubscriber<Arinc429Values>().on('altitudeAr'),
  );

  private visibility = Signal.of('hidden');

  private path = Signal.of('');

  private radioAltitudeValid = false;

  private qnhLandingAltValid = false;

  private qfeLandingAltValid = false;

  private inLandingPhases = false;

  private readonly fcuEisDiscreteWord2 = Arinc429LocalVarConsumerSubject.create(null);

  private readonly mda = Arinc429RegisterSubject.createEmpty();

  private landingElevation = new Arinc429Word(0);

  private updateIndication(): void {
    const isQnh = this.fcuEisDiscreteWord2.get().bitValueOr(12, false);
    const isQfe = !isQnh && !this.fcuEisDiscreteWord2.get().bitValueOr(11, true);

    this.qnhLandingAltValid =
      !this.landingElevation.isFailureWarning() &&
      !this.landingElevation.isNoComputedData() &&
      this.inLandingPhases &&
      isQnh;

    this.qfeLandingAltValid = this.inLandingPhases && isQfe;

    const altDelta = this.mda.get().value - this.altitude.get().value;

    const showMda =
      (this.radioAltitudeValid || this.qnhLandingAltValid || this.qfeLandingAltValid) &&
      Math.abs(altDelta) <= DisplayRange &&
      !this.mda.get().isFailureWarning() &&
      !this.mda.get().isNoComputedData();

    if (!showMda) {
      this.visibility.set('hidden');
      return;
    }

    const offset = (altDelta * DistanceSpacing) / ValueSpacing;
    this.path.set(`m 577 ,${360 - offset} h 23 v 5 h -23z`);
    this.visibility.set('visible');
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const isFo = getDisplayIndex() === 2;
    this.fcuEisDiscreteWord2.setConsumer(
      this.sub.topic(isFo ? 'fcu_efis_r_discrete_word_2' : 'fcu_efis_l_discrete_word_2'),
    );

    const sub = this.props.bus.getArincSubscriber<HUDSimvars & Arinc429Values>();
    sub
      .on('chosenRa')
      .whenArinc429SsmChanged()
      .listen((ra) => {
        this.radioAltitudeValid = !ra.isFailureWarning() && !ra.isNoComputedData();
        this.updateIndication();
      });

    sub
      .on('landingElevation')
      .withArinc429Precision(0)
      .listen((landingElevation) => {
        this.landingElevation = landingElevation;
        this.updateIndication();
      });

    this.altitude.watch(this.updateIndication.bind(this), true);

    this.mda.watch(this.updateIndication.bind(this));
    this.fcuEisDiscreteWord2.watch(this.updateIndication.bind(this));

    sub
      .on('fwcFlightPhase')
      .distinct()
      .listen((fp) => {
        this.inLandingPhases = fp === 9 || fp === 10;
        this.updateIndication();
      });

    sub.on('fmMdaRaw').listen(this.mda.setWord.bind(this.mda));
  }

  render(): ViewNode {
    return <path visibility={this.visibility} id="AltTapeMdaIndicator" class="Fill Green" d={this.path} />;
  }
}

interface AltitudeIndicatorProps {
  bus: ArincEventBus;
}

export class AltitudeIndicator extends View<AltitudeIndicatorProps> {
  private crosswindMode = false;
  private subscribable = Signal.of<number>(0);
  private tapeRef = UI.ref<HTMLDivElement>();
  private altIndicatorGroupRef = UI.ref<SVGGElement>();

  private readonly altitude = Arinc429ConsumerSubject.create(
    this.props.bus.getArincSubscriber<Arinc429Values>().on('altitudeAr'),
  );
  private sub = this.props.bus.inbox<HudElems>();
  private readonly altTape = BusSignal.from(this.sub.topic('altTape').distinct(), '');
  private readonly xWindAltTape = BusSignal.from(this.sub.topic('xWindAltTape').distinct(), '');
  private isBothAltTapesOff = Derived.from(
    ([altTape, xWindAltTape]) => {
      return altTape === 'none' && xWindAltTape === 'none' ? 'none' : 'block';
    },
    this.altTape,
    this.xWindAltTape,
  );
  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.altIndicatorGroupRef.target.style.transform = `translate3d(${ALT_TAPE_XPOS}px, ${ALT_TAPE_YPOS}px, 0px)`;

    this.sub
      .topic('cWndMode')
      .distinct()
      .listen((value) => {
        this.crosswindMode = value;
        if (this.crosswindMode) {
          DisplayRange = 200;
          this.tapeRef.target.style.transform = `translate3d(0px, ${XWIND_TO_AIR_REF_OFFSET}px, 0px)`;
        } else {
          DisplayRange = 600;
          this.tapeRef.target.style.transform = 'translate3d(0px, 0px, 0px)';
        }
      });
  }

  render(): ViewNode {
    return (
      <g id="AltitudeTape" ref={this.altIndicatorGroupRef} display={this.isBothAltTapesOff}>
        <LandingElevationIndicator bus={this.props.bus} />
        <g
          ref={this.tapeRef}
          transform="translate3d(0px, 0px, 0px)"
          style={{
            display: this.altitude.derive((v) => (v.isNormalOperation() || v.isFunctionalTest() ? 'inline' : 'none')),
          }}
        >
          <VerticalTape
            displayRange={DisplayRange + 30}
            valueSpacing={ValueSpacing}
            distanceSpacing={DistanceSpacing}
            lowerLimit={-1500}
            upperLimit={50000}
            tapeValue={this.altitude.derive((v) => v.value)}
            type="altitude"
          />
        </g>
      </g>
    );
  }
}

interface AltitudeIndicatorOfftapeProps {
  bus: ArincEventBus;
  filteredRadioAltitude: Readable<number>;
}

export class AltitudeIndicatorOfftape extends View<AltitudeIndicatorOfftapeProps> {
  private altTape = 'block';
  private xWindAltTape = 'none';

  private altTapeRef = UI.ref<SVGGElement>();
  private xWindAltTapeRef = UI.ref<SVGGElement>();

  private readonly sub = this.props.bus.inbox<PrimFgBusBaseEvents & Arinc429Values>();
  private readonly altitude = Arinc429ConsumerSubject.create(this.sub.topic('altitudeAr'));

  private readonly altFlagVisible = this.altitude.derive((v) => !v.isNormalOperation() && !v.isFunctionalTest());

  private readonly selectedAlt = Arinc429LocalVarConsumerSubject.create(this.sub.topic('prim_selected_altitude'));

  private readonly altConstraint = Arinc429LocalVarConsumerSubject.create(this.sub.topic('prim_fm_alt_constraint'));

  private readonly fgDiscreteWord1 = Arinc429LocalVarConsumerSubject.create(this.sub.topic('prim_fg_discrete_word_1'));

  private readonly fgDiscreteWord3 = Arinc429LocalVarConsumerSubject.create(this.sub.topic('prim_fg_discrete_word_3'));

  private readonly selectedAltIgnored = Derived.from(
    ([fgDiscreteWord1, fgDiscreteWord3]) => {
      const landTrackActive = fgDiscreteWord1.bitValueOr(23, false);
      const gsCaptActive = fgDiscreteWord3.bitValueOr(21, false);
      const gsTrkActive = fgDiscreteWord3.bitValueOr(22, false);
      const finalDesActive = fgDiscreteWord3.bitValueOr(23, false);

      return landTrackActive || gsCaptActive || gsTrkActive || finalDesActive;
    },
    this.fgDiscreteWord1,
    this.fgDiscreteWord3,
  );

  private readonly useAltConstraint = Derived.from(
    ([altConstraint, selectedAltIgnored]) =>
      !(selectedAltIgnored || altConstraint.isFailureWarning() || altConstraint.isNoComputedData()),
    this.altConstraint,
    this.selectedAltIgnored,
  );

  private readonly shownTargetAltitude = Arinc429RegisterSubject.createEmpty();

  private readonly selectedAltPipe = this.selectedAlt.forward(this.shownTargetAltitude, false);
  private readonly altConstraintPipe = this.altConstraint.forward(this.shownTargetAltitude, true);

  private tcasFailed = UI.ref<SVGGElement>();

  private altIndicatorOffTapeGroupRef = UI.ref<SVGGElement>();
  private doOnce = 0;
  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.useAltConstraint.watch((useAltConstraint) => {
      if (useAltConstraint) {
        this.selectedAltPipe.hold();
        this.altConstraintPipe.release(true);
      } else {
        this.selectedAltPipe.release(true);
        this.altConstraintPipe.hold();
      }
    }, true);

    const sub = this.props.bus.inbox<HUDSimvars & Arinc429Values & ClockTopics & HudElems>();
    sub
      .topic('altTape')
      .distinct()
      .listen((v) => {
        this.altTape = v;
        this.altTapeRef.target.style.display = `${this.altTape}`;
      });
    sub
      .topic('xWindAltTape')
      .distinct()
      .listen((v) => {
        this.xWindAltTape = v;
        this.xWindAltTapeRef.target.style.display = `${this.xWindAltTape}`;
        this.xWindAltTapeRef.target.style.transform = `translate3d(0px, ${XWIND_TO_AIR_REF_OFFSET}px, 0px)`;
      });

    sub
      .topic('tcasFail')
      .distinct()
      .listen((tcasFailed) => {
        if (tcasFailed) {
          this.tcasFailed.target.style.display = 'inline';
        } else {
          this.tcasFailed.target.style.display = 'none';
        }
      });
    this.altIndicatorOffTapeGroupRef.target.style.transform = `translate3d(${ALT_TAPE_XPOS}px, ${ALT_TAPE_YPOS}px, 0px)`;
  }

  render(): ViewNode {
    return (
      <g id="altOfftapeGroup" ref={this.altIndicatorOffTapeGroupRef}>
        <g style={{ display: this.altFlagVisible.derive((v) => (v ? 'inherit' : 'none')) }}>
          <path id="AltTapeOutlineVert" class="NormalStroke Green" d="m587 170.834v383" />
          <path id="AltTapeOutlineUpper" class="NormalStroke Green" d="m528 170.834h100" />
          <path id="AltTapeOutlineLower" class="NormalStroke Green" d="m528 554.167h100" />
          <path
            id="AltReadoutBackground"
            class="BlackFill"
            d="m587 382.606h-58.888v-40.233h58.888v-11.979h39.758v64.194h-39.758z"
          />
          <text id="AltFailText" class="Blink9Seconds FontLargest Green MiddleAlign" x="588" y="374">
            ALT
          </text>
        </g>
        <g ref={this.tcasFailed} style="display: none">
          <text class="Blink9Seconds FontLargest Green MiddleAlign" x="630" y="420">
            T
          </text>
          <text class="Blink9Seconds FontLargest Green MiddleAlign" x="630" y="455">
            C
          </text>
          <text class="Blink9Seconds FontLargest Green MiddleAlign" x="630" y="490">
            A
          </text>
          <text class="Blink9Seconds FontLargest Green MiddleAlign" x="630" y="525">
            S
          </text>
        </g>

        <g id="NormalOfftapeGroup" style={{ display: this.altFlagVisible.derive((v) => (v ? 'none' : 'inherit')) }}>
          <g id="CrosswindAltTape" class="cwTest" transform="translate3d(0px, -311px, 0px)" ref={this.xWindAltTapeRef}>
            <path id="AltTapeOutlineVert" class="NormalStroke Green" d="m587 288.5 v 150" />
            <path id="AltTapeOutlineUpper" class="NormalStroke Green" d="m528 288.5 h100" />
            <path id="AltTapeOutlineLower" class="NormalStroke Green" d="m528 438.5 h100" />
            <MinimumDescentAltitudeIndicator bus={this.props.bus} />
            <SelectedAltIndicator bus={this.props.bus} mode={WindMode.CrossWind} targetAlt={this.shownTargetAltitude} />
            <path id="AltReadoutBackground" class="BlackFill" d="m528 341.25 h 100 v45 h -100z" />
            <RadioAltIndicator bus={this.props.bus} filteredRadioAltitude={this.props.filteredRadioAltitude} />
            <CrosswindDigitalAltitudeReadout bus={this.props.bus} />
          </g>

          <g id="NormalAltTape" ref={this.altTapeRef}>
            <path id="AltTapeOutlineVert" class="NormalStroke Green" d="m587 170.834v383" />
            <path id="AltTapeOutlineUpper" class="NormalStroke Green" d="m528 170.834h100" />
            <path id="AltTapeOutlineLower" class="NormalStroke Green" d="m528 554.167h100" />
            <MinimumDescentAltitudeIndicator bus={this.props.bus} />
            <SelectedAltIndicator bus={this.props.bus} mode={WindMode.Normal} targetAlt={this.shownTargetAltitude} />
            <path
              id="AltReadoutBackground"
              class="BlackFill"
              d="m587 382.606h-58.888v-40.233h58.888v-11.979h39.758v64.194h-39.758z"
            />
            <RadioAltIndicator bus={this.props.bus} filteredRadioAltitude={this.props.filteredRadioAltitude} />
            <DigitalAltitudeReadout bus={this.props.bus} />
          </g>
        </g>
        <AltimeterIndicator bus={this.props.bus} altitude={this.altitude.derive((v) => v.value)} />
        <MetricAltIndicator bus={this.props.bus} targetAlt={this.shownTargetAltitude} />
      </g>
    );
  }
}

interface SelectedAltIndicatorProps {
  bus: ArincEventBus;
  mode: WindMode;
  targetAlt: Arinc429RegisterSubject;
}

class SelectedAltIndicator extends View<SelectedAltIndicatorProps> {
  private readonly sub = this.props.bus.inbox<
    FcuEfisCpBusEvents & PrimFgBusBaseEvents & HUDSimvars & Arinc429Values
  >();

  private readonly fcuEisDiscreteWord2 = Arinc429LocalVarConsumerSubject.create(null);

  private selectedAltLowerGroupRef = UI.ref<SVGGElement>();

  private selectedAltLowerText = UI.ref<SVGTextElement>();

  private selectedAltLowerFLText = UI.ref<SVGTextElement>();

  private selectedAltUpperGroupRef = UI.ref<SVGGElement>();

  private readonly selectedAltFailed = Signal.of(false);

  private selectedAltUpperText = UI.ref<SVGTextElement>();

  private selectedAltUpperFLText = UI.ref<SVGTextElement>();

  private targetGroupRef = UI.ref<SVGGElement>();

  private blackFill = UI.ref<SVGPathElement>();

  private targetSymbolRef = UI.ref<SVGPathElement>();

  private altTapeTargetText = UI.ref<SVGTextElement>();

  private minimumLowerText = UI.ref<SVGTextElement>();

  private shownTargetAltitude = Arinc429Register.empty();

  private textSub = Signal.of('');

  private readonly fmEisDiscrete2 = Arinc429RegisterSubject.createEmpty();
  private readonly hudMode = BusSignal.from(this.sub.topic('hudMode').distinct(), 0);
  private readonly altitude = Arinc429ConsumerSubject.create(this.sub.topic('altitudeAr'));
  private readonly ra = Arinc429ConsumerSubject.create(this.sub.topic('chosenRa').distinct());
  private readonly mda = Arinc429RegisterSubject.createEmpty();
  private readonly dh = Arinc429RegisterSubject.createEmpty();
  private readonly noDhSelected = this.fmEisDiscrete2.derive((r) => r.bitValueOr(29, false));

  private readonly mdaDhMode = Derived.from(
    ([noDh, dh, mda]) => {
      if (noDh) {
        return MdaMode.NoDh;
      }

      if (!dh.isNoComputedData() && !dh.isFailureWarning()) {
        return MdaMode.Radio;
      }

      if (!mda.isNoComputedData() && !mda.isFailureWarning()) {
        return MdaMode.Baro;
      }

      return MdaMode.None;
    },
    this.noDhSelected,
    this.dh,
    this.mda,
  );

  private readonly isMinLowerAltTxtVisible = Derived.from(
    ([mda, dh, mdaDhMode, altitude, ra, hudMode]) => {
      let diff;
      if (hudMode === 0) {
        switch (mdaDhMode) {
          case MdaMode.Baro:
            diff = altitude.value - mda.value;
            break;
          case MdaMode.Radio:
            diff = ra.value - dh.value;
            break;
          case MdaMode.NoDh:
            diff = ra.value;
            break;
          default:
            diff = 0;
            break;
        }

        if (mdaDhMode === MdaMode.NoDh || mdaDhMode === MdaMode.None) {
          return 'none';
        } else {
          return diff <= 0 ? 'block' : 'none';
        }
      } else {
        return 'none';
      }
    },
    this.mda,
    this.dh,
    this.mdaDhMode,
    this.altitude,
    this.ra,
    this.hudMode,
  );

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const isFo = getDisplayIndex() === 2;
    this.fcuEisDiscreteWord2.setConsumer(
      this.sub.topic(isFo ? 'fcu_efis_r_discrete_word_2' : 'fcu_efis_l_discrete_word_2'),
    );

    this.props.targetAlt.watch((targetAlt) => {
      this.shownTargetAltitude.set(targetAlt.rawWord);

      this.getOffset();
      this.handleAltitudeDisplay();
      this.setText();
    }, true);

    this.sub.topic('fmMdaRaw').listen(this.mda.setWord.bind(this.mda));
    this.sub.topic('fmDhRaw').listen(this.dh.setWord.bind(this.dh));

    this.sub
      .topic('fmgcFlightPhase')
      .distinct()
      .listen((value) => {
        value === FmgcFlightPhase.Approach
          ? this.targetSymbolRef.target.setAttribute('stroke-dasharray', '3 6')
          : this.targetSymbolRef.target.setAttribute('stroke-dasharray', '');
      });

    this.altitude.watch(() => {
      this.handleAltitudeDisplay();
      this.getOffset();
      this.handleCrosswinMode();
    }, true);

    this.fcuEisDiscreteWord2.watch(() => {
      const isStd = this.fcuEisDiscreteWord2.get().bitValueOr(11, true);

      if (isStd) {
        this.selectedAltLowerFLText.target.style.visibility = 'visible';
        this.selectedAltUpperFLText.target.style.visibility = 'visible';
      } else {
        this.selectedAltLowerFLText.target.style.visibility = 'hidden';
        this.selectedAltUpperFLText.target.style.visibility = 'hidden';
      }

      this.handleAltitudeDisplay();
      this.setText();
    }, true);
  }

  private handleAltitudeDisplay() {
    if (this.shownTargetAltitude.isNoComputedData() || this.shownTargetAltitude.isFailureWarning()) {
      this.selectedAltUpperGroupRef.target.style.display = 'none';
      this.selectedAltLowerGroupRef.target.style.display = 'none';
      this.targetGroupRef.target.style.display = 'none';
      this.selectedAltFailed.set(true);
    } else if (this.altitude.get().value - this.shownTargetAltitude.value > DisplayRange) {
      this.selectedAltLowerGroupRef.target.style.display = 'block';
      this.selectedAltUpperGroupRef.target.style.display = 'none';
      this.targetGroupRef.target.style.display = 'none';
      this.selectedAltFailed.set(false);
    } else if (this.altitude.get().value - this.shownTargetAltitude.value < -DisplayRange) {
      this.targetGroupRef.target.style.display = 'none';
      this.selectedAltUpperGroupRef.target.style.display = 'block';
      this.selectedAltLowerGroupRef.target.style.display = 'none';
      this.selectedAltFailed.set(false);
    } else {
      this.selectedAltUpperGroupRef.target.style.display = 'none';
      this.selectedAltLowerGroupRef.target.style.display = 'none';
      this.targetGroupRef.target.style.display = 'inline';
      this.selectedAltFailed.set(false);
    }
  }

  private handleCrosswinMode() {
    if (this.props.mode === WindMode.Normal) {
      this.minimumLowerText.target.setAttribute('y', '580');
      this.selectedAltLowerText.target.setAttribute('y', '580');
      this.selectedAltLowerFLText.target.setAttribute('y', '580');
      this.selectedAltUpperText.target.setAttribute('y', '167');
      this.selectedAltUpperFLText.target.setAttribute('y', '167');
    } else {
      this.minimumLowerText.target.setAttribute('y', '465');
      this.selectedAltLowerText.target.setAttribute('y', '465');
      this.selectedAltLowerFLText.target.setAttribute('y', '465');
      this.selectedAltUpperText.target.setAttribute('y', '280');
      this.selectedAltUpperFLText.target.setAttribute('y', '280');
    }
  }

  private setText() {
    let boxLength = 95.7;
    let text = '0';
    const isStd = this.fcuEisDiscreteWord2.get().bitValueOr(11, true);
    if (isStd) {
      text = Math.round(this.shownTargetAltitude.value / 100)
        .toString()
        .padStart(3, '0');
      boxLength = 62.5;
    } else {
      text = Math.round(this.shownTargetAltitude.value).toString().padStart(5, ' ');
    }
    this.textSub.set(text);
    this.blackFill.target.setAttribute('d', `m 528.109 348.861 h ${boxLength}v 27.123 h-${boxLength} z`);
  }

  private getOffset() {
    const offset = ((this.altitude.get().value - this.shownTargetAltitude.value) * DistanceSpacing) / ValueSpacing;
    this.targetGroupRef.target.style.transform = `translate3d(0px, ${offset}px, 0px)`;
  }

  render(): ViewNode | null {
    return (
      <>
        <text
          id="MinimumLowerText"
          ref={this.minimumLowerText}
          display={this.isMinLowerAltTxtVisible}
          class="FontMedium EndAlign Green"
          x="634"
          y="580"
          style="white-space: pre"
        >
          MINIMUM
        </text>
        <g id="SelectedAltLowerGroup" ref={this.selectedAltLowerGroupRef}>
          <text
            id="SelectedAltLowerText"
            ref={this.selectedAltLowerText}
            class="FontMedium EndAlign Green"
            x="618"
            y="580"
            style="white-space: pre"
          >
            {this.textSub}
          </text>
          <text
            id="SelectedAltLowerFLText"
            ref={this.selectedAltLowerFLText}
            class="FontSmall MiddleAlign Green"
            x="542"
            y="580"
          >
            FL
          </text>
        </g>
        <g id="SelectedAltUpperGroup" ref={this.selectedAltUpperGroupRef}>
          <text
            id="SelectedAltUpperText"
            ref={this.selectedAltUpperText}
            class="FontMedium EndAlign Green"
            x="620"
            y="167"
            style="white-space: pre"
          >
            {this.textSub}
          </text>
          <text
            id="SelectedAltUpperFLText"
            ref={this.selectedAltUpperFLText}
            class="FontSmall MiddleAlign Green"
            x="542"
            y="167"
          >
            FL
          </text>
        </g>
        <g id="AltTapeTargetSymbol" ref={this.targetGroupRef}>
          <path class="BlackFill" ref={this.blackFill} />
          <path
            class="NormalStroke Green"
            ref={this.targetSymbolRef}
            d="m550.713 375.982  v 29.384 h -31.626 v -38.426  l 9.035 -4.521 l -9.035 -4.521 v -38.426  h 31 v  29.384"
          />
          <text
            id="AltTapeTargetText"
            ref={this.altTapeTargetText}
            class="FontMedium StartAlign Green"
            x="530.25258"
            y="372.55577307"
            style="white-space: pre"
          >
            {this.textSub}
          </text>
          <FlashOneHertz bus={this.props.bus} flashDuration={9} visible={this.selectedAltFailed}>
            <text id="SelectedAltUpperText" class="FontSmall EndAlign Red" x="620" y="167">
              ALT SEL
            </text>
          </FlashOneHertz>
        </g>
      </>
    );
  }
}

interface AltimeterIndicatorProps {
  altitude: Readable<number>;
  bus: ArincEventBus;
}

class AltimeterIndicator extends View<AltimeterIndicatorProps> {
  private QFERef = UI.ref<SVGGElement>();
  private readonly sub = this.props.bus.inbox<
    HUDSimvars & FcuEfisCpBusEvents & Arinc429Values & HudElems & ClockTopics
  >();

  private readonly fcuEisDiscreteWord1 = Arinc429LocalVarConsumerSubject.create(null);

  private readonly fcuEisDiscreteWord2 = Arinc429LocalVarConsumerSubject.create(null);

  private readonly fcuBaroCorrectionHg = Arinc429LocalVarConsumerSubject.create(null);

  private readonly fcuBaroCorrectionHpa = Arinc429LocalVarConsumerSubject.create(null);

  private readonly isHg = this.fcuEisDiscreteWord2.derive((v) => v.bitValueOr(13, false));

  private readonly text = Signal.of('');

  private readonly hgTextPipe = this.fcuBaroCorrectionHg.forward(
    this.text,
    (v) => (v.isNormalOperation() || v.isFunctionalTest() ? v.value.toFixed(2) : ''),
    true,
  );

  private readonly hpaTextPipe = this.fcuBaroCorrectionHpa.forward(
    this.text,
    (v) => (v.isNormalOperation() || v.isFunctionalTest() ? v.value.toFixed(0) : ''),
    true,
  );

  private mode = Signal.of('');

  private transAltAr = Arinc429Register.empty();

  private transLvlAr = Arinc429Register.empty();

  private fmgcFlightPhase = 0;

  private stdGroup = UI.ref<SVGGElement>();

  private qfeGroup = UI.ref<SVGGElement>();

  private qfeBorder = UI.ref<SVGGElement>();
  private needsUpdate = false;

  afterRender(node: ViewNode): void {
    super.afterRender(node);
    this.sub
      .topic('QFE')
      .distinct()
      .listen((v) => {
        this.QFERef.target.style.display = v;
        this.needsUpdate = true;
      });

    const isFo = getDisplayIndex() === 2;

    this.fcuEisDiscreteWord2.setConsumer(
      this.sub.topic(isFo ? 'fcu_efis_r_discrete_word_2' : 'fcu_efis_l_discrete_word_2'),
    );
    this.fcuBaroCorrectionHg.setConsumer(
      this.sub.topic(isFo ? 'fcu_efis_r_baro_setting_inhg' : 'fcu_efis_l_baro_setting_inhg'),
    );
    this.fcuBaroCorrectionHpa.setConsumer(this.sub.topic(isFo ? 'fcu_efis_r_baro_setting' : 'fcu_efis_l_baro_setting'));

    const sub = this.props.bus.getArincSubscriber<HUDSimvars & Arinc429Values>();

    this.fcuEisDiscreteWord2.watch(() => {
      const isQnh = this.fcuEisDiscreteWord2.get().bitValueOr(12, false);
      const isStd = this.fcuEisDiscreteWord2.get().bitValueOr(11, true);
      const isQfe = !isQnh && !isStd;

      if (isQfe) {
        this.mode.set('QFE');
        this.stdGroup.target.classList.add('HiddenElement');
        this.qfeGroup.target.classList.remove('HiddenElement');
        this.qfeBorder.target.classList.remove('HiddenElement');
      } else if (isQnh) {
        this.mode.set('QNH');
        this.stdGroup.target.classList.add('HiddenElement');
        this.qfeGroup.target.classList.remove('HiddenElement');
        this.qfeBorder.target.classList.add('HiddenElement');
      } else if (isStd) {
        this.mode.set('STD');
        this.stdGroup.target.classList.remove('HiddenElement');
        this.qfeGroup.target.classList.add('HiddenElement');
        this.qfeBorder.target.classList.add('HiddenElement');
      }
    }, true);

    sub
      .on('fmgcFlightPhase')
      .distinct()
      .listen((fp) => {
        this.fmgcFlightPhase = fp;

        this.handleBlink();
      });

    sub
      .on('fmTransAltRaw')
      .distinct()
      .listen((ta) => {
        this.transAltAr.set(ta);

        this.handleBlink();
      });

    sub
      .on('fmTransLvlRaw')
      .distinct()
      .listen((tl) => {
        this.transLvlAr.set(tl);

        this.handleBlink();
      });

    this.isHg.watch((isHg) => {
      if (isHg) {
        this.hpaTextPipe.hold();
        this.hgTextPipe.release(true);
      } else {
        this.hgTextPipe.hold();
        this.hpaTextPipe.release(true);
      }
    }, true);

    this.props.altitude.watch((_a) => {
      this.handleBlink();
    });
  }

  private handleBlink() {
    if (this.mode.get() === 'STD') {
      if (
        this.fmgcFlightPhase > FmgcFlightPhase.Cruise &&
        this.transLvlAr.isNormalOperation() &&
        100 * this.transLvlAr.value > this.props.altitude.get()
      ) {
        this.stdGroup.target.classList.add('BlinkInfinite');
      } else {
        this.stdGroup.target.classList.remove('BlinkInfinite');
      }
    } else if (
      this.fmgcFlightPhase <= FmgcFlightPhase.Cruise &&
      this.transAltAr.isNormalOperation() &&
      this.transAltAr.value < this.props.altitude.get()
    ) {
      this.qfeGroup.target.classList.add('BlinkInfinite');
    } else {
      this.qfeGroup.target.classList.remove('BlinkInfinite');
    }
  }

  render(): ViewNode {
    return (
      <>
        <g id="BaroModeGroup" ref={this.QFERef}>
          <g ref={this.stdGroup} id="STDAltimeterModeGroup">
            <path class="NormalStroke Green" d="m525 590 h 63 v 31 h -63 z" />
            <text class="FontMedium Green AlignLeft" x="530" y="616">
              STD
            </text>
          </g>
          <g id="AltimeterGroup">
            <g ref={this.qfeGroup} id="QFEGroup">
              <path ref={this.qfeBorder} class="NormalStroke Green" d="m524.013 596.805h62.513v26.431h-62.513z" />
              <text id="AltimeterModeText" class="FontMedium Green" x="530.26" y="619.4386887">
                {this.mode}
              </text>
              <text id="AltimeterSettingText" class="FontMedium StartAlign Green" x="587.535" y="619.3339191">
                {this.text}
              </text>
            </g>
          </g>
        </g>
      </>
    );
  }
}

interface MetricAltIndicatorProps {
  bus: ArincEventBus;
  targetAlt: Arinc429RegisterSubject;
}

class MetricAltIndicator extends View<MetricAltIndicatorProps> {
  private sub = this.props.bus.inbox<PrimFgBusBaseEvents & Arinc429Values & HUDSimvars>();
  private primFgDiscreteWord5 = Arinc429LocalVarConsumerSubject.create(this.sub.topic('prim_fg_discrete_word_5'));

  private readonly metricAltActive = this.primFgDiscreteWord5.derive((word) => word.bitValueOr(14, false));

  private readonly altitude = Arinc429ConsumerSubject.create(this.sub.topic('altitudeAr'));

  private currentMetricAlt = this.altitude.derive((alt) => Math.round((alt.value * 0.3048) / 10) * 10);

  private metricTargetAlt = this.props.targetAlt.derive((alt) => Math.round((alt.value * 0.3048) / 10) * 10);

  private readonly mda = BusSignal.from(this.sub.topic('mda'), 0);

  private readonly belowMda = Derived.from(([mda, altitude]) => altitude.value < mda, this.mda, this.altitude);
  private metricAltText = UI.ref<SVGTextElement>();

  private metricAltHudVis = false;
  private metricAltTargetText = UI.ref<SVGTextElement>();
  private metricAltTargetMText = UI.ref<SVGTextElement>();

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<HUDSimvars & Arinc429Values & ClockTopics & HudElems>();
    sub
      .topic('cWndMode')
      .distinct()
      .listen((v) => {
        if (v) {
          this.metricAltTargetText.target.setAttribute('x', '557');
          this.metricAltTargetMText.target.setAttribute('x', '608');
          this.metricAltTargetText.target.setAttribute('y', '60');
          this.metricAltTargetMText.target.setAttribute('y', '60');
        } else {
          this.metricAltTargetText.target.setAttribute('x', '457');
          this.metricAltTargetMText.target.setAttribute('x', '498');
          this.metricAltTargetText.target.setAttribute('y', '170');
          this.metricAltTargetMText.target.setAttribute('y', '170');
        }
      });
    sub
      .topic('metricAlt')
      .distinct()
      .listen((v) => {
        v ? (this.metricAltHudVis = true) : (this.metricAltHudVis = false);
      });
  }

  render(): ViewNode {
    return (
      <g id="MetricAltGroup" visibility={this.metricAltActive.derive((active) => (active ? 'inherit' : 'hidden'))}>
        <path class="NormalStroke Green" d="m522.772 628.887h131.02v31.644h-131.02z" />
        <text class="FontMedium Green MiddleAlign" x="637.0286344499999" y="654.2220165">
          M
        </text>
        <text
          ref={this.metricAltText}
          id="MetricAltText"
          class={{
            FontMedium: true,
            MiddleAlign: true,
            Green: true,
          }}
          x="576.9821538"
          y="654.19066635"
        >
          {this.currentMetricAlt}
        </text>
        <g id="MetricAltTargetGroup">
          <text
            id="MetricAltTargetText"
            ref={this.metricAltTargetText}
            class="FontSmallest Green MiddleAlign"
            x="457.47"
            y="170.100877245"
          >
            {this.metricTargetAlt}
          </text>
          <text
            ref={this.metricAltTargetMText}
            class="FontSmallest Green MiddleAlign"
            x="497.83500000000004"
            y="169.860050685"
          >
            M
          </text>
        </g>
      </g>
    );
  }
}
