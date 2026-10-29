import { BusSignal, Bus, UI, Derived, ViewNode } from 'xplane-js/ui';
import { DestroyableComponent } from '@flybywiresim/msfs-avionics-common';
import { SDSimvars } from '../../../SDSimvarPublisher';

export interface CruiseCondProps {
  readonly bus: Bus;
}

export class CruiseCond extends DestroyableComponent<CruiseCondProps> {
  private readonly sub = this.props.bus.inbox<SDSimvars>();

  private readonly cockpitCabinTemp = BusSignal.from(this.sub.topic('cockpitCabinTemp'), 0);
  private readonly cockpitCabinTempFormatted = this.cockpitCabinTemp.derive((v) => v.toFixed(0));

  private readonly fwdCargoTemp = BusSignal.from(this.sub.topic('fwdCargoTemp'), 0);
  private readonly fwdCargoTempFormatted = this.cockpitCabinTemp.derive((v) => v.toFixed(0));

  private readonly aftCargoTemp = BusSignal.from(this.sub.topic('aftCargoTemp'), 0);
  private readonly aftCargoTempFormatted = this.cockpitCabinTemp.derive((v) => v.toFixed(0));

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subscriptions.push(
      this.cockpitCabinTemp,
      this.cockpitCabinTempFormatted,
      this.fwdCargoTemp,
      this.fwdCargoTempFormatted,
      this.aftCargoTemp,
      this.aftCargoTempFormatted,
    );
  }

  render() {
    return (
      <>
        <text class="F22 Cyan" x="19" y="506">
          °C
        </text>
        <text id="CockpitTemp" class="F29 Green EndAlign" x="109" y="528">
          {this.cockpitCabinTempFormatted}
        </text>

        <CabinTemperatures bus={this.props.bus} />

        {/* Cargo Temps */}
        <text id="fwdCargoTemp" class="F29 Green EndAlign" x="178" y="595">
          {this.fwdCargoTempFormatted}
        </text>
        <text id="AftCargoTemp" class="F29 Green EndAlign" x="378" y="595">
          {this.aftCargoTempFormatted}
        </text>
      </>
    );
  }
}

export class CabinTemperatures extends DestroyableComponent<CruiseCondProps> {
  private readonly sub = this.props.bus.inbox<SDSimvars>();

  private readonly mainDeckTemp: BusSignal<number>[] = [
    BusSignal.from(this.sub.topic('condMainDeckTemp_1'), 0),
    BusSignal.from(this.sub.topic('condMainDeckTemp_2'), 0),
    BusSignal.from(this.sub.topic('condMainDeckTemp_3'), 0),
    BusSignal.from(this.sub.topic('condMainDeckTemp_4'), 0),
    BusSignal.from(this.sub.topic('condMainDeckTemp_5'), 0),
    BusSignal.from(this.sub.topic('condMainDeckTemp_6'), 0),
    BusSignal.from(this.sub.topic('condMainDeckTemp_7'), 0),
    BusSignal.from(this.sub.topic('condMainDeckTemp_8'), 0),
  ];

  private readonly upperDeckTemp: BusSignal<number>[] = [
    BusSignal.from(this.sub.topic('condUpperDeckTemp_1'), 0),
    BusSignal.from(this.sub.topic('condUpperDeckTemp_2'), 0),
    BusSignal.from(this.sub.topic('condUpperDeckTemp_3'), 0),
    BusSignal.from(this.sub.topic('condUpperDeckTemp_4'), 0),
    BusSignal.from(this.sub.topic('condUpperDeckTemp_5'), 0),
    BusSignal.from(this.sub.topic('condUpperDeckTemp_6'), 0),
    BusSignal.from(this.sub.topic('condUpperDeckTemp_7'), 0),
  ];

  private readonly minMaxMainDeckTemp = Derived.from(
    (temps) => {
      return [Math.min(...temps), Math.max(...temps)];
    },
    ...this.mainDeckTemp,
  );
  private readonly minMainDeckTemp = this.minMaxMainDeckTemp.derive((v) => v[0].toFixed(0));
  private readonly maxMainDeckTemp = this.minMaxMainDeckTemp.derive((v) => v[1].toFixed(0));

  private readonly minMaxUpperDeckTemp = Derived.from(
    (temps) => {
      return [Math.min(...temps), Math.max(...temps)];
    },
    ...this.upperDeckTemp,
  );
  private readonly minUpperDeckTemp = this.minMaxUpperDeckTemp.derive((v) => v[0].toFixed(0));
  private readonly maxUpperDeckTemp = this.minMaxUpperDeckTemp.derive((v) => v[1].toFixed(0));

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subscriptions.push(
      ...this.mainDeckTemp,
      ...this.upperDeckTemp,
      this.minMaxMainDeckTemp,
      this.minMainDeckTemp,
      this.maxMainDeckTemp,
      this.minMaxUpperDeckTemp,
      this.minUpperDeckTemp,
      this.maxUpperDeckTemp,
    );
  }

  render() {
    return (
      <>
        <text id="ForwardUpperTemp" class="F29 Green EndAlign" x="210" y="506">
          {this.minUpperDeckTemp}
        </text>
        <text class="F24 White LS2" x="239" y="506">
          TO
        </text>
        <text id="AftUpperTemp" class="F29 Green EndAlign" x="340" y="506">
          {this.maxUpperDeckTemp}
        </text>

        <text id="ForwardLowerTemp" class="F29 Green EndAlign" x="210" y="552">
          {this.minMainDeckTemp}
        </text>
        <text class="F24 White LS2" x="239" y="552">
          TO
        </text>
        <text id="AftLowerTemp" class="F29 Green EndAlign" x="340" y="552">
          {this.maxMainDeckTemp}
        </text>
      </>
    );
  }
}

export default CruiseCond;
