import {
  View,
  Bus,
  UI,
  PanelEventTopics,
  Signal,
  Readable,
  ViewNode,
  ClockTopics,
  BusSignal,
  Derived,
} from 'xplane-js/ui';
import { FIVE_DEG, getSmallestAngle, HudElems } from './HUDUtils';
import { HUDSimvars } from './shared/HUDSimvarPublisher';
import { Arinc429Values } from './shared/ArincValueProvider';
import { getDisplayIndex } from './HUD';
import { DmcLogicEvents } from '../MsfsAvionicsCommon/providers/DmcPublisher';
import { Arinc429ConsumerSubject } from '@flybywiresim/fbw-sdk';
import { FcuEfisCpBusEvents } from '@shared/publishers/EfisCpBusPublisher';
import { Arinc429LocalVarConsumerSubject } from '@flybywiresim/fbw-sdk';
import { PrimFgBusBaseEvents } from '@shared/publishers/PrimFgPublisher';

const DisplayRange = 18;
const DistanceSpacing = FIVE_DEG;
const ValueSpacing = 5;

export class HeadingOfftape extends View<{ bus: Bus; failed: Readable<boolean> }> {
  private readonly sub = this.props.bus.inbox<FcuEfisCpBusEvents>();

  private readonly fcuEisDiscreteWord2 = Arinc429LocalVarConsumerSubject.create(null);

  private readonly lsPressed = this.fcuEisDiscreteWord2.derive((word) => word.bitValueOr(14, true));

  private normalRef = UI.ref<SVGGElement>();

  private heading = Signal.of(0);

  private ILSCourse = Signal.of(0);

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const isFo = getDisplayIndex() === 2;

    this.fcuEisDiscreteWord2.setConsumer(
      this.sub.topic(isFo ? 'fcu_efis_r_discrete_word_2' : 'fcu_efis_l_discrete_word_2'),
    );
    const sub = this.props.bus.inbox<HUDSimvars & Arinc429Values & PanelEventTopics>();

    sub.topic('headingAr').listen((h) => {
      this.heading.set(h.value);

      if (h.isNormalOperation()) {
        this.normalRef.target.style.visibility = 'visible';
      } else {
        this.normalRef.target.style.visibility = 'hidden';
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
        <g id="HeadingOfftapeGroup" ref={this.normalRef}>
          <SelectedHeading heading={this.heading} bus={this.props.bus} />
          <QFUIndicator heading={this.heading} ILSCourse={this.ILSCourse} lsPressed={this.lsPressed} />
          <path class="Fill Green" d="m642 532 h -4 v -40 h 4 z" />
          <GroundTrackBug bus={this.props.bus} />
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
  private readonly sub = this.props.bus.inbox<HUDSimvars & HudElems & PrimFgBusBaseEvents & ClockTopics>();

  private targetIndicator = UI.ref<SVGPathElement>();

  private readonly targetIndicatorVis = BusSignal.from(this.sub.topic('spdChevrons'), 'block');

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

  afterRender(): void {
    this.trkFpaModeActive.watch((trkFpaModeActive) => {
      this.selectedHdgTrk.setConsumer(this.sub.topic(trkFpaModeActive ? 'prim_selected_track' : 'prim_selected_heading'));
    }, true);
  }

  render(): ViewNode {
    return (
      <>
        {' '}
        <path
          display={this.targetIndicatorVis}
          style={{ transform: this.headingBugTransform }}
          id="HeadingTargetIndicator"
          class={{ NormalStroke: true, Green: true, CornerRound: true, HiddenElement: this.headingBugHidden }}
          d="m641 512 10 -20 h -22 l 10 20"
        />
      </>
    );
  }
}

interface GroundTrackBugProps {
  bus: Bus;
}

class GroundTrackBug extends View<GroundTrackBugProps> {
  private trackIndicator = UI.ref<SVGGElement>();
  private heading = 0;
  private groundTrack = 0;
  private needsUpdate = false;
  private readonly sub = this.props.bus.inbox<Arinc429Values & ClockTopics & HudElems>();
  private readonly hdg = Arinc429ConsumerSubject.create(this.sub.topic('headingAr').distinct());
  private readonly trk = Arinc429ConsumerSubject.create(this.sub.topic('groundTrackAr').distinct());
  private readonly headingTrk = BusSignal.from(this.sub.topic('headingTrk').distinct(), '');
  private readonly fpa = Arinc429ConsumerSubject.create(this.sub.topic('fpa').distinct());
  private setPos(hdg: number, trk: number) {
    const offset = -(getSmallestAngle(hdg, trk) * DistanceSpacing) / ValueSpacing;
    this.trackIndicator.target.style.transform = `translate3d(${offset}px, 0px, 0px)`;
  }
  private readonly isHdgTrkVisible = Derived.from(
    ([hdg, trk, headingTrk, fpa]) => {
      if (headingTrk === 'block') {
        this.setPos(hdg.value, trk.value);
      }
      return fpa.value > -1.5 && fpa.value < 0.5 ? 'none' : headingTrk;
    },
    this.hdg,
    this.trk,
    this.headingTrk,
    this.fpa,
  );

  render(): ViewNode {
    return (
      <g ref={this.trackIndicator} id="ActualTrackIndicator" display={this.isHdgTrkVisible}>
        <path class="NormalOutline CornerRound" d="m640 512 -6 9 6 9 6 -9z" />
        <path class="NormalStroke Green CornerRound" d="m640 512 -6 9 6 9 6 -9z" />
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
            this.ilsCoursePointer.target.classList.add('HiddenElement');
          } else {
            this.ilsCoursePointer.target.classList.add('HiddenElement');
          }
        } else {
          const offset = (getSmallestAngle(this.ilsCourse, this.heading) * DistanceSpacing) / ValueSpacing;
          this.ilsCoursePointer.target.style.transform = `translate3d(${offset}px, 0px, 0px)`;
          this.ilsCoursePointer.target.classList.remove('HiddenElement');
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
            this.ilsCoursePointer.target.classList.add('HiddenElement');
          } else {
            this.ilsCoursePointer.target.classList.add('HiddenElement');
          }
        } else {
          const offset = (getSmallestAngle(this.ilsCourse, this.heading) * DistanceSpacing) / ValueSpacing;
          this.ilsCoursePointer.target.style.transform = `translate3d(${offset}px, 0px, 0px)`;
          this.ilsCoursePointer.target.classList.remove('HiddenElement');
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
          <path class="NormalStroke Green" d="m 620 505 h 20 m-10 -32.5 v 40" />
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
      .inbox<HUDSimvars>()
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
