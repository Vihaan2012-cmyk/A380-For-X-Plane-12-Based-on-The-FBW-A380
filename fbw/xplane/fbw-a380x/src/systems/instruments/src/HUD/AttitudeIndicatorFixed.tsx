import {
  View,
  Bus,
  UI,
  Signal,
  Readable,
  ViewNode,
  ClockTopics,
  BusSignal,
  Derived,
  Bond,
} from 'xplane-js/ui';
import { Arinc429Word, Arinc429LocalVarConsumerSubject, ArincEventBus } from '@flybywiresim/fbw-sdk';
import { FlightPathDirector } from './FlightPathDirector';
import { FlightPathVector } from './FlightPathVector';
import { Arinc429Values } from './shared/ArincValueProvider';
import { HUDSimvars } from './shared/HUDSimvarPublisher';
import { FIVE_DEG, HudElems, HudMode, LagFilter } from './HUDUtils';
import { LateralMode } from '@shared/autopilot';
import { FmgcFlightPhase } from '@shared/flightphase';
import { SelectedFdEvents } from './shared/FdSelectionProvider';
import { FcdcBusEvents } from '@shared/publishers/FcdcPublisher';
import { PrimFgBusBaseEvents } from '@shared/publishers/PrimFgPublisher';
import { getDisplayIndex } from './HUD';
import { PanelInstrument } from 'xplane-js/data';

interface AttitudeIndicatorFixedUpperProps {
  readonly bus: Bus;
}

export class AttitudeIndicatorFixedUpper extends View<AttitudeIndicatorFixedUpperProps> {
  private readonly sub = this.props.bus.inbox<Arinc429Values & HudElems & FcdcBusEvents>();
  private fullGroupVis = '';
  private fullGroupRef = UI.ref<SVGGElement>();
  private attitudeIndicatorRef = UI.ref<SVGGElement>();
  private alternateLawRef = UI.ref<SVGGElement>();
  private attitudeIndicator = '';
  private roll = new Arinc429Word(0);

  private pitch = new Arinc429Word(0);

  private visibilitySub = Signal.of('hidden');

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

    this.sub
      .topic('attitudeIndicator')
      .distinct()
      .listen((v) => {
        this.attitudeIndicator = v;
        this.fullGroupVis = v;
        this.attitudeIndicatorRef.target.style.display = `${this.attitudeIndicator}`;
        this.fullGroupRef.target.style.display = `${this.fullGroupVis}`;
      });

    this.sub
      .topic('rollAr')
      .distinct()
      .listen((roll) => {
        this.roll = roll;
        if (!this.roll.isNormalOperation()) {
          this.attitudeIndicatorRef.target.style.display = 'none';
          this.alternateLawRef.target.style.display = 'none';
        } else {
          this.attitudeIndicatorRef.target.style.display = 'block';
          Math.abs(roll.value) > 35 && Math.abs(roll.value) <= 71
            ? (this.alternateLawRef.target.style.display = 'block')
            : (this.alternateLawRef.target.style.display = 'none');
        }
        if (Math.abs(roll.value) > 71) {
          this.attitudeIndicatorRef.target.style.display = 'none';
          this.alternateLawRef.target.style.display = 'none';
        }
      });

    this.sub
      .topic('pitchAr')
      .distinct()
      .listen((pitch) => {
        this.pitch = pitch;
        if (!this.pitch.isNormalOperation()) {
          this.attitudeIndicatorRef.target.style.display = 'none';
        } else {
          if (pitch.value > 39 || pitch.value < -25) {
            this.attitudeIndicatorRef.target.style.display = 'none';
            this.alternateLawRef.target.style.display = 'none';
          } else {
            this.attitudeIndicatorRef.target.style.display = 'block';
          }
        }
      });
  }

  render(): ViewNode {
    return (
      <g id="FullAttitudeUpperInfoGroup" ref={this.fullGroupRef}>
        <g id="AttitudeUpperInfoGroup" ref={this.attitudeIndicatorRef}>
          <g id="RollIndicatorFixed" class="NormalStroke Green">
            <path d="m 640,138.44282 12.21523,-20.20611 h -24.43047 zz" />
            <path d="m 735.2  164 14.1,-24.5" />
            <path d="m 705.1 150 7.7,-20.7" />
            <path d="m 673 141.5 4,-22.2" />
            <path d="m 544.8  164 -14.1,-24.5" />
            <path d="m 574.9 150 -7.7,-20.7" />
            <path d="m 607 141.5 -4,-22.2" />

            <g id="alternateLawRollRef" ref={this.alternateLawRef}>
              <path d="m 774.5 194.5 20,-20" />
              <path d="m 813 249 19,-9" />
              <path d="m 505.5 194.5 -20,-20" />
              <path d="m 467 249 -19,-9" />
              <path d="M 467.5 249.3 A 190 190 263 0 1  812.5 249.3 " />
            </g>
          </g>
        </g>
      </g>
    );
  }
}

interface AttitudeIndicatorFixedCenterProps {
  bus: ArincEventBus;
  isAttExcessive: Readable<boolean>;
  filteredRadioAlt: Readable<number>;
  instrument: PanelInstrument;
}

export class AttitudeIndicatorFixedCenter extends View<AttitudeIndicatorFixedCenterProps> {
  private roll = new Arinc429Word(0);

  private pitch = new Arinc429Word(0);

  private visibilitySub = Signal.of('hidden');

  private failureVis = Signal.of('hidden');

  private fdVisibilitySub = Signal.of('hidden');
  private inAirRef = UI.ref<SVGGElement>();
  private lateralMode = 0;

  private fdActive = false;
  private isActive(): boolean {
    if (!this.fdActive || !(this.lateralMode === LateralMode.ROLL_OUT || this.lateralMode === LateralMode.RWY)) {
      return false;
    }
    return true;
  }
  private sub = this.props.bus.inbox<Arinc429Values & HUDSimvars & HudElems>();
  private readonly inairAcftRef = BusSignal.from(this.sub.topic('inAirAcftRef').distinct(), '');
  private readonly ngc = BusSignal.from(this.sub.topic('noseGearCompressed').distinct(), true);
  private readonly hudMode = BusSignal.from(this.sub.topic('hudFlightPhaseMode').distinct(), 0);

  private readonly isInAirRefVisible = Derived.from(
    ([inairAcftRef, ngc, hudMode]) => {
      if (hudMode === HudMode.TAKEOFF || hudMode === HudMode.ROLLOUT_OR_RTO) {
        return !ngc || inairAcftRef === 'block' ? 'block' : 'none';
      } else {
        return inairAcftRef;
      }
    },
    this.inairAcftRef,
    this.ngc,
    this.hudMode,
  );

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.sub.topic('rollAr').listen((r) => {
      this.roll = r;
      if (!this.roll.isNormalOperation()) {
        this.visibilitySub.set('display:none');
        this.failureVis.set('display:block');
        this.fdVisibilitySub.set('display:none');
      } else {
        this.visibilitySub.set('display:inline');
        this.failureVis.set('display:none');
        if (!this.props.isAttExcessive.get()) {
          this.fdVisibilitySub.set('display:inline');
        }
      }
    });

    this.sub.topic('pitchAr').listen((p) => {
      this.pitch = p;

      if (!this.pitch.isNormalOperation()) {
        this.visibilitySub.set('display:none');
        this.failureVis.set('display:block');
        this.fdVisibilitySub.set('display:none');
      } else {
        this.visibilitySub.set('display:inline');
        this.failureVis.set('display:none');
        if (!this.props.isAttExcessive.get()) {
          this.fdVisibilitySub.set('display:inline');
        }
      }
    });

    this.props.isAttExcessive.watch((a) => {
      if (a) {
        this.fdVisibilitySub.set('display:none');
      } else if (this.roll.isNormalOperation() && this.pitch.isNormalOperation()) {
        this.fdVisibilitySub.set('display:inline');
      }
    });
  }

  render(): ViewNode {
    return (
      <>
        <text
          style={this.failureVis}
          id="AttFailText"
          class="Blink9Seconds FontLargest Green MiddleAlign"
          x="640"
          y="532"
        >
          ATT / HDG
        </text>
        <g id="AttitudeSymbolsGroup" style={this.visibilitySub}>
          <FlightPathDirector bus={this.props.bus} isAttExcessive={this.props.isAttExcessive} />
          <FlightPathVector
            bus={this.props.bus}
            instrument={this.props.instrument}
            isAttExcessive={this.props.isAttExcessive}
            filteredRadioAlt={this.props.filteredRadioAlt}
          />

          <g style={this.fdVisibilitySub}>
            <FDYawBar bus={this.props.bus} instrument={this.props.instrument} />
          </g>

          <ReverserIndicator bus={this.props.bus} />
          <g id="AircraftReferences">
            <g
              id="AircraftReferenceInAir"
              class="SmallStroke Green"
              ref={this.inAirRef}
              display={this.isInAirRefVisible}
            >
              <path d="m 625,335  v -6 h -30" />
              <path d="m 637,332 h 6 v -6 h -6 z" />
              <path d="m 655, 335 v -6 h 30" />
            </g>
          </g>
        </g>
      </>
    );
  }
}

class FDYawBar extends View<{ bus: Bus; instrument: PanelInstrument }> {
  private readonly sub = this.props.bus.inbox<
    SelectedFdEvents & Arinc429Values & HUDSimvars & HudElems & PrimFgBusBaseEvents
  >();

  private fdYawCommand = Arinc429LocalVarConsumerSubject.create(this.sub.topic('prim_yaw_fd_command'));

  private groundYawGroupRef = UI.ref<SVGGElement>();
  private GroundYawRef = UI.ref<SVGPathElement>();
  private onGround = true;
  private fdActive = BusSignal.from(this.sub.topic('fd_engaged'), false);

  private readonly transform = this.fdYawCommand.derive((word) => {
    const offset = -Math.max(Math.min(word.value, 45), -45) * 0.44;

    return `translate3d(${offset}px, 400px, 0px)`;
  });

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const isFo = getDisplayIndex() === 2;
    this.fdYawCommand.setConsumer(this.sub.topic(isFo ? 'prim_yaw_fd_command_2' : 'prim_yaw_fd_command_1'));

    this.sub
      .topic('hudFlightPhaseMode')
      .distinct()
      .listen((v) => {
        v === 0 ? (this.onGround = false) : (this.onGround = true);
        this.onGround
          ? (this.groundYawGroupRef.target.style.display = 'block')
          : (this.groundYawGroupRef.target.style.display = 'none');
      });
  }

  private readonly visibility = Derived.from(
    ([fdActive, fdYawCommand]) => {
      const visible = fdActive && !(fdYawCommand.isNoComputedData() || fdYawCommand.isFailureWarning());

      return visible ? 'inherit' : 'hidden';
    },
    this.fdActive,
    this.fdYawCommand,
  );

  render(): ViewNode {
    return (
      <g id="GroundYawGroup" ref={this.groundYawGroupRef}>
        <path
          style={{ transform: this.transform }}
          visibility={this.visibility}
          id="GroundYawSymbol"
          class="NormalStroke Green"
          d="m 640 0 v 40 h 8.059 v -40 l -4.03 -6.854 z"
        />
        <path ref={this.GroundYawRef} id="GroundYawRef" class="NormalStroke Green" d="m 640 385  l 10 -17.3 h -20  z" />
        <LocalizerIndicator bus={this.props.bus} instrument={this.props.instrument} />
      </g>
    );
  }
}

class LocalizerIndicator extends View<{ bus: Bus; instrument: PanelInstrument }> {
  private LSLocRef = UI.ref<SVGGElement>();
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
      this.locDiamond.target.style.transform = `translate3d(${(dots * 90.6) / 2}px, 0px, 0px)`;
    }
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<HUDSimvars & Arinc429Values & ClockTopics & HudElems>();
    const navRadialSub = sub.topic('navRadialError').listen(this.handleNavRadialError.bind(this), true);

    sub
      .topic('hasLoc')
      //.whenChanged()
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
      <g ref={this.LSLocRef} id="YawRefBackCourseLocalizer" transform={`translate(433.5 77)`}>
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
        <path id="LocalizerNeutralLine" class="Green Fill" d="m204.294 403.5v-24.191h4.536v24.191z" />
      </g>
    );
  }
}

interface DeclutterIndicatorProps {
  bus: ArincEventBus;
}

export class DeclutterIndicator extends View<DeclutterIndicatorProps> {
  private readonly sub = this.props.bus.inbox<HUDSimvars & HudElems & Arinc429Values>();
  private readonly decMode = BusSignal.from(this.sub.topic('decMode'), 0);
  private readonly fmgcFlightPhase = BusSignal.from(this.sub.topic('fmgcFlightPhase'), 0);
  private readonly hudMode = BusSignal.from(this.sub.topic('hudMode'), 0);

  private readonly declutterText = Derived.from(
    ([decMode, fmgcFlightPhase, hudMode]) => {
      if (hudMode === HudMode.NORMAL) {
        if (fmgcFlightPhase === FmgcFlightPhase.Approach) {
          if (decMode == 0) {
            return 'N';
          } else if (decMode == 1) {
            return 'D';
          } else if (decMode == 2) {
            return '';
          }
        } else {
          if (decMode == 0) {
            return 'N';
          } else {
            return 'D';
          }
        }
      } else {
        return '';
      }
    },
    this.decMode,
    this.fmgcFlightPhase,
    this.hudMode,
  );

  afterRender(node: ViewNode): void {
    super.afterRender(node);
  }

  render(): ViewNode {
    return (
      <g id="DeclutterModeIndicator">
        <text class="FontMedium  MiddleAlign Green" x="1000" y="900">
          {this.declutterText}
        </text>
      </g>
    );
  }
}

export class ReverserIndicator extends View<{ bus: ArincEventBus }> {
  private readonly subscriptions: Bond[] = [];
  private readonly sub = this.props.bus.getArincSubscriber<HUDSimvars & HudElems & ClockTopics>();
  private revGroupRef = UI.ref<SVGGElement>();
  private rev2Ref = UI.ref<SVGGElement>();
  private rev3Ref = UI.ref<SVGGElement>();
  private rev2TxtRef = UI.ref<SVGTextElement>();
  private rev3TxtRef = UI.ref<SVGTextElement>();
  private text: string = '';
  private readonly eng2State = BusSignal.from(this.sub.on('eng2State').distinct(), 0); // no rev failure implemented  using on/off state instead
  private readonly eng3State = BusSignal.from(this.sub.on('eng3State').distinct(), 0); // no rev failure implemented  using on/off state instead
  private readonly rev2 = BusSignal.from(this.sub.on('rev2').distinct(), 0);
  private readonly rev3 = BusSignal.from(this.sub.on('rev3').distinct(), 0);
  private readonly rev2Pos = BusSignal.from(this.sub.on('rev2Pos'), 0);
  private readonly rev3Pos = BusSignal.from(this.sub.on('rev3Pos'), 0);
  private readonly hudMode = BusSignal.from(this.sub.on('hudFlightPhaseMode').distinct(), 0);

  private readonly reverser2State = Derived.from(
    ([rev2, rev2Pos, eng2State, hudMode]) => {
      if (hudMode === HudMode.ROLLOUT_OR_RTO) {
        if (rev2 === 1) {
          if (eng2State === 1) {
            if (rev2Pos < 0.95) {
              return 1; // rev deployement in progress  display dash
            } else {
              return 2; // rev on  display R
            }
          } else {
            return 3; // not opperative  display cross
          }
        } else {
          return 0; // show nothing
        }
      } else {
        return 0; // show nothing
      }
    },
    this.rev2,
    this.rev2Pos,
    this.eng2State,
    this.hudMode,
  );
  private readonly reverser3State = Derived.from(
    ([rev3, rev3Pos, eng3State, hudMode]) => {
      if (hudMode === HudMode.ROLLOUT_OR_RTO || hudMode === HudMode.TAXI) {
        if (rev3 === 1) {
          if (eng3State === 1) {
            if (rev3Pos < 0.95) {
              return 1; // rev deployement in progress  display dash
            } else {
              return 2; // rev on  display R
            }
          } else {
            return 3; // not opperative  display cross
          }
        } else {
          return 0; // show nothing
        }
      } else {
        return 0; // show nothing
      }
    },
    this.rev3,
    this.rev3Pos,
    this.eng3State,
    this.hudMode,
  );

  private setState(eng: number, state: any) {
    if (eng === 2) {
      if (state === 1) {
        this.rev2Ref.target.setAttribute('d', 'm 615 290 v -17 h 17 v 17 z');
        this.rev2Ref.target.setAttribute('stroke-dasharray', '3 6');
        this.rev2TxtRef.target.textContent = '';
      } else if (state === 2) {
        this.rev2Ref.target.setAttribute('d', 'm 615 290 v -17 h 17 v 17 z');
        this.rev2Ref.target.setAttribute('stroke-dasharray', '');
        this.rev2TxtRef.target.textContent = 'R';
      } else if (state === 3) {
        this.rev2Ref.target.setAttribute('d', 'm 615 290 v -17 h 17 v 17 z  m 0 0 l 17 -17   m -17 0 l 17 17 ');
        this.rev2Ref.target.setAttribute('stroke-dasharray', '');
        this.rev2TxtRef.target.textContent = '';
      } else {
        this.rev2Ref.target.setAttribute('d', '');
        this.rev2TxtRef.target.textContent = '';
      }
    }
    if (eng === 3) {
      if (state === 1) {
        this.rev3Ref.target.setAttribute('d', 'm 648 290 v -17 h 17 v 17 z');
        this.rev3Ref.target.setAttribute('stroke-dasharray', '3 6');
        this.rev3TxtRef.target.textContent = '';
      } else if (state === 2) {
        this.rev3Ref.target.setAttribute('d', 'm 648 290 v -17 h 17 v 17 z');
        this.rev3Ref.target.setAttribute('stroke-dasharray', '');
        this.rev3TxtRef.target.textContent = 'R';
      } else if (state === 3) {
        this.rev3Ref.target.setAttribute('d', 'm 648 290 v -17 h 17 v 17 z  m 0 0 l 17 -17   m -17 0 l 17 17 ');
        this.rev3Ref.target.setAttribute('stroke-dasharray', '');
        this.rev3TxtRef.target.textContent = '';
      } else {
        this.rev3Ref.target.setAttribute('d', '');
        this.rev3TxtRef.target.textContent = '';
      }
    }
  }
  afterRender(node: ViewNode): void {
    super.afterRender(node);
    this.subscriptions.push(
      this.eng2State,
      this.eng3State,
      this.rev2,
      this.rev3,
      this.rev2Pos,
      this.rev3Pos,
      this.hudMode,
    );

    this.subscriptions.push(
      this.reverser2State.watch((state) => {
        this.setState(2, state);
      }),
    );
    this.subscriptions.push(
      this.reverser3State.watch((state) => {
        this.setState(3, state);
      }),
    );
  }

  destroy(): void {
    for (const s of this.subscriptions) {
      s.destroy();
    }

    super.destroy();
  }

  render(): ViewNode | null {
    return (
      <g id="ReverseIndicator" ref={this.revGroupRef}>
        <path ref={this.rev2Ref} class="NormalStroke Green " d="" />
        <text ref={this.rev2TxtRef} x="623.5" y="288 " class="FontForAnts MiddleAlign Green ">
          {this.text}
        </text>
        <path ref={this.rev3Ref} class="NormalStroke Green " d="" />
        <text ref={this.rev3TxtRef} x="656.5" y="288 " class="FontForAnts MiddleAlign Green ">
          {this.text}
        </text>
      </g>
    );
  }
}
