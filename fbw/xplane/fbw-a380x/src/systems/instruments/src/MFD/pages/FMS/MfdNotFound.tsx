import { View, UI, Signal, Bond, ViewNode } from 'xplane-js/ui';

import { ActivePageTitleBar } from '../common/ActivePageTitleBar';
import { AbstractMfdPageProps } from '../../MFD';
import { Footer } from '../common/Footer';

interface MfdNotFoundProps extends AbstractMfdPageProps {}

export class MfdNotFound extends View<MfdNotFoundProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  private subs = [] as Bond[];

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    new Promise((resolve) => setTimeout(resolve, 500)).then(() => this.props.mfd.uiService.navigateTo('back'));
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    this.subs.forEach((x) => x.destroy());

    super.destroy();
  }

  render(): ViewNode {
    return (
      <>
        <ActivePageTitleBar
          activePage={Signal.of('ERROR 404 NOT FOUND')}
          offset={Signal.of('')}
          eoIsActive={Signal.of(false)}
        />
        {/* begin page content */}
        <div class="mfd-page-container" />
        {/* end page content */}
        <Footer
          bus={this.props.bus}
          mfd={this.props.mfd}
          fmcService={this.props.fmcService}
          flightPlanInterface={this.props.fmcService.master.flightPlanInterface}
        />
      </>
    );
  }
}
