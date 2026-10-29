//  Copyright (c) 2026 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0
import {
  UI,
  ScopedView,
  Derived,
  Signal,
  Logic,
  Bond,
  ViewNode,
} from 'xplane-js/ui';
import { AbstractMfdPageProps } from '../../MFD';
import { dataStatusUri, flightPlanUriPage, fuelAndLoadPage, lateralRevisionHoldPage } from '../../shared/utils';
import { ActivePageTitleBar } from './ActivePageTitleBar';

export abstract class FmsPage<T extends AbstractMfdPageProps = AbstractMfdPageProps> extends ScopedView<T> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  protected readonly subs = [] as Bond[];

  protected readonly activePageTitle = Signal.of<string>('');

  protected readonly eoActive = Signal.of<boolean>(false);

  private readonly penaltyActive = Signal.of<boolean>(false);

  /** Penalty is only displayed in DATA STATUS, FUEL & LOAD, F-PLN, HOLD, ALTERNATE & WHAT IF Pages */
  private readonly penaltyUri = this.props.mfd.uiService.activeUri.derive(
    (uri) =>
      uri.uri === dataStatusUri ||
      uri.page === fuelAndLoadPage ||
      uri.page === flightPlanUriPage ||
      uri.page === lateralRevisionHoldPage,
  );

  protected readonly displayPenalty = Derived.from(
    Logic.and(),
    this.penaltyUri,
    this.penaltyActive,
  );

  public afterRender(node: ViewNode): void {
    super.afterRender(node);
    this.subs.push(
      this.props.mfd.uiService.activeUri.watch((val) => {
        this.activePageTitle.set(`${val.category.toUpperCase()}/${this.props.pageTitle}`);
      }, true),
      this.props.fmcService.masterFmcChanged.watch(() => {
        // Check if master FMC exists, re-route subjects
        this.props.fmcService.master.fmgc.data.engineOut.forward(this.eoActive);
        this.props.fmcService.master.fmgc.data.fuelPenaltyActive.forward(this.penaltyActive);
      }, true),
    );
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    for (const s of this.subs) {
      s.destroy();
    }

    super.destroy();
  }

  render(): ViewNode {
    return (
      <ActivePageTitleBar
        activePage={this.activePageTitle}
        offset={Signal.of('')}
        eoIsActive={this.eoActive}
        penaltyIsActive={this.displayPenalty}
        isFmsSubsystemPage={true}
      />
    );
  }
}
