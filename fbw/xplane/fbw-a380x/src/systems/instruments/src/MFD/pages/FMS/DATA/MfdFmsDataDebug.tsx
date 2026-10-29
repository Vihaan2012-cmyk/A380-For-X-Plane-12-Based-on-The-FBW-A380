/* eslint-disable max-len */

import { ClockTopics, UI, Signal, ViewNode } from 'xplane-js/ui';

import './MfdFmsDataStatus.scss';
import { AbstractMfdPageProps } from '../../../MFD';
import { Footer } from '../../common/Footer';

import { FmsPage } from '../../common/FmsPage';
import { MfdSimvars } from '../../../shared/MFDSimvarPublisher';
import { TopTabNavigator, TopTabNavigatorPage } from '../../../../MsfsAvionicsCommon/UiWidgets/TopTabNavigator';
import { Arinc429Register } from '@flybywiresim/fbw-sdk';
import { Vars } from 'xplane-js/data';

interface MfdFmsDataDebugProps extends AbstractMfdPageProps {}
export class MfdFmsDataDebug extends FmsPage<MfdFmsDataDebugProps> {
  private selectedPageIndex = Signal.of<number>(0);

  private readonly primVls = Arinc429Register.empty();
  private readonly primV_a_prot = Arinc429Register.empty();
  private readonly primV_a_max = Arinc429Register.empty();

  private tab1lineLabels = [
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
  ];

  private tab1lineValues = [
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
    Signal.of<string>(''),
  ];

  protected onNewData() {
    this.tab1lineLabels[1].set('VLS (FMC)');
    this.tab1lineValues[1].set((Vars.read('L:A32NX_SPEEDS_VLS', 'number') as number).toFixed(2) ?? '');

    this.tab1lineLabels[2].set('V_A_PROT (FMC)');
    this.tab1lineValues[2].set(
      (Vars.read('L:A32NX_SPEEDS_ALPHA_PROTECTION_CALC', 'number') as number).toFixed(2) ?? '',
    );

    this.tab1lineLabels[3].set('V_A_MAX (FMC)');
    this.tab1lineValues[3].set(
      (Vars.read('L:A32NX_SPEEDS_ALPHA_MAX_CALC', 'number') as number).toFixed(2) ?? '',
    );

    this.primVls.set(Vars.read('L:A32NX_PRIM_1_V_LS', 'number'));
    this.tab1lineLabels[5].set('VLS (PRIM 1)');
    this.tab1lineValues[5].set(this.primVls.value.toFixed(1));

    this.primV_a_prot.set(Vars.read('L:A32NX_PRIM_1_V_ALPHA_PROT', 'number'));
    this.tab1lineLabels[6].set('V_A_PROT (PRIM 1)');
    this.tab1lineValues[6].set(this.primV_a_prot.value.toFixed(1));

    this.primV_a_max.set(Vars.read('L:A32NX_PRIM_1_V_ALPHA_LIM', 'number'));
    this.tab1lineLabels[7].set('V_A_MAX ((PRIM 1))');
    this.tab1lineValues[7].set(this.primV_a_max.value.toFixed(1));

    this.tab1lineLabels[8].set('PRIM SSM');
    this.tab1lineValues[8].set(this.primVls.ssm.toFixed(0));

    this.tab1lineLabels[10].set('GD SPEED (FCOM LOOKUP)');
    this.tab1lineValues[10].set((Vars.read('L:A32NX_SPEEDS_GD', 'number') as number).toFixed(2) ?? '');
    this.tab1lineLabels[11].set('F SPEED (FCOM LOOKUP)');
    this.tab1lineValues[11].set((Vars.read('L:A32NX_SPEEDS_F', 'number') as number).toFixed(2) ?? '');
    this.tab1lineLabels[12].set('S SPEED (FCOM LOOKUP)');
    this.tab1lineValues[12].set((Vars.read('L:A32NX_SPEEDS_S', 'number') as number).toFixed(2) ?? '');
  }

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<ClockTopics & MfdSimvars>();
    this.subs.push(
      sub
        .topic('realTime')
        .limit(2)
        .listen((_t) => {
          this.onNewData();
        }),
    );
  }

  render(): ViewNode {
    return (
      <>
        {super.render()}
        {/* begin page content */}
        <div class="mfd-page-container">
          <TopTabNavigator
            pageTitles={Signal.of(['FMGC', 'N/A'])}
            selectedPageIndex={this.selectedPageIndex}
            pageChangeCallback={(val) => this.selectedPageIndex.set(val)}
            selectedTabTextColor="white"
            tabBarSlantedEdgeAngle={25}
          >
            <TopTabNavigatorPage>
              {/* FMGC */}
              {this.tab1lineLabels.map((_, idx) => (
                <div style="margin-bottom: 10px;">
                  <span class="mfd-label" style="margin-right: 25px;">
                    {this.tab1lineLabels[idx]}
                  </span>
                  <span class="mfd-value bigger">{this.tab1lineValues[idx]}</span>
                </div>
              ))}
            </TopTabNavigatorPage>
            <TopTabNavigatorPage>{/* N/A */}</TopTabNavigatorPage>
          </TopTabNavigator>
          <div style="flex-grow: 1;" />
          {/* fill space vertically */}
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
