// @ts-strict-ignore
import {
  ClockTopics,
  ViewProps,
  BusSignal,
  View,
  UI,
  Derived,
  Signal,
  Logic,
  Bond,
  ViewNode,
} from 'xplane-js/ui';
import { LowerArea } from './LowerArea';
import { Arinc429LocalVarConsumerSubject, Arinc429Word, ArincEventBus, FailuresConsumer } from '@flybywiresim/fbw-sdk';

import { AttitudeIndicatorWarnings } from '@flybywiresim/pfd';
import { AttitudeIndicatorWarningsA380 } from './AttitudeIndicatorWarningsA380';
import { LinearDeviationIndicator } from './LinearDeviationIndicator';
import { CdsDisplayUnit, DisplayUnitID } from '../MsfsAvionicsCommon/CdsDisplayUnit';
import { LagFilter } from './PFDUtils';
import { Arinc429Values } from './shared/ArincValueProvider';
import { AltitudeIndicator, AltitudeIndicatorOfftape } from './AltitudeIndicator';
import { AttitudeIndicatorFixedCenter, AttitudeIndicatorFixedUpper } from './AttitudeIndicatorFixed';
import { FMA } from './FMA';
import { HeadingOfftape, HeadingTape } from './HeadingIndicator';
import { Horizon } from './AttitudeIndicatorHorizon';
import { LandingSystem } from './LandingSystemIndicator';
import { AirspeedIndicator, AirspeedIndicatorOfftape, MachNumber } from './SpeedIndicator';
import { VerticalSpeedIndicator } from './VerticalSpeedIndicator';

import './style.scss';
import { PitchTrimDisplay } from './PitchTrimDisplay';
import { PFDSimvars } from './shared/PFDSimvarPublisher';
import { PanelInstrument } from 'xplane-js/data';

export const getDisplayIndex = () => {
  const url = Array.from(document.querySelectorAll('vcockpit-panel > *'))
    .find((it) => it.tagName.toLowerCase() !== 'wasm-instrument')
    .getAttribute('url');

  const duId = url ? parseInt(url.substring(url.length - 1), 10) : -1;

  switch (duId) {
    case 0:
      return 1;
    case 3:
      return 2;
    default:
      return 0;
  }
};

interface PFDProps extends ViewProps {
  bus: ArincEventBus;
  instrument: PanelInstrument;
}

export class PFDComponent extends View<PFDProps> {
  private readonly subscriptions: Bond[] = [];
  private readonly sub = this.props.bus.inbox<Arinc429Values & ClockTopics & PFDSimvars>();

  private headingFailed = Signal.of(true);

  private displayFailed = Signal.of(false);

  private isAttExcessive = Signal.of(false);

  private pitch = new Arinc429Word(0);

  private roll = new Arinc429Word(0);

  private ownRadioAltitude = new Arinc429Word(0);

  private filteredRadioAltitude = Signal.of(0);

  private radioAltitudeFilter = new LagFilter(5);

  private failuresConsumer: FailuresConsumer;

  private readonly groundSpeed = Arinc429LocalVarConsumerSubject.create(this.sub.topic('groundSpeed'), 0);

  private readonly spoilersArmed = BusSignal.from(this.sub.topic('spoilersArmed'), false);

  private readonly thrustTla = [
    BusSignal.from(this.sub.topic('tla1'), 0),
    BusSignal.from(this.sub.topic('tla2'), 0),
    BusSignal.from(this.sub.topic('tla3'), 0),
    BusSignal.from(this.sub.topic('tla4'), 0),
  ];
  private readonly atLeastThreeThrustLeversOutOfIdle = Derived.from(
    ([t1, t2, t3, t4]) => [t1, t2, t3, t4].filter((t) => t > 5).length > 2,
    ...this.thrustTla,
  );

  private readonly leftMainGearCompressed = BusSignal.from(this.sub.topic('leftMainGearCompressed'), false);
  private readonly rightMainGearCompressed = BusSignal.from(this.sub.topic('rightMainGearCompressed'), false);
  private readonly eitherMainLgCompressed = Derived.from(
    Logic.or(),
    this.leftMainGearCompressed,
    this.rightMainGearCompressed,
  );

  private previousFlapHandlePosition = 0;

  private readonly pitchTrimIndicatorVisible = Signal.of(false);

  private updatePitchTrimVisible(flapsRetracted = false) {
    const gs = this.groundSpeed.get().valueOr(0);
    if (this.filteredRadioAltitude.get() > 50) {
      this.pitchTrimIndicatorVisible.set(false);
    } else if (gs < 30) {
      this.pitchTrimIndicatorVisible.set(true);
    } else if (
      this.eitherMainLgCompressed.get() &&
      gs > 80 &&
      (this.spoilersArmed.get() === false || flapsRetracted === true || this.atLeastThreeThrustLeversOutOfIdle.get())
    ) {
      // FIXME add "flight crew presses pitch trim switches"
      this.pitchTrimIndicatorVisible.set(true);
    }
  }

  constructor(props: PFDProps) {
    super(props);
    this.failuresConsumer = new FailuresConsumer();
  }

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subscriptions.push(
      this.sub.topic('headingAr').listen((h) => {
        if (this.headingFailed.get() !== h.isNormalOperation()) {
          this.headingFailed.set(!h.isNormalOperation());
        }
      }),
    );

    this.subscriptions.push(
      this.sub.topic('rollAr').listen((r) => {
        this.roll = r;
      }),
    );

    this.subscriptions.push(
      this.sub.topic('pitchAr').listen((p) => {
        this.pitch = p;
      }),
    );

    this.subscriptions.push(
      this.sub
        .topic('realTime')
        .limit(1)
        .listen((_t) => {
          this.failuresConsumer.update();
          if (
            !this.isAttExcessive.get() &&
            ((this.pitch.isNormalOperation() && (this.pitch.value > 25 || this.pitch.value < -13)) ||
              (this.roll.isNormalOperation() && Math.abs(this.roll.value) > 45))
          ) {
            this.isAttExcessive.set(true);
          } else if (
            this.isAttExcessive.get() &&
            this.pitch.isNormalOperation() &&
            this.pitch.value < 22 &&
            this.pitch.value > -10 &&
            this.roll.isNormalOperation() &&
            Math.abs(this.roll.value) < 40
          ) {
            this.isAttExcessive.set(false);
          }
        }),
    );

    this.subscriptions.push(
      this.sub.topic('chosenRa').listen((ra) => {
        this.ownRadioAltitude = ra;
        const filteredRadioAltitude = this.radioAltitudeFilter.step(
          this.ownRadioAltitude.value,
          this.props.instrument.frameDelta / 1000,
        );
        this.filteredRadioAltitude.set(filteredRadioAltitude);
        this.updatePitchTrimVisible();
      }),
    );

    this.subscriptions.push(
      this.sub
        .topic('flapHandleIndex')
        .distinct()
        .listen((fh) => {
          if (this.previousFlapHandlePosition > fh) {
            this.updatePitchTrimVisible(true);
          }
          this.previousFlapHandlePosition = fh;
        }),
    );

    this.subscriptions.push(
      this.groundSpeed.watch(() => this.updatePitchTrimVisible()),
      this.spoilersArmed.watch(() => this.updatePitchTrimVisible()),
      this.atLeastThreeThrustLeversOutOfIdle.watch(() => this.updatePitchTrimVisible()),
      this.groundSpeed,
      this.spoilersArmed,
      this.leftMainGearCompressed,
      this.rightMainGearCompressed,
      this.eitherMainLgCompressed,
      this.atLeastThreeThrustLeversOutOfIdle,
    );

    for (const s of this.thrustTla) {
      this.subscriptions.push(s);
    }
  }

  render(): ViewNode {
    return (
      <CdsDisplayUnit
        bus={this.props.bus}
        displayUnitId={getDisplayIndex() === 1 ? DisplayUnitID.CaptPfd : DisplayUnitID.FoPfd}
        test={Signal.of(-1)}
        failed={Signal.of(false)}
      >
        <svg
          class="pfd-svg"
          version="1.1"
          viewBox="0 0 158.75 211.6"
          xmlns="http://www.w3.org/2000/svg"
          xmlnsXlink="http://www.w3.org/1999/xlink"
        >
          <Horizon
            bus={this.props.bus}
            instrument={this.props.instrument}
            isAttExcessive={this.isAttExcessive}
            filteredRadioAlt={this.filteredRadioAltitude}
          />
          <AttitudeIndicatorFixedCenter bus={this.props.bus} isAttExcessive={this.isAttExcessive} />
          <path
            id="Mask1"
            class="BackgroundFill"
            // eslint-disable-next-line max-len
            d="m 32.138 101.25 c 7.4164 13.363 21.492 21.652 36.768 21.652 c 15.277 0 29.352 -8.2886 36.768 -21.652 v -40.859 c -7.4164 -13.363 -21.492 -21.652 -36.768 -21.652 c -15.277 0 -29.352 8.2886 -36.768 21.652 z m -32.046 110.498 h 158.66 v -211.75 h -158.66 z"
          />
          <HeadingTape bus={this.props.bus} failed={this.headingFailed} />
          <AltitudeIndicator bus={this.props.bus} />
          <AirspeedIndicator bus={this.props.bus} instrument={this.props.instrument} />
          <path
            id="Mask2"
            class="BackgroundFill"
            // eslint-disable-next-line max-len
            d="m 32.138 145.34 h 73.536 v 10.382 h -73.536 z m 0 -44.092 c 7.4164 13.363 21.492 21.652 36.768 21.652 c 15.277 0 29.352 -8.2886 36.768 -21.652 v -40.859 c -7.4164 -13.363 -21.492 -21.652 -36.768 -21.652 c -15.277 0 -29.352 8.2886 -36.768 21.652 z m -32.046 110.498 h 158.66 v -211.746 h -158.66 z m 115.14 -88.191 v -85.473 h 20.344 v 85.473 z m -113.33 0 v -85.473 h 27.548 v 85.473 z"
          />
          <AirspeedIndicatorOfftape bus={this.props.bus} />

          <LandingSystem bus={this.props.bus} instrument={this.props.instrument} />
          <AttitudeIndicatorFixedUpper bus={this.props.bus} />
          <AttitudeIndicatorWarnings bus={this.props.bus} instrument={this.props.instrument} />
          <AttitudeIndicatorWarningsA380 bus={this.props.bus} instrument={this.props.instrument} />
          <VerticalSpeedIndicator
            bus={this.props.bus}
            instrument={this.props.instrument}
            filteredRadioAltitude={this.filteredRadioAltitude}
          />
          <HeadingOfftape bus={this.props.bus} failed={this.headingFailed} />
          <AltitudeIndicatorOfftape bus={this.props.bus} filteredRadioAltitude={this.filteredRadioAltitude} />
          <LinearDeviationIndicator bus={this.props.bus} />

          <MachNumber bus={this.props.bus} />
          <FMA bus={this.props.bus} isAttExcessive={this.isAttExcessive} />

          <LowerArea bus={this.props.bus} pitchTrimIndicatorVisible={this.pitchTrimIndicatorVisible} />
        </svg>
        <PitchTrimDisplay bus={this.props.bus} visible={this.pitchTrimIndicatorVisible} />
      </CdsDisplayUnit>
    );
  }

  destroy(): void {
    for (const s of this.subscriptions) {
      s.destroy();
    }

    super.destroy();
  }
}
