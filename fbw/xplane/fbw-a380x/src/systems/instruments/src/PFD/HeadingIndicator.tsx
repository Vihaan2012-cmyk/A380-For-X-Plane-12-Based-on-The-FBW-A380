import {
  ClockTopics,
  View,
  Bus,
  UI,
  PanelEventTopics,
  Derived,
  Signal,
  Readable,
  ViewNode,
} from 'xplane-js/ui';
import { HorizontalTape } from './HorizontalTape';
import { getSmallestAngle } from './PFDUtils';
import { PFDSimvars } from './shared/PFDSimvarPublisher';
import { Arinc429Values } from './shared/ArincValueProvider';
import { getDisplayIndex } from './PFD';
import { DmcLogicEvents } from '../MsfsAvionicsCommon/providers/DmcPublisher';
import { FcuEfisCpBusEvents } from '@shared/publishers/EfisCpBusPublisher';
import { Arinc429LocalVarConsumerSubject } from '@flybywiresim/fbw-sdk';
import { PrimFgBusBaseEvents } from '@shared/publishers/PrimFgPublisher';

const DisplayRange = 24;
const DistanceSpacing = 7.555;
const ValueSpacing = 5;

interface HeadingTapeProps {
  bus: Bus;
  failed: Readable<boolean>;
}

export class HeadingTape extends View<HeadingTapeProps> {
  private headingTapeRef = UI.ref<SVGGElement>();

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.props.failed.watch((failed) => {
      if (failed) {
        this.headingTapeRef.target.style.visibility = 'hidden';
      } else {
        this.headingTapeRef.target.style.visibility = 'visible';
      }
    });
  }

  render(): ViewNode {
    return (
      <g ref={this.headingTapeRef}>
        <path id="HeadingTapeBackground" d="m32.138 145.34h73.536v10.382h-73.536z" class="TapeBackground" />
        <HorizontalTape
          bus={this.props.bus}
          type="headingTape"
          displayRange={DisplayRange + 3}
          valueSpacing={ValueSpacing}
          distanceSpacing={DistanceSpacing}
        />
      </g>
    );
  }
}

export class HeadingOfftape extends View<{ bus: Bus; failed: Readable<boolean> }> {
  private readonly sub = this.props.bus.inbox<FcuEfisCpBusEvents>();

  private readonly fcuEisDiscreteWord2 = Arinc429LocalVarConsumerSubject.create(null);

  private readonly lsPressed = this.fcuEisDiscreteWord2.derive((word) => word.bitValueOr(14, true));

  private normalRef = UI.ref<SVGGElement>();

  private abnormalRef = UI.ref<SVGGElement>();

  private heading = Signal.of(0);

  private ILSCourse = Signal.of(0);

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const isFo = getDisplayIndex() === 2;

    this.fcuEisDiscreteWord2.setConsumer(
      this.sub.topic(isFo ? 'fcu_efis_r_discrete_word_2' : 'fcu_efis_l_discrete_word_2'),
    );

    const sub = this.props.bus.inbox<PFDSimvars & Arinc429Values & PanelEventTopics>();

    sub.topic('headingAr').listen((h) => {
      this.heading.set(h.value);

      if (h.isNormalOperation()) {
        this.normalRef.target.style.visibility = 'visible';
        this.abnormalRef.target.style.visibility = 'hidden';
      } else {
        this.normalRef.target.style.visibility = 'hidden';
        this.abnormalRef.target.style.visibility = 'visible';
      }
    });

    sub
      .topic('ilsCourse')
      .distinct()
      .listen((n) => {
        this.ILSCourse.set(n);
      });
  }

  render(): ViewNode {
    return (
      <>
        <g ref={this.abnormalRef}>
          <path id="HeadingTapeBackground" d="m32.138 145.34h73.536v10.382h-73.536z" class="TapeBackground" />
          <path id="HeadingTapeOutline" class="NormalStroke Red" d="m32.138 156.23v-10.886h73.536v10.886" />
          <text id="HDGFailText" class="Blink9Seconds FontLargest EndAlign Red" x="75.926208" y="151.95506">
            HDG
          </text>
        </g>
        <g id="HeadingOfftapeGroup" ref={this.normalRef}>
          <path id="HeadingTapeOutline" class="NormalStroke White" d="m32.138 156.23v-10.886h73.536v10.886" />
          <SelectedHeading heading={this.heading} bus={this.props.bus} />
          <QFUIndicator heading={this.heading} ILSCourse={this.ILSCourse} lsPressed={this.lsPressed} />
          <path class="Fill Yellow" d="m69.61 147.31h-1.5119v-8.0635h1.5119z" />
          <GroundTrackBug bus={this.props.bus} heading={this.heading} />
          <TrueFlag bus={this.props.bus} />
        </g>
      </>
    );
  }
}

interface SelectedHeadingProps {
  bus: Bus;
  heading: Readable<number>;
}

class SelectedHeading extends View<SelectedHeadingProps> {
  private sub = this.props.bus.inbox<PrimFgBusBaseEvents & ClockTopics>();

  private fgDiscreteWord5 = Arinc429LocalVarConsumerSubject.create(this.sub.topic('prim_fg_discrete_word_5'));

  private selectedHdgTrk = Arinc429LocalVarConsumerSubject.create(null);

  private readonly trkFpaModeActive = this.fgDiscreteWord5.derive((word) => word.bitValueOr(11, true));

  private headingDelta = Derived.from(
    ([heading, selectedHeading]) => {
      return getSmallestAngle(selectedHeading.value, heading);
    },
    this.props.heading,
    this.selectedHdgTrk,
  );

  private readonly selectedHeadingValid = this.selectedHdgTrk.derive(
    (hdg) => hdg.isNormalOperation() || hdg.isFunctionalTest(),
  );

  private readonly text = this.selectedHdgTrk.derive((selectedHeading) =>
    Math.round(selectedHeading.value).toString().padStart(3, '0'),
  );

  private readonly headingBugTransform = this.headingDelta.derive(
    (headingDelta) => `translate3d(${(headingDelta * DistanceSpacing) / ValueSpacing}px, 0px, 0px)`,
  );

  private readonly headingBugHidden = Derived.from(
    ([selectedHeadingValid, headingDelta]) => {
      return !(selectedHeadingValid && Math.abs(headingDelta) < DisplayRange);
    },
    this.selectedHeadingValid,
    this.headingDelta,
  );

  private readonly leftTextHidden = Derived.from(
    ([selectedHeadingValid, headingDelta]) => {
      return !(selectedHeadingValid && Math.abs(headingDelta) > DisplayRange && headingDelta < 0);
    },
    this.selectedHeadingValid,
    this.headingDelta,
  );

  private readonly rightTextHidden = Derived.from(
    ([selectedHeadingValid, headingDelta]) => {
      return !(selectedHeadingValid && Math.abs(headingDelta) > DisplayRange && headingDelta > 0);
    },
    this.selectedHeadingValid,
    this.headingDelta,
  );

  afterRender(): void {
    this.trkFpaModeActive.watch((trkFpaModeActive) => {
      this.selectedHdgTrk.setConsumer(this.sub.topic(trkFpaModeActive ? 'prim_selected_track' : 'prim_selected_heading'));
    }, true);
  }

  render(): ViewNode {
    return (
      <>
        <path
          style={{ transform: this.headingBugTransform }}
          id="HeadingTargetIndicator"
          class={{ NormalStroke: true, Cyan: true, CornerRound: true, HiddenElement: this.headingBugHidden }}
          d="m69.978 145.1 1.9501-5.3609h-6.0441l1.9501 5.3609"
        />
        <text
          id="SelectedHeadingTextRight"
          class={{ FontSmallest: true, Cyan: true, MiddleAlign: true, HiddenElement: this.rightTextHidden }}
          x="101.70432"
          y="144.34792"
        >
          {this.text}
        </text>
        <text
          id="SelectedHeadingTextLeft"
          class={{ FontSmallest: true, Cyan: true, MiddleAlign: true, HiddenElement: this.leftTextHidden }}
          x="36.418198"
          y="144.32108"
        >
          {this.text}
        </text>
      </>
    );
  }
}

interface GroundTrackBugProps {
  heading: Readable<number>;
  bus: Bus;
}

class GroundTrackBug extends View<GroundTrackBugProps> {
  private trackIndicator = UI.ref<SVGGElement>();

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<Arinc429Values>();

    sub.topic('groundTrackAr').listen((groundTrack) => {
      //  if (groundTrack.isNormalOperation()) {
      const offset = (getSmallestAngle(groundTrack.value, this.props.heading.get()) * DistanceSpacing) / ValueSpacing;
      this.trackIndicator.target.style.display = 'inline';
      this.trackIndicator.target.style.transform = `translate3d(${offset}px, 0px, 0px)`;
      //   } else {
      //       this.trackIndicator.instance.style.display = 'none';
      //   }
    });
  }

  render(): ViewNode {
    return (
      <g ref={this.trackIndicator} id="ActualTrackIndicator">
        <path class="ThickOutline CornerRound" d="m68.906 145.75-1.2592 1.7639 1.2592 1.7639 1.2592-1.7639z" />
        <path class="ThickStroke Green CornerRound" d="m68.906 145.75-1.2592 1.7639 1.2592 1.7639 1.2592-1.7639z" />
      </g>
    );
  }
}

class QFUIndicator extends View<{
  ILSCourse: Readable<number>;
  heading: Readable<number>;
  lsPressed: Readable<boolean>;
}> {
  private qfuContainer = UI.ref<SVGGElement>();

  private ilsCourseRight = UI.ref<SVGGElement>();

  private ilsCourseLeft = UI.ref<SVGGElement>();

  private ilsCoursePointer = UI.ref<SVGGElement>();

  private heading = 0;

  private ilsCourse = -1;

  private lsPressed = false;

  private text = Signal.of('');

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.props.heading.watch((h) => {
      this.heading = h;

      const delta = getSmallestAngle(this.ilsCourse, this.heading);
      this.text.set(Math.round(this.ilsCourse).toString().padStart(3, '0'));

      if (this.ilsCourse < 0) {
        this.qfuContainer.target.classList.add('HiddenElement');
      } else if (this.lsPressed) {
        this.qfuContainer.target.classList.remove('HiddenElement');
        if (Math.abs(delta) > DisplayRange) {
          if (delta > 0) {
            this.ilsCourseRight.target.classList.remove('HiddenElement');
            this.ilsCourseLeft.target.classList.add('HiddenElement');
            this.ilsCoursePointer.target.classList.add('HiddenElement');
          } else {
            this.ilsCourseLeft.target.classList.remove('HiddenElement');
            this.ilsCourseRight.target.classList.add('HiddenElement');
            this.ilsCoursePointer.target.classList.add('HiddenElement');
          }
        } else {
          const offset = (getSmallestAngle(this.ilsCourse, this.heading) * DistanceSpacing) / ValueSpacing;
          this.ilsCoursePointer.target.style.transform = `translate3d(${offset}px, 0px, 0px)`;
          this.ilsCoursePointer.target.classList.remove('HiddenElement');
          this.ilsCourseRight.target.classList.add('HiddenElement');
          this.ilsCourseLeft.target.classList.add('HiddenElement');
        }
      }
    });

    this.props.ILSCourse.watch((c) => {
      this.ilsCourse = c;

      const delta = getSmallestAngle(this.ilsCourse, this.heading);
      this.text.set(Math.round(this.ilsCourse).toString().padStart(3, '0'));

      if (c < 0) {
        this.qfuContainer.target.classList.add('HiddenElement');
      } else if (this.lsPressed) {
        this.qfuContainer.target.classList.remove('HiddenElement');
        if (Math.abs(delta) > DisplayRange) {
          if (delta > 0) {
            this.ilsCourseRight.target.classList.remove('HiddenElement');
            this.ilsCourseLeft.target.classList.add('HiddenElement');
            this.ilsCoursePointer.target.classList.add('HiddenElement');
          } else {
            this.ilsCourseLeft.target.classList.remove('HiddenElement');
            this.ilsCourseRight.target.classList.add('HiddenElement');
            this.ilsCoursePointer.target.classList.add('HiddenElement');
          }
        } else {
          const offset = (getSmallestAngle(this.ilsCourse, this.heading) * DistanceSpacing) / ValueSpacing;
          this.ilsCoursePointer.target.style.transform = `translate3d(${offset}px, 0px, 0px)`;
          this.ilsCoursePointer.target.classList.remove('HiddenElement');
          this.ilsCourseRight.target.classList.add('HiddenElement');
          this.ilsCourseLeft.target.classList.add('HiddenElement');
        }
      }
    });

    this.props.lsPressed.watch((ls) => {
      this.lsPressed = ls;
      if (ls) {
        this.qfuContainer.target.classList.remove('HiddenElement');
      } else {
        this.qfuContainer.target.classList.add('HiddenElement');
      }
    });
  }

  render(): ViewNode {
    return (
      <g ref={this.qfuContainer}>
        <g id="ILSCoursePointer" class="HiddenElement" ref={this.ilsCoursePointer}>
          <path class="ThickOutline" d="m66.992 152.82h3.8279m-1.914-6.5471v9.4518" />
          <path class="ThickStroke Magenta" d="m66.992 152.82h3.8279m-1.914-6.5471v9.4518" />
        </g>
        <g id="ILSCourseRight" class="HiddenElement" ref={this.ilsCourseRight}>
          <path class="BlackFill NormalStroke White" d="m100.57 149.68h12.088v6.5516h-12.088z" />
          <text id="ILSCourseTextRight" class="FontMedium MiddleAlign Magenta" x="106.95047" y="155.22305">
            {this.text}
          </text>
        </g>
        <g id="ILSCourseLeft" class="HiddenElement" ref={this.ilsCourseLeft}>
          <path class="BlackFill NormalStroke White" d="m26.094 156.18v-6.5516h12.088v6.5516z" />
          <text id="ILSCourseTextLeft" class="FontMedium MiddleAlign Magenta" x="32.406616" y="155.22305">
            {this.text}
          </text>
        </g>
      </g>
    );
  }
}

interface TrueFlagProps {
  bus: Bus;
}

class TrueFlag extends View<TrueFlagProps> {
  private readonly trueRefActive = Signal.of(false);

  private readonly slatsExtended = Signal.of(false);

  private readonly slatsExtendedWithTrue = Signal.of(false);

  private readonly trueFlagRef = UI.ref<SVGGElement>();

  /** @inheritdoc */
  afterRender(node: ViewNode): void {
    super.afterRender(node);
    this.props.bus
      .inbox<DmcLogicEvents>()
      .topic('trueRefActive')
      .distinct()
      .listen((v) => this.trueRefActive.set(v));
    // FIXME this should be 127-11 from FWC
    this.props.bus
      .inbox<PFDSimvars>()
      .topic('slatPosLeft')
      .rounded(0.25)
      .listen((v) => this.slatsExtended.set(v > 0.4));

    this.trueRefActive.watch((trueRef) => this.trueFlagRef.target.classList.toggle('HiddenElement', !trueRef), true);

    this.trueRefActive.watch(this.handleSlatsTrue.bind(this));
    this.slatsExtended.watch(this.handleSlatsTrue.bind(this));
    this.slatsExtendedWithTrue.watch((flash) => this.trueFlagRef.target.classList.toggle('Blink10Seconds', flash));
  }

  private handleSlatsTrue(): void {
    this.slatsExtendedWithTrue.set(this.trueRefActive.get() && this.slatsExtended.get());
  }

  /** @inheritdoc */
  render(): ViewNode {
    return (
      <g id="TrueRefFlag" ref={this.trueFlagRef}>
        <rect x="62.439" y="134.468" width="12.935" height="4.575" class="Cyan NormalStroke" />
        <text x="68.9065" y="137.008" text-anchor="middle" alignment-baseline="middle" class="FontSmallest Cyan">
          TRUE
        </text>
      </g>
    );
  }
}
