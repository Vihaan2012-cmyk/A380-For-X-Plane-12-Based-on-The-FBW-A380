import {
  BusSignal,
  View,
  UI,
  VarUnit,
  Signal,
  Bond,
  ViewNode,
} from 'xplane-js/ui';

import './MfdSurvStatusSwitching.scss';

import { MfdSurvEvents } from '../../../MsfsAvionicsCommon/providers/MfdSurvPublisher';
import { ActivePageTitleBar } from '../common/ActivePageTitleBar';
import { AbstractMfdPageProps } from '../../MFD';
import { Footer } from '../common/Footer';
import { MfdSimvars } from '../../shared/MFDSimvarPublisher';
import { SurvStatusButton } from '../../../MsfsAvionicsCommon/UiWidgets/SurvStatusButton';
import { SurvStatusItem } from '../../../MsfsAvionicsCommon/UiWidgets/SurvStatusItem';
import { Vars } from 'xplane-js/data';

interface MfdSurvStatusSwitchingProps extends AbstractMfdPageProps {}

export enum StatusItemState {
  Off = 0,
  On = 1,
  Failed = 2,
}

export class MfdSurvStatusSwitching extends View<MfdSurvStatusSwitchingProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  private readonly subs = [] as Bond[];

  private readonly sub = this.props.bus.inbox<MfdSimvars & MfdSurvEvents>();

  private readonly tcas1Failed = BusSignal.from(this.sub.topic('tcasFail'), true);

  private readonly wxr1Failed = Signal.of<boolean>(false);

  private readonly turb1Failed = Signal.of<boolean>(false);

  private readonly predWs1Failed = Signal.of<boolean>(false);

  private readonly xpdr1Failed = Signal.of<boolean>(false);

  private readonly terr1Failed = BusSignal.from(this.sub.topic('terr1Failed'), false);

  private readonly gpws1Failed = BusSignal.from(this.sub.topic('gpws1Failed'), false);

  private readonly wxr2Failed = Signal.of<boolean>(false);

  private readonly turb2Failed = Signal.of<boolean>(false);

  private readonly predWs2Failed = Signal.of<boolean>(false);

  private readonly terr2Failed = BusSignal.from(this.sub.topic('terr2Failed'), false);

  private readonly gpws2Failed = BusSignal.from(this.sub.topic('gpws2Failed'), false);

  private readonly xpdr2Failed = Signal.of<boolean>(false);

  private readonly tcas2Failed = Signal.of<boolean>(false);

  private readonly activeSystemGroupWxrTaws = BusSignal.from(this.sub.topic('wxrTawsSysSelected'), 0);
  private readonly wxrTaws1Active = this.activeSystemGroupWxrTaws.derive((s) => s === 1);
  private readonly wxrTaws2Active = this.activeSystemGroupWxrTaws.derive((s) => s === 2);

  private readonly activeSystemGroupXpdrTcas = Signal.of<number>(1);
  private readonly xpdrTcas1Active = this.activeSystemGroupXpdrTcas.derive((s) => s === 1);
  private readonly xpdrTcas2Active = this.activeSystemGroupXpdrTcas.derive((s) => s === 2);

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subs.push(
      this.tcas1Failed,
      this.terr1Failed,
      this.gpws1Failed,
      this.terr2Failed,
      this.gpws2Failed,
      this.activeSystemGroupWxrTaws,
      this.wxrTaws1Active,
      this.wxrTaws2Active,
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
      <>
        <ActivePageTitleBar activePage={Signal.of('STATUS & SWITCHING')} offset={Signal.of('')} />
        {/* begin page content */}
        <div class="mfd-page-container">
          <div style="width: 100%; display: flex; flex-direction: row; justify-content: space-between; align-items: top; padding: 50px;">
            {/* upper left sys box */}
            <div class="sys-box">
              <SurvStatusButton
                label={'SYS 1'}
                active={this.wxrTaws1Active}
                onClick={() =>
                  Vars.write(
                    'L:A32NX_WXR_TAWS_SYS_SELECTED',
                    VarUnit.Number,
                    this.wxrTaws1Active.get() ? 0 : 1,
                  )
                }
              />
              <div
                class={{
                  'sys-group': true,
                  active: this.wxrTaws1Active,
                }}
                style="margin-bottom: 10px;"
              >
                <SurvStatusItem
                  label={'WXR DISPLAY'}
                  sys={'1'}
                  active={this.wxrTaws1Active}
                  failed={this.wxr1Failed}
                  style={'margin-bottom: 10px;'}
                />
                <SurvStatusItem
                  label={'TURB'}
                  sys={'1'}
                  active={this.wxrTaws1Active}
                  failed={this.turb1Failed}
                  style={'margin-bottom: 10px;'}
                />
                <SurvStatusItem label={'PRED W/S'} sys={'1'} active={this.wxrTaws1Active} failed={this.predWs1Failed} />
              </div>
              <div class={{ 'sys-group': true, active: this.wxrTaws1Active }}>
                <SurvStatusItem
                  label={'TERR SYS'}
                  sys={'1'}
                  active={this.wxrTaws1Active}
                  failed={this.terr1Failed}
                  style={'margin-bottom: 10px;'}
                />
                <SurvStatusItem label={'GPWS'} sys={'1'} active={this.wxrTaws1Active} failed={this.gpws1Failed} />
              </div>
            </div>
            {/* upper middle text */}
            <div style="text-align: center;">
              <div class="mfd-label bigger" style="margin-top: 55px;">
                WXR
              </div>
              <div class="mfd-label bigger" style="margin-top: 90px;">
                TAWS
              </div>
            </div>
            {/* upper right sys box */}
            <div class="sys-box">
              <SurvStatusButton
                label={'SYS 2'}
                active={this.wxrTaws2Active}
                onClick={() =>
                  Vars.write(
                    'L:A32NX_WXR_TAWS_SYS_SELECTED',
                    VarUnit.Number,
                    this.wxrTaws2Active.get() ? 0 : 2,
                  )
                }
              />
              <div
                class={{
                  'sys-group': true,
                  active: this.wxrTaws2Active,
                }}
                style="margin-bottom: 10px;"
              >
                <SurvStatusItem
                  label={'WXR DISPLAY'}
                  sys={'2'}
                  active={this.wxrTaws2Active}
                  failed={this.wxr2Failed}
                  style={'margin-bottom: 10px;'}
                />
                <SurvStatusItem
                  label={'TURB'}
                  sys={'2'}
                  active={this.wxrTaws2Active}
                  failed={this.turb2Failed}
                  style={'margin-bottom: 10px;'}
                />
                <SurvStatusItem label={'PRED W/S'} sys={'2'} active={this.wxrTaws2Active} failed={this.predWs2Failed} />
              </div>
              <div class={{ 'sys-group': true, active: this.wxrTaws2Active }}>
                <SurvStatusItem
                  label={'TERR SYS'}
                  sys={'2'}
                  active={this.wxrTaws2Active}
                  failed={this.terr2Failed}
                  style={'margin-bottom: 10px;'}
                />
                <SurvStatusItem label={'GPWS'} sys={'2'} active={this.wxrTaws2Active} failed={this.gpws2Failed} />
              </div>
            </div>
          </div>
          {/* lower line */}
          <div style="width: 100%; display: flex; flex-direction: row; justify-content: space-between; align-items: top; padding: 50px;">
            {/* lower left sys box */}
            <div class="sys-box">
              <SurvStatusButton label={'SYS 1'} active={this.xpdrTcas1Active} />
              <div class={{ 'sys-group': true, active: this.xpdrTcas1Active }} style="margin-bottom: 5px;">
                <SurvStatusItem label={'XPDR'} sys={'1'} active={this.xpdrTcas1Active} failed={this.xpdr1Failed} />
              </div>
              <div class={{ 'sys-group': true, active: this.xpdrTcas1Active }}>
                <SurvStatusItem label={'TCAS'} sys={'1'} active={this.xpdrTcas1Active} failed={this.tcas1Failed} />
              </div>
            </div>
            {/* lower middle text */}
            <div style="text-align: center;">
              <div class="mfd-label bigger" style="margin-top: 55px;">
                XPDR
              </div>
              <div class="mfd-label bigger" style="margin-top: 20px;">
                TCAS
              </div>
            </div>
            {/* lower right sys box */}
            <div class="sys-box">
              <SurvStatusButton
                label={'SYS 2'}
                active={this.xpdrTcas2Active}
                onClick={() => console.log('button clicked')}
              />
              <div class={{ 'sys-group': true, active: this.xpdrTcas2Active }} style="margin-bottom: 5px;">
                <SurvStatusItem label={'XPDR'} sys={'2'} active={this.xpdrTcas2Active} failed={this.xpdr2Failed} />
              </div>
              <div class={{ 'sys-group': true, active: this.xpdrTcas2Active }}>
                <SurvStatusItem label={'TCAS'} sys={'2'} active={this.xpdrTcas2Active} failed={this.tcas2Failed} />
              </div>
            </div>
          </div>
        </div>
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
