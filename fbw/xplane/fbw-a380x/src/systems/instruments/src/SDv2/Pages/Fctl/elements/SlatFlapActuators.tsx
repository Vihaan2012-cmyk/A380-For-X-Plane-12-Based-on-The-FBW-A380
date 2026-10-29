import { BusSignal, View, Bus, UI } from 'xplane-js/ui';
import { ActuatorIndication, ActuatorType, ElecPowerSource, HydraulicPowerSource } from './ActuatorIndication';
import { SDSimvars } from '../../../SDSimvarPublisher';

interface SlatFlapActuatorIndicationProps {
  x: number;
  y: number;
  type: 'SLATS' | 'FLAPS';
  bus: Bus;
}

export class SlatFlapActuatorIndication extends View<SlatFlapActuatorIndicationProps> {
  private readonly hydGreenAvailable = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic(`greenPressureSwitch`),
    false,
  );

  private readonly hydYellowAvailable = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic(`yellowPressureSwitch`),
    false,
  );

  private readonly elecAcEssAvailable = BusSignal.from(
    this.props.bus.inbox<SDSimvars>().topic(`acEssPowered`),
    false,
  );

  render() {
    return (
      <g id={`${this.props.type}-actuators`} transform={`translate(${this.props.x} ${this.props.y})`}>
        <path class="White SW1 LineRound" d="m10,0 h -10 v 52 h98 v-52 h-10" />

        <text x={13} y={10} class="F23 White">
          {this.props.type}
        </text>

        <ActuatorIndication
          x={19}
          y={17}
          type={this.props.type === 'SLATS' ? ActuatorType.EHA : ActuatorType.Conventional}
          powerSource={this.props.type === 'SLATS' ? ElecPowerSource.AcEss : HydraulicPowerSource.Green}
          powerSourceAvailable={this.props.type === 'SLATS' ? this.elecAcEssAvailable : this.hydGreenAvailable}
        />
        <ActuatorIndication
          x={53}
          y={17}
          type={ActuatorType.Conventional}
          powerSource={this.props.type === 'SLATS' ? HydraulicPowerSource.Green : HydraulicPowerSource.Yellow}
          powerSourceAvailable={this.props.type === 'SLATS' ? this.hydGreenAvailable : this.hydYellowAvailable}
        />
      </g>
    );
  }
}
