// Copyright (c) 2025-2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { View, UI, Signal, ViewNode } from 'xplane-js/ui';

import './MfdAtccomDAtis.scss';
import { AtccomMfdPageProps } from '../../MFD';
import { DAtisBlock } from './DAtisBlock';

import { Button } from '../../../MsfsAvionicsCommon/UiWidgets/Button';
import { ActivePageTitleBar } from '../common/ActivePageTitleBar';
import { AtccomFooter } from './MfdAtccomFooter';

interface MfdAtccomDAtisProps extends AtccomMfdPageProps {}

export class MfdAtccomDAtis extends View<MfdAtccomDAtisProps> {
  protected onNewData() {}

  private airports = this.props.atcService.getAtisAirports();

  public afterRender(node: ViewNode): void {
    super.afterRender(node);
  }
  render(): ViewNode {
    return (
      <>
        <ActivePageTitleBar activePage={Signal.of('D-ATIS/LIST')} offset={Signal.of('')} />
        {/* begin page content */}
        <div class="mfd-page-container">
          <DAtisBlock
            bus={this.props.bus}
            mfd={this.props.mfd}
            atcService={this.props.atcService}
            index={0}
            data={this.airports[0]}
          />
          <DAtisBlock
            bus={this.props.bus}
            mfd={this.props.mfd}
            atcService={this.props.atcService}
            index={1}
            data={this.airports[1]}
          />
          <DAtisBlock
            bus={this.props.bus}
            mfd={this.props.mfd}
            atcService={this.props.atcService}
            index={2}
            data={this.airports[2]}
          />
          <div class="mfd-atccom-datis-footer">
            <Button
              label="PRINT<br/>ALL"
              disabled={true}
              onClick={() => {}}
              buttonStyle="width: 190px;"
              containerStyle="position:absolute; top: 3px; right:190px"
            />
            <Button
              label="UPDATE<br/>ALL"
              onClick={() => {
                this.props.atcService.updateAllAtis();
              }}
              buttonStyle="width: 190px;"
              containerStyle="position:absolute; top: 3px; right:0px"
            />
          </div>
        </div>
        <AtccomFooter bus={this.props.bus} mfd={this.props.mfd} atcService={this.props.atcService} />
      </>
    );
  }
}
