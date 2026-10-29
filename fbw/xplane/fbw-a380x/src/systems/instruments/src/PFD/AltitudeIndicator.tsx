// @ts-strict-ignore
import {
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  Signal,
  Readable,
  Logic,
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
import { PFDSimvars } from './shared/PFDSimvarPublisher';
import { DigitalAltitudeReadout } from './DigitalAltitudeReadout';
import { VerticalTape } from './VerticalTape';
import { Arinc429Values } from './shared/ArincValueProvider';
import { FmgcFlightPhase } from '@shared/flightphase';
import { FcuEfisCpBusEvents } from '@shared/publishers/EfisCpBusPublisher';
import { getDisplayIndex } from './PFD';
import { PrimFgBusBaseEvents } from '@shared/publishers/PrimFgPublisher';
import { FlashOneHertz } from '../MsfsAvionicsCommon/FlashingElementUtils';

const DisplayRange = 600;
const ValueSpacing = 100;
const DistanceSpacing = 7.5;

class LandingElevationIndicator extends View<{ bus: ArincEventBus }> {
  private readonly altitude = Arinc429ConsumerSubject.create(
    this.props.bus.getArincSubscriber<Arinc429Values>().on('altitudeAr'),
  );

  private landingElevationIndicator = UI.ref<SVGPathElement>();

  private landingElevation = new Arinc429Word(0);

  private flightPhase = 0;

  private delta = 0;

  private handleLandingElevation() {
    const landingElevationValid =
      !this.landingElevation.isFailureWarning() && !this.landingElevation.isNoComputedData();
    const delta = this.altitude.get().value - this.landingElevation.value;
    const offset = ((delta - DisplayRange) * DistanceSpacing) / ValueSpacing;
    this.delta = delta;
    if (delta > DisplayRange || (this.flightPhase !== 9 && this.flightPhase !== 10) || !landingElevationValid) {
      this.landingElevationIndicator.target.classList.add('HiddenElement');
    } else {
      this.landingElevationIndicator.target.classList.remove('HiddenElement');
    }
    this.landingElevationIndicator.target.setAttribute('d', `m130.85 123.56h-13.096v${offset}h13.096z`);
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<PFDSimvars & Arinc429Values>();

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
      .topic('landingElevation')
      .distinct()
      .listen((le) => {
        this.landingElevation = le;
        this.handleLandingElevation();
      });

    this.altitude.watch(() => this.handleLandingElevation.bind(this), true);
  }

  render(): ViewNode {
    return <path ref={this.landingElevationIndicator} id="AltTapeLandingElevation" class="EarthFill" />;
  }
}

class RadioAltIndicator extends View<{ bus: Bus; filteredRadioAltitude: Readable<number> }> {
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
      this.offsetSub.set(`m131.15 123.56h2.8709v${offset}h-2.8709z`);
    }
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<Arinc429Values>();

    this.props.filteredRadioAltitude.watch((_filteredRadioAltitude) => {
      this.setOffset();
    }, true);

    sub.topic('chosenRa').listen((ra) => {
      this.radioAltitude = ra;
      this.setOffset();
    });
  }

  render(): ViewNode {
    return <path visibility={this.visibilitySub} id="AltTapeGroundReference" class="Fill Red" d={this.offsetSub} />;
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
      Math.abs(altDelta) <= 570 &&
      !this.mda.get().isFailureWarning() &&
      !this.mda.get().isNoComputedData();

    if (!showMda) {
      this.visibility.set('hidden');
      return;
    }

    const offset = (altDelta * DistanceSpacing) / ValueSpacing;
    this.path.set(`m 127.9276,${80.249604 - offset} h 5.80948 v 1.124908 h -5.80948 z`);
    this.visibility.set('visible');
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const isFo = getDisplayIndex() === 2;
    this.fcuEisDiscreteWord2.setConsumer(
      this.sub.topic(isFo ? 'fcu_efis_r_discrete_word_2' : 'fcu_efis_l_discrete_word_2'),
    );

    const sub = this.props.bus.getArincSubscriber<PFDSimvars & Arinc429Values>();

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
    return <path visibility={this.visibility} id="AltTapeMdaIndicator" class="Fill Amber" d={this.path} />;
  }
}

interface AltitudeIndicatorProps {
  bus: ArincEventBus;
}

export class AltitudeIndicator extends View<AltitudeIndicatorProps> {
  private readonly altitude = Arinc429ConsumerSubject.create(
    this.props.bus.getArincSubscriber<Arinc429Values>().on('altitudeAr'),
  );

  render(): ViewNode {
    return (
      <g id="AltitudeTape">
        <AltTapeBackground />
        <LandingElevationIndicator bus={this.props.bus} />
        <g
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

class AltTapeBackground extends View<any> {
  render(): ViewNode {
    return <path id="AltTapeBackground" d="m130.85 123.56h-13.096v-85.473h13.096z" class="TapeBackground" />;
  }
}

interface AltitudeIndicatorOfftapeProps {
  bus: ArincEventBus;
  filteredRadioAltitude: Readable<number>;
}

enum TargetAltitudeColor {
  Cyan,
  Magenta,
  White,
}

export class AltitudeIndicatorOfftape extends View<AltitudeIndicatorOfftapeProps> {
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

  private readonly selectedAltColor = Derived.from(
    ([useAltConstraint, selectedAltIgnored]) => {
      if (selectedAltIgnored) {
        return TargetAltitudeColor.White;
      } else if (useAltConstraint) {
        return TargetAltitudeColor.Magenta;
      } else {
        return TargetAltitudeColor.Cyan;
      }
    },
    this.useAltConstraint,
    this.selectedAltIgnored,
  );

  private tcasFailed = UI.ref<SVGGElement>();

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

    const sub = this.props.bus.inbox<PFDSimvars>();

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
  }

  render(): ViewNode {
    return (
      <>
        <g style={{ display: this.altFlagVisible.derive((v) => (v ? 'inherit' : 'none')) }}>
          <path id="AltTapeOutlineUpper" class="NormalStroke Red" d="m 117.75,38.09 h 13.10 6.73" />
          <path id="AltTapeOutlineLower" class="NormalStroke Red" d="m 117.75,123.56 h 13.10 6.73" />
          <path id="AltReadoutBackground" class="BlackFill" d="m131.35 85.308h-13.63v-8.9706h13.63z" />
          <text id="AltFailText" class="Blink9Seconds FontLargest Red EndAlign" x="131.16769" y="83.433167">
            ALT
          </text>
        </g>
        <g ref={this.tcasFailed} style="display: none">
          <text class="Blink9Seconds FontLargest Amber EndAlign" x="141.5" y="96">
            T
          </text>
          <text class="Blink9Seconds FontLargest Amber EndAlign" x="141.5" y="104">
            C
          </text>
          <text class="Blink9Seconds FontLargest Amber EndAlign" x="141.5" y="112">
            A
          </text>
          <text class="Blink9Seconds FontLargest Amber EndAlign" x="141.5" y="120">
            S
          </text>
        </g>
        <g style={{ display: this.altFlagVisible.derive((v) => (v ? 'none' : 'inherit')) }}>
          <path id="AltTapeOutlineUpper" class="NormalStroke White" d="m 117.75,38.09 h 13.10 6.73" />
          <path id="AltTapeOutlineLower" class="NormalStroke White" d="m 117.75,123.56 h 13.10 6.73" />
          <MinimumDescentAltitudeIndicator bus={this.props.bus} />
          <SelectedAltIndicator
            bus={this.props.bus}
            targetAlt={this.shownTargetAltitude}
            targetAltColor={this.selectedAltColor}
          />
          <AltimeterIndicator bus={this.props.bus} altitude={this.altitude.derive((v) => v.value)} />
          <MetricAltIndicator
            bus={this.props.bus}
            targetAlt={this.shownTargetAltitude}
            targetAltColor={this.selectedAltColor}
          />
          <path
            id="AltReadoutBackground"
            class="BlackFill"
            d="m130.85 85.308h-13.13v-8.9706h13.13v-2.671h8.8647v14.313h-8.8647z"
          />
          <RadioAltIndicator bus={this.props.bus} filteredRadioAltitude={this.props.filteredRadioAltitude} />
          <DigitalAltitudeReadout bus={this.props.bus} />
        </g>
      </>
    );
  }
}

interface SelectedAltIndicatorProps {
  bus: ArincEventBus;
  targetAlt: Arinc429RegisterSubject;
  targetAltColor: Readable<TargetAltitudeColor>;
}

class SelectedAltIndicator extends View<SelectedAltIndicatorProps> {
  private readonly sub = this.props.bus.inbox<FcuEfisCpBusEvents & PrimFgBusBaseEvents>();

  private readonly altitude = Arinc429ConsumerSubject.create(
    this.props.bus.getArincSubscriber<Arinc429Values>().on('altitudeAr'),
  );

  private readonly fcuEisDiscreteWord2 = Arinc429LocalVarConsumerSubject.create(null);

  private selectedAltLowerGroupRef = UI.ref<SVGGElement>();

  private selectedAltLowerText = UI.ref<SVGTextElement>();

  private selectedAltLowerFLText = UI.ref<SVGTextElement>();

  private selectedAltUpperGroupRef = UI.ref<SVGGElement>();

  private selectedAltUpperText = UI.ref<SVGTextElement>();

  private selectedAltUpperFLText = UI.ref<SVGTextElement>();

  private readonly selectedAltFailed = Signal.of(false);

  private targetGroupRef = UI.ref<SVGGElement>();

  private blackFill = UI.ref<SVGPathElement>();

  private targetSymbolRef = UI.ref<SVGPathElement>();

  private altTapeTargetText = UI.ref<SVGTextElement>();

  private shownTargetAltitude = Arinc429Register.empty();

  private textSub = Signal.of('');

  private updateAltitudeColor(color: TargetAltitudeColor) {
    this.selectedAltLowerFLText.target.classList.toggle('Cyan', color === TargetAltitudeColor.Cyan);
    this.selectedAltLowerFLText.target.classList.toggle('Magenta', color === TargetAltitudeColor.Magenta);
    this.selectedAltLowerFLText.target.classList.toggle('White', color === TargetAltitudeColor.White);

    this.selectedAltLowerText.target.classList.toggle('Cyan', color === TargetAltitudeColor.Cyan);
    this.selectedAltLowerText.target.classList.toggle('Magenta', color === TargetAltitudeColor.Magenta);
    this.selectedAltLowerText.target.classList.toggle('White', color === TargetAltitudeColor.White);

    this.selectedAltUpperFLText.target.classList.toggle('Cyan', color === TargetAltitudeColor.Cyan);
    this.selectedAltUpperFLText.target.classList.toggle('Magenta', color === TargetAltitudeColor.Magenta);
    this.selectedAltUpperFLText.target.classList.toggle('White', color === TargetAltitudeColor.White);

    this.selectedAltUpperText.target.classList.toggle('Cyan', color === TargetAltitudeColor.Cyan);
    this.selectedAltUpperText.target.classList.toggle('Magenta', color === TargetAltitudeColor.Magenta);
    this.selectedAltUpperText.target.classList.toggle('White', color === TargetAltitudeColor.White);

    this.altTapeTargetText.target.classList.toggle('Cyan', color === TargetAltitudeColor.Cyan);
    this.altTapeTargetText.target.classList.toggle('Magenta', color === TargetAltitudeColor.Magenta);
    this.altTapeTargetText.target.classList.toggle('White', color === TargetAltitudeColor.White);

    this.targetSymbolRef.target.classList.toggle('Cyan', color === TargetAltitudeColor.Cyan);
    this.targetSymbolRef.target.classList.toggle('Magenta', color === TargetAltitudeColor.Magenta);
    this.targetSymbolRef.target.classList.toggle('White', color === TargetAltitudeColor.White);
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const isFo = getDisplayIndex() === 2;
    this.fcuEisDiscreteWord2.setConsumer(
      this.sub.topic(isFo ? 'fcu_efis_r_discrete_word_2' : 'fcu_efis_l_discrete_word_2'),
    );

    this.props.targetAltColor.watch((targetAltColor) => {
      this.updateAltitudeColor(targetAltColor);
    }, true);

    this.props.targetAlt.watch((targetAlt) => {
      this.shownTargetAltitude.set(targetAlt.rawWord);

      this.getOffset();
      this.handleAltitudeDisplay();
      this.setText();
    }, true);

    this.altitude.watch(() => {
      this.handleAltitudeDisplay();
      this.getOffset();
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

  private setText() {
    let boxLength = 19.14;
    let text = '0';
    const isStd = this.fcuEisDiscreteWord2.get().bitValueOr(11, true);
    if (isStd) {
      text = Math.round(this.shownTargetAltitude.value / 100)
        .toString()
        .padStart(3, '0');
      boxLength = 12.5;
    } else {
      text = Math.round(this.shownTargetAltitude.value).toString().padStart(5, ' ');
    }
    this.textSub.set(text);
    this.blackFill.target.setAttribute('d', `m117.75 77.784h${boxLength}v6.0476h-${boxLength}z`);
  }

  private getOffset() {
    const offset = ((this.altitude.get().value - this.shownTargetAltitude.value) * DistanceSpacing) / ValueSpacing;
    this.targetGroupRef.target.style.transform = `translate3d(0px, ${offset}px, 0px)`;
  }

  render(): ViewNode | null {
    return (
      <>
        <g id="SelectedAltLowerGroup" ref={this.selectedAltLowerGroupRef}>
          <text
            id="SelectedAltLowerText"
            ref={this.selectedAltLowerText}
            class="FontMedium EndAlign Cyan"
            x="137.7511"
            y="128.70299"
            style="white-space: pre"
          >
            {this.textSub}
          </text>
          <text
            id="SelectedAltLowerFLText"
            ref={this.selectedAltLowerFLText}
            class="FontSmall MiddleAlign Cyan"
            x="120.87094"
            y="128.71681"
          >
            FL
          </text>
        </g>
        <g id="SelectedAltUpperGroup" ref={this.selectedAltUpperGroupRef}>
          <text
            id="SelectedAltUpperText"
            ref={this.selectedAltUpperText}
            class="FontMedium EndAlign Cyan"
            x="138.22987"
            y="37.250134"
            style="white-space: pre"
          >
            {this.textSub}
          </text>
          <text
            id="SelectedAltUpperFLText"
            ref={this.selectedAltUpperFLText}
            class="FontSmall MiddleAlign Cyan"
            x="120.85925"
            y="37.125755"
          >
            FL
          </text>
        </g>
        <g id="AltTapeTargetSymbol" ref={this.targetGroupRef}>
          <path class="BlackFill" ref={this.blackFill} />
          <path
            class="NormalStroke Cyan"
            ref={this.targetSymbolRef}
            d="m122.79 83.831v6.5516h-7.0514v-8.5675l2.0147-1.0079m4.8441-3.0238v-6.5516h-6.8588v8.5675l2.0147 1.0079"
          />
          <text
            id="AltTapeTargetText"
            ref={this.altTapeTargetText}
            class="FontMedium StartAlign Cyan"
            x="118.228"
            y="83.067062"
            style="white-space: pre"
          >
            {this.textSub}
          </text>
        </g>
        <FlashOneHertz bus={this.props.bus} flashDuration={9} visible={this.selectedAltFailed}>
          <text id="SelectedAltUpperText" class="FontSmall EndAlign Red" x="136.22987" y="37.250134">
            ALT SEL
          </text>
        </FlashOneHertz>
      </>
    );
  }
}

interface AltimeterIndicatorProps {
  altitude: Readable<number>;
  bus: ArincEventBus;
}

class AltimeterIndicator extends View<AltimeterIndicatorProps> {
  private readonly sub = this.props.bus.inbox<FcuEfisCpBusEvents>();

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

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const isFo = getDisplayIndex() === 2;

    this.fcuEisDiscreteWord2.setConsumer(
      this.sub.topic(isFo ? 'fcu_efis_r_discrete_word_2' : 'fcu_efis_l_discrete_word_2'),
    );
    this.fcuBaroCorrectionHg.setConsumer(
      this.sub.topic(isFo ? 'fcu_efis_r_baro_setting_inhg' : 'fcu_efis_l_baro_setting_inhg'),
    );
    this.fcuBaroCorrectionHpa.setConsumer(this.sub.topic(isFo ? 'fcu_efis_r_baro_setting' : 'fcu_efis_l_baro_setting'));

    const sub = this.props.bus.getArincSubscriber<PFDSimvars & Arinc429Values>();

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
        <g ref={this.stdGroup} id="STDAltimeterModeGroup">
          <path class="NormalStroke Yellow" d="m124.79 131.74h13.096v7.0556h-13.096z" />
          <text class="FontMedium Cyan AlignLeft" x="125.75785" y="137.36">
            STD
          </text>
        </g>
        <g id="AltimeterGroup">
          <g ref={this.qfeGroup} id="QFEGroup">
            <path
              ref={this.qfeBorder}
              class="NormalStroke White"
              d="m 116.83686,133.0668 h 13.93811 v 5.8933 h -13.93811 z"
            />
            <text id="AltimeterModeText" class="FontMedium White" x="118.23066" y="138.11342">
              {this.mode}
            </text>
            <text id="AltimeterSettingText" class="FontMedium StartAlign Cyan" x="131" y="138.09006">
              {this.text}
            </text>
          </g>
        </g>
      </>
    );
  }
}

interface MetricAltIndicatorProps {
  bus: ArincEventBus;
  targetAlt: Arinc429RegisterSubject;
  targetAltColor: Readable<TargetAltitudeColor>;
}

class MetricAltIndicator extends View<MetricAltIndicatorProps> {
  private sub = this.props.bus.inbox<PrimFgBusBaseEvents & Arinc429Values & PFDSimvars>();

  private primFgDiscreteWord5 = Arinc429LocalVarConsumerSubject.create(this.sub.topic('prim_fg_discrete_word_5'));

  private readonly metricAltActive = this.primFgDiscreteWord5.derive((word) => word.bitValueOr(14, false));

  private readonly altitude = Arinc429ConsumerSubject.create(this.sub.topic('altitudeAr'));

  private currentMetricAlt = this.altitude.derive((alt) => Math.round((alt.value * 0.3048) / 10) * 10);

  private metricTargetAlt = this.props.targetAlt.derive((alt) => Math.round((alt.value * 0.3048) / 10) * 10);

  private readonly mda = BusSignal.from(this.sub.topic('mda'), 0);

  private readonly belowMda = Derived.from(([mda, altitude]) => altitude.value < mda, this.mda, this.altitude);

  render(): ViewNode {
    return (
      <g id="MetricAltGroup" visibility={this.metricAltActive.derive((active) => (active ? 'inherit' : 'hidden'))}>
        <path class="NormalStroke Yellow" d="m116.56 140.22h29.213v7.0556h-29.213z" />
        <text class="FontMedium Cyan MiddleAlign" x="142.03537" y="145.8689">
          M
        </text>
        <text
          id="MetricAltText"
          class={{
            FontMedium: true,
            MiddleAlign: true,
            Green: this.belowMda.derive(Logic.not()),
            Amber: this.belowMda,
          }}
          x="128.64708"
          y="145.86191"
        >
          {this.currentMetricAlt}
        </text>
        <g id="MetricAltTargetGroup">
          <text
            id="MetricAltTargetText"
            class={{
              FontSmallest: true,
              MiddleAlign: true,
              White: this.props.targetAltColor.derive((color) => color === TargetAltitudeColor.White),
              Cyan: this.props.targetAltColor.derive((color) => color === TargetAltitudeColor.Cyan),
              Magenta: this.props.targetAltColor.derive((color) => color === TargetAltitudeColor.Magenta),
            }}
            x="94.088852"
            y="37.926617"
          >
            {this.metricTargetAlt}
          </text>
          <text class="FontSmallest Cyan MiddleAlign" x="105.25774" y="37.872921">
            M
          </text>
        </g>
      </g>
    );
  }
}
