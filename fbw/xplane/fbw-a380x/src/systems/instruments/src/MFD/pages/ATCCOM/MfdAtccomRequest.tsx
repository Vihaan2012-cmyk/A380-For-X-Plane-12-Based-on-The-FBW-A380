import { SignalList, View, UI, Signal, ViewNode } from 'xplane-js/ui';

import './MfdAtccomRequest.scss';
import { AtccomMfdPageProps } from '../../MFD';
import { AtccomFooter } from './MfdAtccomFooter';

import { Button } from '../../../MsfsAvionicsCommon/UiWidgets/Button';
import { RequestMenuButton } from '../common/RequestMenuButton';
import { ActivePageTitleBar } from '../common/ActivePageTitleBar';
import { DropdownMenu } from '../../../MsfsAvionicsCommon/UiWidgets/DropdownMenu';
import { MaxRequestElements, MessageFrame, MessageTable } from './Messages/Registry';
import { DclMessage } from '@datalink/common';

interface MfdAtccomRequestProps extends AtccomMfdPageProps {}

export class MfdAtccomRequest extends View<MfdAtccomRequestProps> {
  private dropdownMenuRef = UI.ref<DropdownMenu>();

  private elements: { id: string; message: undefined; readyToSend: boolean }[] = [];
  private FanMode: string = 'A';

  private messageElements = SignalList.of<MessageFrame>();

  protected onNewData() {}

  private elementDelete = (elementIndex: number) => {
    this.elements.splice(elementIndex, 1);
    this.renderElements();
  };

  private onSelect(frameName: string): void {
    if (!MessageTable[frameName]) {
      return console.log('Request frame not found: ' + frameName);
    }

    if (
      MessageTable[frameName].singleMessage ||
      (this.elements.length !== 0 && MessageTable[this.elements[0].id].singleMessage)
    ) {
      this.elements = [];
    }

    // check and swap frame is frames are to be exchanged
    if (MessageTable[frameName].exchanging !== undefined) {
      for (let i = 0; i < this.elements.length; i++) {
        if (MessageTable[frameName].exchanging === this.elements[i].id) {
          this.elements.splice(i, 1, { id: frameName, message: undefined, readyToSend: false });
          this.renderElements();
          return;
        }
      }
    }

    if (this.elements.length < MaxRequestElements) {
      this.elements.push({ id: frameName, message: undefined, readyToSend: false });
      this.renderElements();
    }
  }

  private createDclMessage(data) {
    const formattedMessage = new DclMessage();
    formattedMessage.Callsign = data.callsign;
    formattedMessage.Origin = data.from;
    formattedMessage.Destination = data.to;
    formattedMessage.AcType = 'A388';
    formattedMessage.Atis = data.atis;
    formattedMessage.Gate = data.gate;
    formattedMessage.Freetext = data.freetext.filter((n) => n);
    formattedMessage.Station = data.station;

    return formattedMessage;
  }

  private transferToMailbox(): void {
    console.log(this.messageElements.snapshot());
    // check for DCL message
    if (this.elements[0].id == 'RequestDepartureClearance') {
      // register dcl message
      this.props.atcService.registerMessages([this.createDclMessage(this.messageElements.snapshot()[0].message)]);
    }
  }

  // Convert this to use a subscription when elements is changed
  private renderElements(): void {
    UI.remove(document.getElementById('request-frame-0'));
    UI.remove(document.getElementById('request-frame-1'));
    UI.remove(document.getElementById('request-frame-2'));
    UI.remove(document.getElementById('request-frame-3'));
    UI.remove(document.getElementById('request-frame-4'));

    this.elements.forEach((element, index) => {
      if (MessageTable[element.id]) {
        UI.mount(
          <div id={`request-frame-${index}`} className="request-frame"></div>,
          document.getElementById('atccom-request-body'),
        );
        UI.mount(
          MessageTable[element.id].visualization(this.props, this.FanMode, index, this.messageElements, () =>
            this.elementDelete(index),
          ),
          document.getElementById(`request-frame-${index}`),
        );
      }
    });
  }

  public afterRender(node: ViewNode): void {
    super.afterRender(node);
  }

  render(): ViewNode {
    return (
      <>
        <ActivePageTitleBar
          activePage={Signal.of('CONNECT')}
          offset={Signal.of('')}
          eoIsActive={Signal.of(false)}
          tmpyIsActive={Signal.of(false)}
        />
        {/* begin page content */}
        <div class="mfd-page-container">
          <div id="atccom-request-container">
            <div id="atccom-request-body"></div>
            <div id="atccom-request-menu">
              <RequestMenuButton
                label="VERTICAL"
                idPrefix="Request_Vertical_Menu"
                onClick={() => {}}
                buttonStyle="width:180px;"
                menuItems={Signal.of([
                  { label: 'CLIMB TO', action: () => this.onSelect('RequestClimb') },
                  { label: 'DESCEND TO', action: () => this.onSelect('RequestDescend') },
                  { label: 'ALT/FL', action: () => this.onSelect('RequestLevel') },
                  { label: 'BLOCK ALT/FL', action: () => this.onSelect('RequestLevelBlock') },
                  { label: 'CRUISE CLIMB', action: () => this.onSelect('RequestCruiseClimb') },
                  { label: 'ITP', action: () => this.onSelect('RequestITP') },
                ])}
              />
              <RequestMenuButton
                label="LATERAL"
                idPrefix="Request_Lateral_Menu"
                onClick={() => {}}
                buttonStyle="width:180px;"
                menuItems={Signal.of([
                  { label: 'DIRECT TO', action: () => this.onSelect('RequestDirect') },
                  { label: 'OFFSET', action: () => this.onSelect('RequestOffset') },
                  { label: 'WX DEVIATION', action: () => this.onSelect('RequestWeatherDeviation') },
                  { label: 'HEADING', action: () => this.onSelect('RequestHeading') },
                  { label: 'TRACK', action: () => this.onSelect('RequestGroundTrack') },
                  { label: 'SID/STAR', action: () => {} },
                  { label: 'TAILORED ARRIVAL', action: () => {} },
                  { label: 'REROUTING', action: () => {} },
                ])}
              />
              <RequestMenuButton
                label="SPEED"
                idPrefix="Request_Speed_Menu"
                onClick={() => {}}
                buttonStyle="width:180px;"
                menuItems={Signal.of([
                  { label: 'SPEED', action: () => {} },
                  { label: 'SPEED RANGE', action: () => {} },
                ])}
              />
              <RequestMenuButton
                label="CLEARANCE"
                idPrefix="Request_Clearance_Menu"
                onClick={() => {}}
                buttonStyle="width:180px;"
                menuItems={Signal.of([
                  { label: 'DEPARTURE', action: () => this.onSelect('RequestDepartureClearance') },
                  { label: 'OCEANIC', action: () => this.onSelect('RequestOceanicClearance') },
                  { label: 'GENERIC', action: () => this.onSelect('RequestGenericClearance') },
                ])}
              />
              <RequestMenuButton
                label="WHEN CAN<br />WE EXPECT"
                idPrefix="Request_WhenCanWeExpect_Menu"
                onClick={() => {}}
                buttonStyle="width:180px;"
                menuItems={Signal.of([
                  { label: 'HIGHER LEVEL', action: () => {} },
                  { label: 'LOWER LEVEL', action: () => {} },
                  { label: 'CLIMB TO', action: () => {} },
                  { label: 'DESCEND TO', action: () => {} },
                  { label: 'CRUISE CLIMB', action: () => {} },
                  { label: 'SPEED', action: () => {} },
                  { label: 'SPEED RANGE', action: () => {} },
                  { label: 'BACK ON ROUTE', action: () => {} },
                ])}
              />
              <RequestMenuButton
                label="OTHER"
                idPrefix="Request_Other_Menu"
                onClick={() => {}}
                buttonStyle="width:180px;"
                menuItems={Signal.of([
                  { label: 'FREETEXT', action: () => {} },
                  { label: 'VOICE CONTACT', action: () => {} },
                  { label: 'OWN SEPARATION & VMC', action: () => {} },
                  { label: 'VMC DES', action: () => {} },
                ])}
              />
              <div style="min-height: 40px;"></div>
              <RequestMenuButton
                label="ADD TEXT"
                idPrefix="Request_AddText_Menu"
                onClick={() => {}}
                buttonStyle="width:180px;"
                menuItems={Signal.of([
                  { label: 'DUE TO WEATHER', action: () => {} },
                  { label: 'DUE TO A/C PERFORMANCE', action: () => {} },
                  { label: 'DUE TO TURBULENCE', action: () => {} },
                  { label: 'DUE TO TECHNICAL', action: () => {} },
                  { label: 'DUE TO MEDICAL', action: () => {} },
                  { label: 'AT PILOTS DISCRETION', action: () => {} },
                  { label: 'FREETEXT', action: () => {} },
                ])}
              />
            </div>
          </div>
          <div class="mfd-atccom-request-footer">
            <div>
              <Button
                label="CANCEL"
                disabled={Signal.of(false)}
                onClick={() => {
                  this.elements = [];
                  this.renderElements();
                }}
                buttonStyle="width: 190px; height:64px;"
              />
            </div>
            <div style="position:absolute; right:0px">
              <Button
                label="XFR<br /> TO MAILBOX"
                disabled={Signal.of(false)}
                onClick={() => this.transferToMailbox()}
                buttonStyle="width: 190px; height:64px;"
              />
            </div>
          </div>
        </div>
        {/* <div
          id="atccom-inop"
          style="
    position: absolute;
    top: 132px;
    width: 768px;
    height: 818px;
    display: flex;
    justify-content: center;
    align-items: center;
    font-size: 35px;
    background-color: rgba(0, 0, 0, 0.7);
    color: #e68000"
        >
          <span>NOT YET IMPLEMENTED</span>
        </div> */}
        <AtccomFooter bus={this.props.bus} mfd={this.props.mfd} atcService={this.props.atcService} />
      </>
    );
  }
}
