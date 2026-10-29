import { UI, Signal, ViewNode } from 'xplane-js/ui';
import { AbstractHeader } from './AbstractHeader';
import { PageSelectorDropdownMenu } from '../../../MsfsAvionicsCommon/UiWidgets/PageSelectorDropdownMenu';

/*
 * Complete header for the ATCCOM system
 */
export class AtccomHeader extends AbstractHeader {
  private readonly connectIsSelected = Signal.of(false);

  private readonly requestIsSelected = Signal.of(false);

  private readonly reportModifyIsSelected = Signal.of(false);

  private readonly msgRecordIsSelected = Signal.of(false);

  private readonly atisIsSelected = Signal.of(false);

  private readonly emerIsSelected = Signal.of(false);

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subs.push(
      this.props.uiService.activeUri.watch((val) => {
        this.connectIsSelected.set(val.category === 'connect');
        this.requestIsSelected.set(val.category === 'request');
        this.reportModifyIsSelected.set(val.category === 'report-modify');
        this.msgRecordIsSelected.set(val.category === 'msg-record');
        this.atisIsSelected.set(val.category === 'd-atis');
        this.emerIsSelected.set(val.category === 'emer');
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
            isActive={this.connectIsSelected}
            label="CONNECT"
            menuItems={[{ label: '', action: () => this.props.uiService.navigateTo('atccom/connect') }]}
            idPrefix={`${this.props.uiService.captOrFo}_MFD_pageSelectorConnect`}
            containerStyle="flex: 1"
          />
          <PageSelectorDropdownMenu
            isActive={this.requestIsSelected}
            label="REQUEST"
            menuItems={[{ label: '', action: () => this.props.uiService.navigateTo('atccom/request') }]}
            idPrefix={`${this.props.uiService.captOrFo}_MFD_pageSelectorRequest`}
            containerStyle="flex: 1"
          />
          <PageSelectorDropdownMenu
            isActive={this.reportModifyIsSelected}
            label="REPORT<br /> & NOTIFY"
            menuItems={[{ label: '', action: () => this.props.uiService.navigateTo('atccom/report-modify/position') }]}
            idPrefix={`${this.props.uiService.captOrFo}_MFD_pageSelectorReportModify`}
            containerStyle="flex: 1.2"
            labelStyle="margin-top: 0; margin-bottom: 0; padding-top: 2px; padding-bottom: 2px"
          />
          <PageSelectorDropdownMenu
            isActive={this.msgRecordIsSelected}
            label="MSG RECORD"
            menuItems={[{ label: '', action: () => this.props.uiService.navigateTo('atccom/msg-record') }]}
            idPrefix={`${this.props.uiService.captOrFo}_MFD_pageSelectorMsgRecord`}
            containerStyle="flex: 1"
            labelStyle="margin-top: 0; margin-bottom: 0; padding-top: 2px; padding-bottom: 2px"
          />
          <PageSelectorDropdownMenu
            isActive={this.atisIsSelected}
            label="D-ATIS"
            menuItems={[{ label: '', action: () => this.props.uiService.navigateTo('atccom/d-atis/list') }]}
            idPrefix={`${this.props.uiService.captOrFo}_MFD_pageSelectorAtis`}
            containerStyle="flex: 1"
          />
          <PageSelectorDropdownMenu
            isActive={this.emerIsSelected}
            label="EMER"
            menuItems={[{ label: '', action: () => this.props.uiService.navigateTo('atccom/emer') }]}
            idPrefix={`${this.props.uiService.captOrFo}_MFD_pageSelectorEmer`}
            containerStyle="flex: 1"
            labelStyle="color: #000; background-color: #ff9601; background-clip: content-box"
          />
        </div>
      </>
    );
  }
}
