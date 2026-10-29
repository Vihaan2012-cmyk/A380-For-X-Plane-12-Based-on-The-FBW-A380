// @ts-strict-ignore
import {
  BusSignal,
  Transforms,
  View,
  Bus,
  UI,
  Derived,
  Signal,
  Logic,
  ViewNode,
} from 'xplane-js/ui';
import { Arinc429ConsumerSubject, Arinc429LocalVarConsumerSubject } from '@flybywiresim/fbw-sdk';
import { calculateHorizonOffsetFromPitch } from './PFDUtils';
import { Arinc429Values } from './shared/ArincValueProvider';
import { getDisplayIndex } from './PFD';
import { FcuEfisCpBusEvents } from '@shared/publishers/EfisCpBusPublisher';
import { PrimFgBusBaseEvents } from '@shared/publishers/PrimFgPublisher';
import { SelectedFdEvents } from './shared/FdSelectionProvider';

const DistanceSpacing = 15;
const ValueSpacing = 10;

export class FlightPathVector extends View<{ bus: Bus }> {
  private static readonly BIRD_CIRCLE = 'm17.75 15.5 a2.25 2.25 0 1 0 -4.5 0 a2.25 2.25 0 1 0 4.5 0';
  private static readonly BIRD_WINGS = 'm17.75 15.5 h5 m-9.5 0 h-5 m7.25 -2.25 v-2';

  private static readonly BIRD_CIRCLE_SMALL = 'm17 15.5 a1.5 1.5 0 1 0 -3 0 a1.5 1.5 0 1 0 3 0';
  private static readonly BIRD_WINGS_SMALL = 'm17 15.5 h2.5 m-5.5 0 h-2.5 m4 -1.5 v-1.5';

  private readonly sub = this.props.bus.inbox<
    Arinc429Values & PrimFgBusBaseEvents & FcuEfisCpBusEvents & SelectedFdEvents
  >();

  private readonly fcuEisDiscreteWord2 = Arinc429LocalVarConsumerSubject.create(null);

  private primFgDiscreteWord5 = Arinc429LocalVarConsumerSubject.create(this.sub.topic('prim_fg_discrete_word_5'));

  private readonly fdEngaged = BusSignal.from(this.sub.topic('fd_engaged'), false);

  private readonly isTrkFpaActive = this.primFgDiscreteWord5.derive((word) => word.bitValueOr(11, true));
  private readonly isBirdBlack = this.isTrkFpaActive.derive(Logic.not());

  private readonly isVelocityVectorActive = this.fcuEisDiscreteWord2.derive((word) => word.bitValueOr(15, true));

  private readonly roll = Arinc429ConsumerSubject.create(this.sub.topic('rollAr'));
  private readonly pitch = Arinc429ConsumerSubject.create(this.sub.topic('pitchAr'));
  private readonly fpa = Arinc429ConsumerSubject.create(this.sub.topic('fpa'));
  private readonly da = Arinc429ConsumerSubject.create(this.sub.topic('da'));

  private readonly isRequested = Derived.from(
    Logic.or(),
    this.isTrkFpaActive,
    this.isVelocityVectorActive,
  );

  private readonly isDaAndFpaValid = Derived.from(
    ([da, fpa]) => da.isNormalOperation() && fpa.isNormalOperation(),
    this.da,
    this.fpa,
  );
  private readonly isRollAndPitchValid = Derived.from(
    ([roll, pitch]) => roll.isNormalOperation() && pitch.isNormalOperation(),
    this.roll,
    this.pitch,
  );

  private readonly isBirdHidden = Derived.from(
    ([isReq, isValid]) => !isReq || !isValid,
    this.isRequested,
    this.isDaAndFpaValid,
  );

  private readonly isFailureFlagHidden = Derived.from(
    ([isRequested, isDaAndFpaValid, isRollAndPitchValid]) => !isRequested || isDaAndFpaValid || !isRollAndPitchValid,
    this.isRequested,
    this.isDaAndFpaValid,
    this.isRollAndPitchValid,
  );

  private readonly birdTransformBuilder = Transforms.translate3d('px');
  private readonly birdTransform = Signal.of(this.birdTransformBuilder.css());

  private readonly birdCirclePath = this.fdEngaged.derive((fdEngaged) =>
    fdEngaged ? FlightPathVector.BIRD_CIRCLE_SMALL : FlightPathVector.BIRD_CIRCLE,
  );

  private readonly birdWingsPath = this.fdEngaged.derive((fdEngaged) =>
    fdEngaged ? FlightPathVector.BIRD_WINGS_SMALL : FlightPathVector.BIRD_WINGS,
  );

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const isFo = getDisplayIndex() === 2;

    this.fcuEisDiscreteWord2.setConsumer(
      this.sub.topic(isFo ? 'fcu_efis_r_discrete_word_2' : 'fcu_efis_l_discrete_word_2'),
    );

    const moveBirdSub = Derived.from(this.roll, this.pitch, this.fpa, this.da).watch(
      this.moveBird.bind(this),
      true,
      true,
    );

    this.isBirdHidden.watch((isHidden) => {
      if (isHidden) {
        moveBirdSub.hold();
      } else {
        moveBirdSub.release(true);
      }
    }, true);
  }

  private moveBird() {
    const daLimConv = (Math.max(Math.min(this.da.get().value, 21), -21) * DistanceSpacing) / ValueSpacing;
    const pitchSubFpaConv =
      calculateHorizonOffsetFromPitch(this.pitch.get().value) - calculateHorizonOffsetFromPitch(this.fpa.get().value);
    const rollCos = Math.cos((this.roll.get().value * Math.PI) / 180);
    const rollSin = Math.sin((-this.roll.get().value * Math.PI) / 180);

    const xOffset = daLimConv * rollCos - pitchSubFpaConv * rollSin;
    const yOffset = pitchSubFpaConv * rollCos + daLimConv * rollSin;

    this.birdTransformBuilder.set(xOffset, yOffset, 0, 0.01, 0.01);
    this.birdTransform.set(this.birdTransformBuilder.css());
  }

  render(): ViewNode {
    return (
      <>
        <g
          id="bird"
          style={{
            transform: this.birdTransform,
          }}
          class={{
            HiddenElement: this.isBirdHidden,
          }}
        >
          <svg
            x="53.4"
            y="65.3"
            width="31px"
            height="31px"
            version="1.1"
            viewBox="0 0 31 31"
            xmlns="http://www.w3.org/2000/svg"
          >
            <g>
              <path
                class={{
                  NormalOutline: true,
                  White: this.isBirdBlack,
                }}
                d={this.birdCirclePath}
              />
              <path
                class={{
                  NormalOutline: true,
                  White: this.isBirdBlack,
                }}
                d={this.birdWingsPath}
              />
              <path
                class={{
                  NormalStroke: true,
                  Green: this.isTrkFpaActive,
                  Black: this.isBirdBlack,
                }}
                d={this.birdCirclePath}
              />
              <path
                class={{
                  NormalStroke: true,
                  Green: this.isTrkFpaActive,
                  Black: this.isBirdBlack,
                }}
                d={this.birdWingsPath}
              />
            </g>
          </svg>
        </g>
        <text
          id="FPVFlag"
          x="62.987099"
          y="89.42025"
          class={{
            HiddenElement: this.isFailureFlagHidden,
            Blink9Seconds: true,
            FontLargest: true,
            Red: true,
            EndAlign: true,
          }}
        >
          FPV
        </text>
      </>
    );
  }
}
