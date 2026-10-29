import { View, UI, Signal, ViewNode } from 'xplane-js/ui';

import './MfdAtccomMsgRecord.scss';
import { AbstractMfdPageProps } from '../../MFD';
import { Footer } from '../common/Footer';

import { ActivePageTitleBar } from '../common/ActivePageTitleBar';
import { Button } from '../../../MsfsAvionicsCommon/UiWidgets/Button';

interface MfdAtccomMsgRecordProps extends AbstractMfdPageProps {}

export class MfdAtccomMsgRecord extends View<MfdAtccomMsgRecordProps> {
  protected onNewData() {}

  public afterRender(node: ViewNode): void {
    super.afterRender(node);
  }

  render(): ViewNode {
    return (
      <>
        <ActivePageTitleBar activePage={Signal.of('MSG RECORD')} offset={Signal.of('')} />
        {/* begin page content */}
        <div class="mfd-page-container" style="position:relative; width:100%">
          <div style="position:absolute; top:90px; width:100%; height:275px; border-top: 1px solid #fff; border-bottom: 1px solid #fff">
            <div style="position:absolute; top:50px; left:65px;">
              <Button
                label="ALL MSG"
                buttonStyle="width:205px; height:64px"
                onClick={() => {
                  this.props.mfd.uiService.navigateTo('atccom/msg-record/all-msg');
                }}
              />
            </div>
            <div style="position:absolute; top:165px; left:65px;">
              <Button
                label="MONITORED MSG"
                buttonStyle="width:205px; height:64px; padding: 9px 5px 5px 5px;"
                onClick={() => {
                  this.props.mfd.uiService.navigateTo('atccom/msg-record/monitored-msg');
                }}
              />
            </div>
          </div>
        </div>
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
