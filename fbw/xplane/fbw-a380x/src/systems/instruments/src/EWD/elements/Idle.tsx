import { BusSignal, View, Bus, UI, Derived } from 'xplane-js/ui';
import { EwdSimvars } from '../shared/EwdSimvarPublisher';

interface IdleProps {
  bus: Bus;
  x: number;
  y: number;
}

export class Idle extends View<IdleProps> {
  private readonly sub = this.props.bus.inbox<EwdSimvars>();

  private readonly n1 = [
    BusSignal.from(this.sub.topic(`n1_1`).rounded(1).distinct(), 0),
    BusSignal.from(this.sub.topic(`n1_2`).rounded(1).distinct(), 0),
    BusSignal.from(this.sub.topic(`n1_3`).rounded(1).distinct(), 0),
    BusSignal.from(this.sub.topic(`n1_4`).rounded(1).distinct(), 0),
  ];

  private readonly fwcFlightPhase = BusSignal.from(this.sub.topic('fwc_flight_phase').distinct(), 0);

  private readonly autothrustStatus = BusSignal.from(this.sub.topic('autothrustStatus').distinct(), 0);

  private readonly n1Idle = BusSignal.from(this.sub.topic('n1Idle').rounded(1).distinct(), 0);

  private readonly visible = Derived.from(
    ([e1, e2, e3, e4, fp, as, idle]) => {
      const idleBuf = idle + 2;
      const enginesIdle = [e1 <= idleBuf, e2 <= idleBuf, e3 <= idleBuf, e4 <= idleBuf];
      return enginesIdle.filter(Boolean).length >= 3 && fp >= 6 && fp <= 9 && as !== 0 ? 'inherit' : 'hidden';
    },
    this.n1[0],
    this.n1[1],
    this.n1[2],
    this.n1[3],
    this.fwcFlightPhase,
    this.autothrustStatus,
    this.n1Idle,
  );

  render() {
    return (
      <text x={this.props.x} y={this.props.y} class={`IdleTextPulse Green F26 Center`} visibility={this.visible}>
        IDLE
      </text>
    );
  }
}
