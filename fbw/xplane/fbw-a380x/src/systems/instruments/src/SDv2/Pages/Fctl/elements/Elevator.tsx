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
import { ActuatorIndication, ActuatorType, ElecPowerSource, HydraulicPowerSource } from './ActuatorIndication';
import { MIN_VERTICAL_DEFLECTION, VerticalDeflectionIndication } from './VerticalDeflectionIndication';
import { SDSimvars } from '../../../SDSimvarPublisher';

export enum ElevatorSide {
  Left = 'left',
  Right = 'right',
}

export enum ElevatorPosition {
  Inboard = 'Inner',
  Outboard = 'Outer',
}

interface ElevatorProps {
  x: number;
  y: number;
  side: ElevatorSide;
  position: ElevatorPosition;
  onGround: Readable<boolean>;
  bus: Bus;
}

export class Elevator extends View<ElevatorProps> {
  private readonly deflectionInfoValid = Signal.of(true);

  private readonly elevatorDeflection = BusSignal.from(
    this.props.bus
      .inbox<SDSimvars>()
      .topic(`${this.props.side}${this.props.position}ElevatorDeflection`)
      .limit(10),
    0,
  );

  private readonly hydPowerSource =
    this.props.side === ElevatorSide.Left ? HydraulicPowerSource.Green : HydraulicPowerSource.Yellow;

  private readonly elecPowerSource =
    (this.props.side === ElevatorSide.Left && this.props.position === ElevatorPosition.Outboard) ||
    (this.props.side === ElevatorSide.Right && this.props.position === ElevatorPosition.Inboard)
      ? ElecPowerSource.AcEha
      : ElecPowerSource.AcEss;

  private readonly hydPowerAvailable = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic(`${this.hydPowerSource}PressureSwitch`),
    false,
  );

  private readonly elecPowerAvailable = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic(`${this.elecPowerSource}Powered`),
    false,
  );

  private readonly powerAvail = Derived.from(
    Logic.or(),
    this.hydPowerAvailable,
    this.elecPowerAvailable,
  );

  render() {
    let actuatorIndicationX: number;
    if (
      (this.props.side === ElevatorSide.Left && this.props.position === ElevatorPosition.Outboard) ||
      (this.props.side === ElevatorSide.Right && this.props.position === ElevatorPosition.Inboard)
    ) {
      actuatorIndicationX = -13;
    } else {
      actuatorIndicationX = -2;
    }

    return (
      <g
        id={`elevator-${this.props.side}-${this.props.position}`}
        transform={`translate(${this.props.x} ${this.props.y})`}
      >
        <VerticalDeflectionIndication
          powerAvail={this.powerAvail}
          deflectionInfoValid={this.deflectionInfoValid}
          deflection={this.elevatorDeflection.derive((elevatorDeflection) => elevatorDeflection * MIN_VERTICAL_DEFLECTION)}
          onGround={this.props.onGround}
        />

        <ActuatorIndication
          x={actuatorIndicationX}
          y={131}
          type={ActuatorType.Conventional}
          powerSource={this.hydPowerSource}
          powerSourceAvailable={this.hydPowerAvailable}
        />
        <ActuatorIndication
          x={actuatorIndicationX}
          y={161}
          type={ActuatorType.EHA}
          powerSource={this.elecPowerSource}
          powerSourceAvailable={this.elecPowerAvailable}
        />
      </g>
    );
  }
}
