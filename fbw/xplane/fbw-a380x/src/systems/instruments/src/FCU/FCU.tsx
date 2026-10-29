import { ViewProps, View, Bus, UI, ViewNode } from 'xplane-js/ui';
import { EisDisplay } from './Components/EisDisplay';
import { AfsDisplay } from './Components/AfsDisplay';
import { FcuSimvars } from './Publishers/FcuSimvarPublisher';

import './style.scss';

interface FCUProps extends ViewProps {
  bus: Bus;
}

export class FCUComponent extends View<FCUProps> {
  private readonly sub = this.props.bus.inbox<FcuSimvars>();

  render(): ViewNode {
    return (
      <div id="Mainframe">
        <div id="Electricity">
          <AfsDisplay bus={this.props.bus} />
          <EisDisplay isCaptSide bus={this.props.bus} />
          <EisDisplay isCaptSide={false} bus={this.props.bus} />
        </div>
      </div>
    );
  }
}
