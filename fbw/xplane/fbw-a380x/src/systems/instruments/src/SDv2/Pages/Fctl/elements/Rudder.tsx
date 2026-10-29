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
import { EbhaActuatorIndication, ElecPowerSource, HydraulicPowerSource } from './ActuatorIndication';
import { HORIZONTAL_MAX_DEFLECTION, HorizontalDeflectionIndication } from './HorizontalDeflectionIndicator';
import { SDSimvars } from '../../../SDSimvarPublisher';

export enum RudderPosition {
  Upper = 'upper',
  Lower = 'lower',
}

interface RudderProps {
  x: number;
  y: number;
  position: RudderPosition;
  onGround: Readable<boolean>;
  bus: Bus;
}

export class Rudder extends View<RudderProps> {
  private readonly deflectionInfoValid = Signal.of(true);

  private readonly rudderDeflection = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic(`${this.props.position}RudderDeflection`).limit(10),
    0,
  );

  private readonly hydGreenAvailable = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic(`greenPressureSwitch`),
    false,
  );

  private readonly hydYellowAvailable = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic(`yellowPressureSwitch`),
    false,
  );

  private readonly elecAc1Available = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic(`ac1Powered`),
    false,
  );

  private readonly elecAcEhaAvailable = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic(`acEhaPowered`),
    false,
  );

  private readonly elecAcEssAvailable = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic(`acEssPowered`),
    false,
  );

  private readonly powerSource1Avail = Derived.from(
    ([elecAcEssAvailable, hydYellowAvailable, hydGreenAvailable]) =>
      this.props.position === RudderPosition.Upper
        ? elecAcEssAvailable || hydYellowAvailable
        : elecAcEssAvailable || hydGreenAvailable,
    this.elecAcEssAvailable,
    this.hydYellowAvailable,
    this.hydGreenAvailable,
  );

  private readonly powerSource2Avail = Derived.from(
    ([elecAcEhaAvailable, elecAc1Available, hydYellowAvailable, hydGreenAvailable]) =>
      this.props.position === RudderPosition.Upper
        ? elecAcEhaAvailable || hydGreenAvailable
        : elecAc1Available || hydYellowAvailable,
    this.elecAcEhaAvailable,
    this.elecAc1Available,
    this.hydYellowAvailable,
    this.hydGreenAvailable,
  );

  private readonly powerSourceAvail = Derived.from(
    Logic.or(),
    this.powerSource1Avail,
    this.powerSource2Avail,
  );

  render() {
    return (
      <g id={`rudder-${this.props.position}`} transform={`translate(${this.props.x} ${this.props.y})`}>
        <HorizontalDeflectionIndication
          powerAvail={this.powerSourceAvail}
          deflectionInfoValid={this.deflectionInfoValid}
          deflection={this.rudderDeflection.derive((rudderDeflection) => rudderDeflection * HORIZONTAL_MAX_DEFLECTION)}
          position={this.props.position}
          onGround={this.props.onGround}
        />

        <EbhaActuatorIndication
          x={-60}
          y={this.props.position === RudderPosition.Upper ? -39 : -2}
          hydraulicPowerSource={
            this.props.position === RudderPosition.Upper ? HydraulicPowerSource.Yellow : HydraulicPowerSource.Green
          }
          elecPowerSource={ElecPowerSource.AcEss}
          hydPowerAvailable={
            this.props.position === RudderPosition.Upper ? this.hydYellowAvailable : this.hydGreenAvailable
          }
          elecPowerAvailable={this.elecAcEssAvailable}
        />
        <EbhaActuatorIndication
          x={-60}
          y={this.props.position === RudderPosition.Upper ? -8 : 30}
          hydraulicPowerSource={
            this.props.position === RudderPosition.Upper ? HydraulicPowerSource.Green : HydraulicPowerSource.Yellow
          }
          elecPowerSource={this.props.position === RudderPosition.Upper ? ElecPowerSource.AcEha : ElecPowerSource.Ac1}
          hydPowerAvailable={
            this.props.position === RudderPosition.Upper ? this.hydGreenAvailable : this.hydYellowAvailable
          }
          elecPowerAvailable={
            this.props.position === RudderPosition.Upper ? this.elecAcEhaAvailable : this.elecAc1Available
          }
        />
      </g>
    );
  }
}
