import {
  ViewProps,
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  ViewNode,
} from 'xplane-js/ui';
import { HdgDisplay } from './HdgDisplay';
import { SpdDisplay } from './SpdDisplay';
import { VsDisplay } from './VsDisplay';
import { AltDisplay } from './AltDisplay';
import { FcuSimvars } from '../Publishers/FcuSimvarPublisher';

interface AfsDisplayProps extends ViewProps {
  bus: Bus;
}

export class AfsDisplay extends View<AfsDisplayProps> {
  private sub = this.props.bus.inbox<FcuSimvars>();

  private afsCpActive = Derived.from(
    ([active, backup]) => active && !backup,
    BusSignal.from(this.sub.topic('afsDisplayActive'), false),
    BusSignal.from(this.sub.topic('afsBackupActive'), false),
  );

  public render(): ViewNode {
    return (
      <div id="LargeScreen" style={{ visibility: this.afsCpActive.derive((active) => (active ? 'inherit' : 'hidden')) }}>
        <SpdDisplay bus={this.props.bus} />
        <HdgDisplay bus={this.props.bus} />

        <div id="AltVS">
          <AltDisplay bus={this.props.bus} />
          <VsDisplay bus={this.props.bus} />
        </div>
      </div>
    );
  }
}
