import {
  ViewProps,
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  ViewNode,
} from 'xplane-js/ui';
import { BaroRefDisplay } from './BaroRefDisplay';
import { NdDataDisplay } from './NdDataDisplay';
import { FcuSimvars } from '../Publishers/FcuSimvarPublisher';

interface EisDisplayProps extends ViewProps {
  isCaptSide: boolean;
  bus: Bus;
}

export class EisDisplay extends View<EisDisplayProps> {
  private readonly sub = this.props.bus.inbox<FcuSimvars>();

  private efisCpActive = Derived.from(
    ([active, backup]) => active && !backup,
    BusSignal.from(this.sub.topic(`eisDisplay${this.props.isCaptSide ? 'Left' : 'Right'}Active`), false),
    BusSignal.from(this.sub.topic(this.props.isCaptSide ? 'eisLeftBackupActive' : 'eisRightBackupActive'), false),
  );

  public render(): ViewNode {
    return (
      <div style={{ visibility: this.efisCpActive.derive((active) => (active ? 'inherit' : 'hidden')) }}>
        <BaroRefDisplay isCaptSide={this.props.isCaptSide} bus={this.props.bus} />
        <NdDataDisplay index={this.props.isCaptSide ? 0 : 1} bus={this.props.bus} />
      </div>
    );
  }
}
