import {
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  Readable,
  Bond,
  ViewNode,
} from 'xplane-js/ui';

import './F-PLN/MfdFmsFpln.scss';
import { FmcServiceInterface } from '../../FMC/FmcServiceInterface';
import { MfdSimvars } from '../../shared/MFDSimvarPublisher';
import { MfdSystem } from '../common/MfdUiService';

interface MfdFmsPageNotAvailProps {
  bus: Bus;
  fmcService: FmcServiceInterface;
  captOrFo: 'CAPT' | 'FO';
  requestedSystem: Readable<MfdSystem>;
}

export class MfdFmsPageNotAvail extends View<MfdFmsPageNotAvailProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  private subs = [] as Bond[];

  private readonly fmsFailed = BusSignal.from(
    this.props.bus.inbox<MfdSimvars>().topic(this.props.captOrFo === 'FO' ? 'fmsFoFailed' : 'fmsCaptFailed'),
    false,
  );

  private readonly warningDisplay = Derived.from(
    ([failed, sys]) => (failed && sys === MfdSystem.Fms ? 'inherit' : 'none'),
    this.fmsFailed,
    this.props.requestedSystem,
  );

  public afterRender(node: ViewNode): void {
    super.afterRender(node);
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    this.subs.forEach((x) => x.destroy());

    super.destroy();
  }

  render(): ViewNode {
    return (
      <div class="mfd-fms-fpln-dialog-outer" style={{ display: this.warningDisplay }}>
        <div class="mfd-fms-fpln-dialog-inner" style="top: 34px; height: 818px;">
          {/* begin page content */}
          <div class="mfd-page-container">
            <div class="mfd-amber-error-message">FMS PAGE NOT AVAIL</div>
          </div>
          {/* end page content */}
        </div>
      </div>
    );
  }
}
