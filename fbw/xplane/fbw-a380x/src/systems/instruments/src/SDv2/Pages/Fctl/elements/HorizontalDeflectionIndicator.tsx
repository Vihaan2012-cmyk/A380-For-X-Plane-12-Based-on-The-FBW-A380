import {
  View,
  UI,
  Derived,
  Readable,
  Logic,
} from 'xplane-js/ui';
import { RudderPosition } from './Rudder';

const SCALE_LENGTH = 116;
export const HORIZONTAL_MAX_DEFLECTION = 30;
export const HORIZONTAL_MIN_DEFLECTION = -30;

interface HorizontalDeflectionIndicationProps {
  x?: number;
  y?: number;
  powerAvail: Readable<boolean>;
  deflectionInfoValid: Readable<boolean>;
  deflection: Readable<number>;
  position: RudderPosition;
  onGround: Readable<boolean>;
}

/**
 * @param deflection Surface deflection.
 * @returns Screen position in px.
 */
export function deflectionToXOffset(deflection: number): number {
  const normalizedDeflection =
    deflection > 0 ? deflection / HORIZONTAL_MAX_DEFLECTION : -deflection / HORIZONTAL_MIN_DEFLECTION;

  return (normalizedDeflection * SCALE_LENGTH) / 2;
}

export class HorizontalDeflectionIndication extends View<HorizontalDeflectionIndicationProps> {
  private readonly deflectionXValue = this.props.deflection.derive((deflection) => deflectionToXOffset(deflection));

  private readonly maxDeflectionVisibility = Derived.from(
    ([onGround, deflectionInfoValid, powerAvail]) =>
      onGround && deflectionInfoValid && powerAvail ? 'inherit' : 'hidden',
    this.props.onGround,
    this.props.deflectionInfoValid,
    this.props.powerAvail,
  );

  private rudderTravelLimiter = 30;
  private rudderTravelLimXValue = deflectionToXOffset(this.rudderTravelLimiter);

  private readonly rudderTravelLimVisibility = Derived.from(
    ([onGround, deflectionInfoValid, powerAvail]) =>
      onGround && deflectionInfoValid && powerAvail ? 'inherit' : 'hidden',
    this.props.onGround,
    this.props.deflectionInfoValid,
    this.props.powerAvail,
  );

  render() {
    return (
      <g transform={`translate(${this.props.x} ${this.props.y})`}>
        <path class="Grey Fill" d="m0,0 h 116 v15 h-116 z" />

        <path class="Green SW2" visibility={this.maxDeflectionVisibility} d="m-1,0 v 15 M117,0 v 15" />
        <path
          class="Green NoFill SW3 LineRound"
          visibility={this.rudderTravelLimVisibility}
          d={`M ${58 - this.rudderTravelLimXValue + 4},0 h -5 v 12 M ${58 + this.rudderTravelLimXValue - 4},0 h 5 v 12`}
        />

        <path
          class={{
            Fill: true,
            Green: this.props.powerAvail,
            Amber: this.props.powerAvail.derive(Logic.not()),
          }}
          visibility={this.props.deflectionInfoValid.derive((deflectionInfoValid) =>
            deflectionInfoValid ? 'inherit' : 'hidden',
          )}
          d={this.deflectionXValue.derive((deflectionXValue) => `m58,0 v15 h${deflectionXValue} v-15 z`)}
        />
        {/* This is the small line in the middle of the scale, when the surface is neutral. */}
        <path
          class={{
            SW2: true,
            Green: this.props.powerAvail,
            Amber: this.props.powerAvail.derive(Logic.not()),
          }}
          visibility={this.props.deflectionInfoValid.derive((deflectionInfoValid) =>
            deflectionInfoValid ? 'inherit' : 'hidden',
          )}
          d="m58,0 v15"
        />

        <text
          x={49}
          y={this.props.position === RudderPosition.Upper ? 15 : 20}
          class="Amber F26"
          visibility={this.props.deflectionInfoValid.derive((deflectionInfoValid) =>
            !deflectionInfoValid ? 'inherit' : 'hidden',
          )}
        >
          X
        </text>
      </g>
    );
  }
}
