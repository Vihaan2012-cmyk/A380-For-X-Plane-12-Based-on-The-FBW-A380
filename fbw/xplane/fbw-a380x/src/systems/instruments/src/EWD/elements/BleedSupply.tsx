import { Arinc429ConsumerSubject } from '@flybywiresim/fbw-sdk';
import {
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  Readable,
  Logic,
  ViewNode,
} from 'xplane-js/ui';
import { Arinc429Values } from '../shared/ArincValueProvider';
import { EwdSimvars } from '../shared/EwdSimvarPublisher';

interface BleedSupplyProps {
  bus: Bus;
  x: number;
  y: number;
  hidden: Readable<boolean>;
}

export class BleedSupply extends View<BleedSupplyProps> {
  private readonly sub = this.props.bus.inbox<EwdSimvars & Arinc429Values>();

  private readonly cpiomBAgsDiscrete = Arinc429ConsumerSubject.create(undefined);
  private readonly noseGearCompressedSys1 = BusSignal.from(
    this.sub.topic('nose_gear_compressed_1').distinct(),
    false,
  );
  private readonly noseGearCompressedSys2 = BusSignal.from(
    this.sub.topic('nose_gear_compressed_2').distinct(),
    false,
  );
  private readonly onGround = Derived.from(
    Logic.or(),
    this.noseGearCompressedSys1,
    this.noseGearCompressedSys2,
  );

  private readonly engine1State = BusSignal.from(this.sub.topic('engine_state_1').distinct(), 0);
  private readonly engine2State = BusSignal.from(this.sub.topic('engine_state_2').distinct(), 0);
  private readonly engine3State = BusSignal.from(this.sub.topic('engine_state_3').distinct(), 0);
  private readonly engine4State = BusSignal.from(this.sub.topic('engine_state_4').distinct(), 0);

  private readonly nai1 = BusSignal.from(this.sub.topic('eng_anti_ice_1').distinct(), false);
  private readonly nai2 = BusSignal.from(this.sub.topic('eng_anti_ice_2').distinct(), false);
  private readonly nai3 = BusSignal.from(this.sub.topic('eng_anti_ice_3').distinct(), false);
  private readonly nai4 = BusSignal.from(this.sub.topic('eng_anti_ice_4').distinct(), false);
  private readonly wai = BusSignal.from(this.sub.topic('wing_anti_ice').distinct(), false);
  private readonly thrustLimit = BusSignal.from(this.sub.topic('thrust_limit_type').distinct(), 0);

  private readonly oneEngineRunning = Derived.from(
    ([eng1State, eng2State, eng3State, eng4State]) =>
      eng1State === 1 || eng2State === 1 || eng3State === 1 || eng4State === 1,
    this.engine1State,
    this.engine2State,
    this.engine3State,
    this.engine4State,
  );

  private readonly bleedSupplyVisible = Derived.from(
    ([onGround, thrustLimit, oneEngineRunning, revSelected]) =>
      !revSelected && ((onGround && oneEngineRunning) || (!onGround && thrustLimit >= 2)), // MCT, FLEX TOGA
    this.onGround,
    this.thrustLimit,
    this.oneEngineRunning,
    this.props.hidden,
  );

  private readonly bleedText = Derived.from(
    ([cpiomB, nai1, nai2, nai3, nai4, wai, visible]) => {
      const text = [];
      if (visible) {
        if (cpiomB.bitValueOr(13, false) || cpiomB.bitValueOr(14, false)) {
          text.push('PACKS');
        }

        if (nai1 || nai2 || nai3 || nai4) {
          text.push('NAI');
        }

        if (wai) {
          text.push('WAI');
        }
      }
      return text.join('/');
    },
    this.cpiomBAgsDiscrete,
    this.nai1,
    this.nai2,
    this.nai3,
    this.nai4,
    this.wai,
    this.bleedSupplyVisible,
  );

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.cpiomBAgsDiscrete.setConsumer(this.sub.topic('cpiomBAgsDiscrete'));
  }

  render() {
    return (
      <text x={this.props.x} y={this.props.y} class="Green F26 End">
        {this.bleedText}
      </text>
    );
  }
}
