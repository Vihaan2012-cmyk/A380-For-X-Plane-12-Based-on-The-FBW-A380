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
import {
  ActuatorIndication,
  ActuatorType,
  ElecPowerSource,
  HydraulicPowerSource,
  powerSourceIsHydraulic,
} from './ActuatorIndication';
import { MIN_VERTICAL_DEFLECTION, VerticalDeflectionIndication } from './VerticalDeflectionIndication';
import { SDSimvars } from '../../../SDSimvarPublisher';

export enum AileronSide {
  Left = 'left',
  Right = 'right',
}

export enum AileronPosition {
  Inboard = 'Inner',
  Mid = 'Middle',
  Outboard = 'Outer',
}

interface AileronProps {
  x: number;
  y: number;
  side: AileronSide;
  position: AileronPosition;
  onGround: Readable<boolean>;
  bus: Bus;
}

export class Aileron extends View<AileronProps> {
  private readonly showAileronDroopSymbol = Signal.of(true);

  private readonly deflectionInfoValid = Signal.of(true);

  private readonly aileronDeflection = BusSignal.from(
    this.props.bus
      .inbox<SDSimvars>()
      .topic(`${this.props.side}${this.props.position}AileronDeflection`)
      .limit(10),
    0,
  );

  private readonly actuator1PowerSource: HydraulicPowerSource;

  private readonly actuator2PowerSource: HydraulicPowerSource | ElecPowerSource;

  private readonly powerSource1Avail: BusSignal<boolean>;

  private readonly powerSource2Avail: BusSignal<boolean>;

  private readonly powerAvail: Readable<boolean>;

  constructor(props: AileronProps) {
    super(props);

    if (props.position === AileronPosition.Outboard) {
      this.actuator1PowerSource = HydraulicPowerSource.Green;
      this.actuator2PowerSource = HydraulicPowerSource.Yellow;
    } else if (props.position === AileronPosition.Mid) {
      this.actuator1PowerSource = HydraulicPowerSource.Yellow;
      this.actuator2PowerSource = ElecPowerSource.AcEss;
    } else {
      this.actuator1PowerSource = HydraulicPowerSource.Green;
      this.actuator2PowerSource = ElecPowerSource.AcEha;
    }

    this.powerSource1Avail = BusSignal.from(
      this.props.bus.inbox<SDSimvars>().topic(`${this.actuator1PowerSource}PressureSwitch`),
      false,
    );

    this.powerSource2Avail = BusSignal.from(
      this.props.bus
        .inbox<SDSimvars>()
        .topic(
          powerSourceIsHydraulic(this.actuator2PowerSource)
            ? `${this.actuator2PowerSource}PressureSwitch`
            : `${this.actuator2PowerSource}Powered`,
        ),
      false,
    );

    this.powerAvail = Derived.from(
      Logic.or(),
      this.powerSource1Avail,
      this.powerSource2Avail,
    );
  }

  render() {
    let actuatorIndicationX: number;
    if (this.props.position === AileronPosition.Mid) {
      actuatorIndicationX = -5;
    } else if (
      (this.props.side === AileronSide.Left && this.props.position === AileronPosition.Outboard) ||
      (this.props.side === AileronSide.Right && this.props.position === AileronPosition.Inboard)
    ) {
      actuatorIndicationX = -13;
    } else {
      actuatorIndicationX = 2;
    }

    return (
      <g
        id={`aileron-${this.props.side}-${this.props.position}`}
        transform={`translate(${this.props.x} ${this.props.y})`}
      >
        <VerticalDeflectionIndication
          powerAvail={this.powerAvail}
          deflectionInfoValid={this.deflectionInfoValid}
          deflection={this.aileronDeflection.derive(
            (aileronDeflection) =>
              (this.props.side === AileronSide.Left ? -aileronDeflection : aileronDeflection) * MIN_VERTICAL_DEFLECTION,
          )}
          showAileronDroopSymbol={this.showAileronDroopSymbol}
          onGround={this.props.onGround}
        />

        <ActuatorIndication
          x={actuatorIndicationX}
          y={128}
          type={ActuatorType.Conventional}
          powerSource={this.actuator1PowerSource}
          powerSourceAvailable={this.powerSource1Avail}
        />
        <ActuatorIndication
          x={actuatorIndicationX}
          y={159}
          type={this.props.position === AileronPosition.Outboard ? ActuatorType.Conventional : ActuatorType.EHA}
          powerSource={this.actuator2PowerSource}
          powerSourceAvailable={this.powerSource2Avail}
        />
      </g>
    );
  }
}
