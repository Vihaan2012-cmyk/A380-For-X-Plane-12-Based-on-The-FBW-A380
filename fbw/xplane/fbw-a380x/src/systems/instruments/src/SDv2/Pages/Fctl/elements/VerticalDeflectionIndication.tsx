import {
  View,
  UI,
  Derived,
  Readable,
  Logic,
  Readables,
} from 'xplane-js/ui';

const SCALE_HEIGHT = 116;
export const MAX_VERTICAL_DEFLECTION = 20;
export const MIN_VERTICAL_DEFLECTION = -30;

interface VerticalDeflectionIndicationProps {
  x?: number;
  y?: number;
  powerAvail: Readable<boolean>;
  deflectionInfoValid: Readable<boolean>;
  deflection: Readable<number>;
  showAileronDroopSymbol?: Readable<boolean>;
  onGround: Readable<boolean>;
}

/**
 * @param deflection Surface deflection.
 * @returns Screen position in px.
 */
function deflectionToYOffset(deflection: number): number {
  const normalizedDeflection =
    deflection > 0 ? deflection / MAX_VERTICAL_DEFLECTION : -deflection / MIN_VERTICAL_DEFLECTION;

  return (normalizedDeflection * SCALE_HEIGHT) / 2;
}

export class VerticalDeflectionIndication extends View<VerticalDeflectionIndicationProps> {
  private readonly deflectionXValue = this.props.deflection.derive((deflection) => deflectionToYOffset(deflection));

  private readonly maxDeflectionVisibility = Derived.from(
    ([onGround, deflectionInfoValid, powerAvail]) =>
      onGround && deflectionInfoValid && powerAvail ? 'inherit' : 'hidden',
    this.props.onGround,
    this.props.deflectionInfoValid,
    this.props.powerAvail,
  );

  private readonly droopSymbolVisibility = Derived.from(
    ([showAileronDroopSymbol, deflectionInfoValid]) =>
      showAileronDroopSymbol && deflectionInfoValid ? 'inherit' : 'hidden',
    Readables.toReadable(this.props.showAileronDroopSymbol ?? false, true),
    this.props.deflectionInfoValid,
  );

  render() {
    return (
      <g transform={`translate(${this.props.x} ${this.props.y})`}>
        <path class="Grey Fill" d="m0,0 v 116 h15 v-116 z" />

        <circle
          class="SW2 White"
          visibility={this.droopSymbolVisibility}
          cx={7.5}
          cy={deflectionToYOffset(5) + SCALE_HEIGHT / 2}
          r={3.75}
        />

        <path class="Green SW2" visibility={this.maxDeflectionVisibility} d="m0,-1 h 15 M0,117 h15" />

        <path
          class={{
            Fill: true,
            Green: this.props.powerAvail,
            Amber: this.props.powerAvail.derive(Logic.not()),
          }}
          visibility={this.props.deflectionInfoValid.derive((deflectionInfoValid) =>
            deflectionInfoValid ? 'inherit' : 'hidden',
          )}
          d={this.deflectionXValue.derive((deflectionXValue) => `m0,58 h15 v${deflectionXValue} h-15 z`)}
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
          d="m0,58 h15"
        />

        <text
          x={-1}
          y={70}
          class="Amber F32"
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
