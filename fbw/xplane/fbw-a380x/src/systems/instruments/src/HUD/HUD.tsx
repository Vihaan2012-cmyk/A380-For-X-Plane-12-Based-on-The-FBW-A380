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
  PanelEventTopics,
} from 'xplane-js/ui';
import { Arinc429Word, ArincEventBus, FailuresConsumer } from '@flybywiresim/fbw-sdk';
import { AttitudeIndicatorWarnings } from '@flybywiresim/hud';
import { AttitudeIndicatorWarningsA380 } from '../HUD/AttitudeIndicatorWarningsA380';
import { LinearDeviationIndicator } from '../HUD/LinearDeviationIndicator';
import { CdsDisplayUnit, DisplayUnitID } from '../MsfsAvionicsCommon/CdsDisplayUnit';
import { LagFilter, HudElems } from './HUDUtils';
import { Arinc429Values } from './shared/ArincValueProvider';
import { AltitudeIndicator, AltitudeIndicatorOfftape } from './AltitudeIndicator';
import { AttitudeIndicatorFixedCenter, AttitudeIndicatorFixedUpper } from './AttitudeIndicatorFixed';
import { FMA } from './FMA';
import { Horizon } from './AttitudeIndicatorHorizon';
import { LandingSystem } from './LandingSystemIndicator';
import { AirspeedIndicator, AirspeedIndicatorOfftape, MachNumber } from './SpeedIndicator';
import { VerticalSpeedIndicator } from './VerticalSpeedIndicator';
import { Grid } from './HUDUtils';
import { DmcLogicEvents } from '../MsfsAvionicsCommon/providers/DmcPublisher';
import './style.scss';
import { HUDSimvars } from '../HUD/shared/HUDSimvarPublisher';
import { WindIndicator } from '../../../../../../fbw-common/src/systems/instruments/src/ND/shared/WindIndicator';
import { DecelIndicator } from './DecelSpeedIndicator';
import { DeclutterIndicator } from './AttitudeIndicatorFixed';
import { HudWarnings } from './HudWarnings';
import { RadioNavInfo } from './RadioNavInfo';
import { PanelInstrument } from 'xplane-js/data';

export const getDisplayIndex = () => {
  const url = Array.from(document.querySelectorAll('vcockpit-panel > *'))
    .find((it) => it.tagName.toLowerCase() !== 'wasm-instrument')
    .getAttribute('url');

  const duId = url ? parseInt(url.substring(url.length - 1), 10) : -1;

  switch (duId) {
    case 8:
      return 1;
    case 9:
      return 2;
    default:
      return 0;
  }
};

interface HUDProps extends ViewProps {
  bus: ArincEventBus;
  instrument: PanelInstrument;
}

export class HUDComponent extends View<HUDProps> {
  private readonly subscriptions: Bond[] = [];
  private readonly sub = this.props.bus.inbox<
    Arinc429Values & ClockTopics & DmcLogicEvents & HUDSimvars & PanelEventTopics & HudElems
  >();

  private headingFailed = Signal.of(true);

  private displayFailed = Signal.of(false);

  private isAttExcessive = Signal.of(false);

  private pitch = new Arinc429Word(0);

  private roll = new Arinc429Word(0);

  private ownRadioAltitude = new Arinc429Word(0);

  private filteredRadioAltitude = Signal.of(0);

  private radioAltitudeFilter = new LagFilter(5);

  private failuresConsumer: FailuresConsumer;

  private readonly leftMainGearCompressed = BusSignal.from(this.sub.topic('leftMainGearCompressed'), false);
  private readonly rightMainGearCompressed = BusSignal.from(this.sub.topic('rightMainGearCompressed'), false);
  private readonly eitherMainLgCompressed = Derived.from(
    Logic.or(),
    this.leftMainGearCompressed,
    this.rightMainGearCompressed,
  );

  private readonly hudLpos = BusSignal.from(this.sub.topic('hudLPos'), 0);
  private readonly hudRpos = BusSignal.from(this.sub.topic('hudRPos'), 0);
  private readonly potentiometerCaptain = BusSignal.from(this.sub.topic('potentiometerCaptain'), 0);
  private readonly potentiometerFo = BusSignal.from(this.sub.topic('potentiometerFo'), 0);
  private readonly spdTape = BusSignal.from(this.sub.topic('spdTape'), '');
  private readonly xWindSpdTape = BusSignal.from(this.sub.topic('xWindSpdTape'), '');
  private readonly altTape = BusSignal.from(this.sub.topic('altTape'), '');
  private readonly xWindAltTape = BusSignal.from(this.sub.topic('xWindAltTape'), '');
  private readonly windIndicator = BusSignal.from(this.sub.topic('windIndicator'), '');
  private readonly side = getDisplayIndex() === 1 ? 1 : 2;
  constructor(props: HUDProps) {
    super(props);
    this.failuresConsumer = new FailuresConsumer();
  }

  public afterRender(node: ViewNode): void {
    super.afterRender(node);
    this.subscriptions.push(
      this.hudLpos,
      this.hudRpos,
      this.potentiometerCaptain,
      this.potentiometerFo,
      this.spdTape,
      this.xWindSpdTape,
      this.altTape,
      this.xWindAltTape,
      this.windIndicator,
    );

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
      }),
    );
  }

  render(): ViewNode {
    return (
      <CdsDisplayUnit
        bus={this.props.bus}
        displayUnitId={getDisplayIndex() === 1 ? DisplayUnitID.CaptHud : DisplayUnitID.FoHud}
        test={Signal.of(-1)}
        failed={Signal.of(false)}
      >
        <svg
          class="hud-svg"
          version="1.1"
          viewBox="0 0 1280 1024"
          xmlns="http://www.w3.org/2000/svg"
          xmlnsXlink="http://www.w3.org/1999/xlink"
        >
          <Grid bus={this.props.bus} />

          <AltitudeIndicator bus={this.props.bus} />
          <AirspeedIndicator bus={this.props.bus} instrument={this.props.instrument} />
          <g id="TapesMasks2">
            <path
              id="Mask2Cw"
              class="LargeStroke BackgroundFill "
              display={this.xWindSpdTape}
              //d="M 95 0 H 240 V 1024 H 95 Z  M 96 238 v 182 h 143 v -182 z"
              d="M 95 0 H 235 V 238 H 95 Z M 95 420 H 235 V 1024 H 95 Z"
            />
            <path
              id="Mask2"
              class="LargeStroke BackgroundFill"
              display={this.spdTape}
              //d="M 95 0 H 237 V 1024 H 95 Z  M 96 329 v 383 h 140 v -383 z"
              d="M 95 0 H 235 V 329 H 95 Z M 95 712 H 235 V 1024 H 95 Z"
            />

            <path
              id="Mask3"
              class="LargeStroke BackgroundFill"
              display={this.altTape}
              //d="M 1028 0 h 115 V 1024 H 1028 Z  M 1029 329 v 383 h 113 v -383 z"
              d="M 1028 0 h 115 V 329 H 1028 Z M 1028 712 h 115 V 1024 H 1028 Z"
            />
            <path
              id="Mask4"
              class="LargeStroke BackgroundFill"
              display={this.xWindAltTape}
              //d="M 1028 0 h 115 V 1024 H 1028 Z  M 1029 254 v 152 h 113 v -152 z"
              d="M 1028 0 h 115 V 254 H 1028 Z M 1028 406 h 115 V 1024 H 1028 Z"
            />
          </g>
          <Horizon
            bus={this.props.bus}
            instrument={this.props.instrument}
            isAttExcessive={this.isAttExcessive}
            filteredRadioAlt={this.filteredRadioAltitude}
          />
          <AttitudeIndicatorFixedCenter
            bus={this.props.bus}
            isAttExcessive={this.isAttExcessive}
            filteredRadioAlt={this.filteredRadioAltitude}
            instrument={this.props.instrument}
          />
          <g id="WindIndicator" class="Wind" transform="translate(250 200) " display={this.windIndicator}>
            <WindIndicator bus={this.props.bus} />
          </g>

          <AirspeedIndicatorOfftape bus={this.props.bus} />

          <LandingSystem bus={this.props.bus} instrument={this.props.instrument} />
          <AttitudeIndicatorFixedUpper bus={this.props.bus} />
          <AttitudeIndicatorWarnings bus={this.props.bus} instrument={this.props.instrument} />
          <AttitudeIndicatorWarningsA380 bus={this.props.bus} instrument={this.props.instrument} />
          <HudWarnings bus={this.props.bus} instrument={this.props.instrument} />
          <VerticalSpeedIndicator
            bus={this.props.bus}
            instrument={this.props.instrument}
            filteredRadioAltitude={this.filteredRadioAltitude}
          />
          <AltitudeIndicatorOfftape bus={this.props.bus} filteredRadioAltitude={this.filteredRadioAltitude} />
          <LinearDeviationIndicator bus={this.props.bus} />
          <DecelIndicator bus={this.props.bus} instrument={this.props.instrument} />

          <MachNumber bus={this.props.bus} />

          <RadioNavInfo bus={this.props.bus} index={1} />
          <RadioNavInfo bus={this.props.bus} index={2} />
          <FMA bus={this.props.bus} isAttExcessive={this.isAttExcessive} />

          <DeclutterIndicator bus={this.props.bus} />
        </svg>
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
