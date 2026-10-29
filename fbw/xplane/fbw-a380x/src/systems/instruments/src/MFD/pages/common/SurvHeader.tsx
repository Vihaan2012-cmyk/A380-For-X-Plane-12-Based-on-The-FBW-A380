import { UI, Signal, ViewNode } from 'xplane-js/ui';
import { AbstractHeader } from './AbstractHeader';
import { PageSelectorDropdownMenu } from '../../../MsfsAvionicsCommon/UiWidgets/PageSelectorDropdownMenu';

/*
 * Complete header for the SURV system
 */
export class SurvHeader extends AbstractHeader {
  private readonly controlsIsSelected = Signal.of(false);

  private readonly statSwitchIsSelected = Signal.of(false);

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subs.push(
      this.props.uiService.activeUri.watch((val) => {
        this.controlsIsSelected.set(val.category === 'controls');
        this.statSwitchIsSelected.set(val.category === 'status-switching');
      }, true),
    );
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    this.subs.forEach((x) => x.destroy());

    super.destroy();
  }

  render(): ViewNode {
    return (
      <>
        {super.render()}
        <div class="mfd-header-page-select-row">
          <PageSelectorDropdownMenu
            isActive={this.controlsIsSelected}
            label="CONTROLS"
            menuItems={[{ label: '', action: () => this.props.uiService.navigateTo('surv/controls') }]}
            idPrefix={`${this.props.uiService.captOrFo}_MFD_pageSelectorControls`}
            containerStyle="width: 25%"
          />
          <PageSelectorDropdownMenu
            isActive={this.statSwitchIsSelected}
            label="STATUS & SWITCHING"
            menuItems={[{ label: '', action: () => this.props.uiService.navigateTo('surv/status-switching') }]}
            idPrefix={`${this.props.uiService.captOrFo}_MFD_pageSelectorStatSwitch`}
            containerStyle="width: 40%"
          />
        </div>
      </>
    );
  }
}
