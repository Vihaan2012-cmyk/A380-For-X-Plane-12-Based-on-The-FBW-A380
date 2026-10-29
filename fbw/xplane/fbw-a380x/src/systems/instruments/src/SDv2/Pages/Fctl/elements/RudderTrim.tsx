import {
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  Signal,
  ViewNode,
} from 'xplane-js/ui';
import { deflectionToXOffset } from './HorizontalDeflectionIndicator';
import { Arinc429LocalVarConsumerSubject } from '@flybywiresim/fbw-sdk';
import { SDSimvars } from '../../../SDSimvarPublisher';

export enum RudderPosition {
  Upper,
  Lower,
}

interface RudderTrimProps {
  x: number;
  y: number;
  bus: Bus;
}

export class RudderTrim extends View<RudderTrimProps> {
  private readonly sub = this.props.bus.inbox<SDSimvars>();

  private readonly sec1RudderStatusWord = Arinc429LocalVarConsumerSubject.create(this.sub.topic('sec1RudderStatusWord'));

  private readonly sec3RudderStatusWord = Arinc429LocalVarConsumerSubject.create(this.sub.topic('sec3RudderStatusWord'));

  private readonly secSourceForTrim = this.sec1RudderStatusWord.derive((word) => (word.bitValueOr(28, false) ? 1 : 3));

  private readonly deflectionInfoValid = Derived.from(
    ([sec1RudderStatusWord, sec3RudderStatusWord]) =>
      sec1RudderStatusWord.bitValueOr(28, false) || sec3RudderStatusWord.bitValueOr(28, false),
    this.sec1RudderStatusWord,
    this.sec3RudderStatusWord,
  );

  private readonly rudderTrimAvail = Signal.of(true);

  private readonly rudderTrim = Arinc429LocalVarConsumerSubject.create(null);

  private readonly powerSource1Avail = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic(`dcEssPowered`),
    false,
  );

  private readonly powerSource2Avail = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic(`dc1Powered`),
    false,
  );

  private readonly leftRightLabelVisible = Derived.from(
    ([rudderTrim, deflectionInfoValid]) => Math.abs(rudderTrim.valueOr(0)) > 0.05 && deflectionInfoValid,
    this.rudderTrim,
    this.deflectionInfoValid,
  );

  private readonly powerAvailableClass = Derived.from(
    ([powerSource1Avail, powerSource2Avail, rudderTrimAvail]) =>
      (powerSource1Avail || powerSource2Avail) && rudderTrimAvail ? 'Cyan' : 'Amber',
    this.powerSource1Avail,
    this.powerSource2Avail,
    this.rudderTrimAvail,
  );

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.secSourceForTrim.watch(
      (source) => this.rudderTrim.setConsumer(this.sub.topic(`sec${source}RudderTrimPosition`).limit(10)),
      true,
    );
  }

  render() {
    return (
      <g id="rudder-trim" transform={`translate(${this.props.x} ${this.props.y})`}>
        <text x={70} y={-8} class="F22 White LS1">
          RUD TRIM
        </text>

        <g
          visibility={this.deflectionInfoValid.derive((deflectionInfoValid) =>
            deflectionInfoValid ? 'visible' : 'hidden',
          )}
        >
          {/* This is to occlude part of the tail graphic. */}
          <path d="m-5,0 h8 v17 h-8 z" class="BackgroundFill" />

          <path
            d="m0,0 l6,8 l-6,8 l-6,-8 z"
            class={this.powerAvailableClass.derive((powerAvailableClass) => `${powerAvailableClass} Fill`)}
            transform={this.rudderTrim.derive((rudderTrim) => `translate(${deflectionToXOffset(-rudderTrim.value)} 0)`)}
          />

          <text
            x={72}
            y={17}
            visibility={this.leftRightLabelVisible.derive((leftRightLabelVisible) =>
              leftRightLabelVisible ? 'visible' : 'hidden',
            )}
            class={this.powerAvailableClass.derive((powerAvailableClass) => `${powerAvailableClass} F22`)}
          >
            {this.rudderTrim.derive((rudderTrim) => (Math.sign(rudderTrim.valueOr(0)) === 1 ? 'L' : 'R'))}
          </text>
          <text
            x={159}
            y={17}
            class={this.powerAvailableClass.derive((powerAvailableClass) => `${powerAvailableClass} F22 EndAlign`)}
          >
            {this.rudderTrim.derive((rudderTrim) => Math.abs(rudderTrim.valueOr(0)).toFixed(1).padStart(4, '\xa0'))}
          </text>
          <text x={159} y={19} class="Cyan F22">
            °
          </text>
        </g>

        <text
          x={94}
          y={17}
          visibility={this.deflectionInfoValid.derive((deflectionInfoValid) =>
            !deflectionInfoValid ? 'visible' : 'hidden',
          )}
          class="Amber F22"
        >
          XX
        </text>
        <text
          x={-7}
          y={18}
          visibility={this.deflectionInfoValid.derive((deflectionInfoValid) =>
            !deflectionInfoValid ? 'visible' : 'hidden',
          )}
          class="Amber F22"
        >
          X
        </text>
      </g>
    );
  }
}
