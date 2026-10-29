import {
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  Signal,
  Readable,
  Logic,
} from 'xplane-js/ui';
import { ActuatorIndication, ActuatorType, HydraulicPowerSource } from './ActuatorIndication';
import { SDSimvars } from '../../../SDSimvarPublisher';
import { MathUtils } from '@flybywiresim/fbw-sdk';

interface PitchTrimProps {
  x: number;
  y: number;
  onGround: Readable<boolean>;
  bus: Bus;
}

export class PitchTrim extends View<PitchTrimProps> {
  private readonly positionInfoValid = Signal.of(true);

  private readonly thsPositionRadians = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic(`thsDeflection`).limit(10),
    0,
  );

  private readonly thsPosition = this.thsPositionRadians.derive(
    (thsPositionRadians) => thsPositionRadians * MathUtils.RADIANS_TO_DEGREES,
  );

  private readonly thsPositionSplit = this.thsPosition.derive((thsPosition) =>
    Math.abs(thsPosition).toFixed(1).split('.'),
  );

  private readonly thsJam = Signal.of(false);

  private readonly hydGreenAvailable = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic(`greenPressureSwitch`),
    false,
  );

  private readonly hydYellowAvailable = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic(`yellowPressureSwitch`),
    false,
  );

  private readonly hydraulicAvailable = Derived.from(
    Logic.or(),
    this.hydGreenAvailable,
    this.hydYellowAvailable,
  );

  private readonly pitchTrimValueClass = this.hydraulicAvailable.derive((hydraulicAvailable) =>
    hydraulicAvailable ? 'Green F26' : 'Amber F26',
  );

  private readonly pitchTrimTitleClass = Derived.from(
    ([hydGreenAvailable, hydYellowAvailable, thsJam]) =>
      (hydGreenAvailable || hydYellowAvailable) && !thsJam ? 'F22 MiddleAlign LS1 White' : 'F22 MiddleAlign LS1 Amber',
    this.hydGreenAvailable,
    this.hydYellowAvailable,
    this.thsJam,
  );

  render() {
    return (
      <g id="ths" transform={`translate(${this.props.x} ${this.props.y})`}>
        <path class="White SW4 LineRound" d="m0,0 v119 M-10,0 h20 M-10,118 h20 M-10,98 h20" />
        <path
          class={{
            SW2: true,
            LineRound: true,
            LineJoinRound: true,
            NoFill: true,
            Green: this.hydraulicAvailable,
            Amber: this.hydraulicAvailable.derive(Logic.not()),
          }}
          d="m-5,98 l-21,-11 v23 l21,-11 z"
          transform={this.thsPosition.derive((thsPosition) => `translate (0 ${-thsPosition * 10})`)}
        />

        <text x={57} y={-6} class={this.pitchTrimTitleClass}>
          PITCH
        </text>
        <text x={57} y={17} class={this.pitchTrimTitleClass}>
          TRIM
        </text>

        <g visibility={this.positionInfoValid.derive((positionInfoValid) => (positionInfoValid ? 'visible' : 'hidden'))}>
          <text
            x={38}
            y={67}
            class={this.pitchTrimValueClass.derive((pitchTrimValueClass) => pitchTrimValueClass + ' EndAlign')}
          >
            {this.thsPositionSplit.derive((thsPositionSplit) => thsPositionSplit[0])}
          </text>
          <text x={37} y={67} class={this.pitchTrimValueClass}>
            .
          </text>
          <text x={55} y={67} class={this.pitchTrimValueClass}>
            {this.thsPositionSplit.derive((thsPositionSplit) => thsPositionSplit[1])}
          </text>
          <text x={65} y={68} class="Cyan F26">
            °
          </text>
          <text
            x={82}
            y={68}
            visibility={this.thsPosition.derive((thsPosition) => (Math.abs(thsPosition) > 0.05 ? 'visible' : 'hidden'))}
            class={this.pitchTrimValueClass}
          >
            {this.thsPosition.derive((thsPosition) => (Math.sign(thsPosition) === 1 ? 'UP' : 'DN'))}
          </text>
        </g>

        <text
          x={26}
          y={68}
          visibility={this.positionInfoValid.derive((positionInfoValid) => (!positionInfoValid ? 'visible' : 'hidden'))}
          class="Amber F26"
        >
          XX
        </text>

        <ActuatorIndication
          x={-63}
          y={24}
          type={ActuatorType.Conventional}
          powerSource={HydraulicPowerSource.Green}
          powerSourceAvailable={this.hydGreenAvailable}
        />
        <ActuatorIndication
          x={-63}
          y={66}
          type={ActuatorType.Conventional}
          powerSource={HydraulicPowerSource.Yellow}
          powerSourceAvailable={this.hydYellowAvailable}
        />
      </g>
    );
  }
}
