//  Copyright (c) 2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { SignalList, View, UI, Signal, Bond, ViewNode } from 'xplane-js/ui';
import { AbstractOitFltOpsPageProps } from '../../OIT';
import { DropdownMenu } from '../../../MsfsAvionicsCommon/UiWidgets/DropdownMenu';
import { InputField } from '../../../MsfsAvionicsCommon/UiWidgets/InputField';
import { AirportFormat, LongAlphanumericFormat } from '../../../MFD/pages/common/DataEntryFormats';
import { Button } from '../../../MsfsAvionicsCommon/UiWidgets/Button';
import { OisInternalData } from '../../OisInternalPublisher';

interface OitFltOpsStatusPageProps extends AbstractOitFltOpsPageProps {}

export class OitFltOpsStatus extends View<OitFltOpsStatusPageProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  private readonly subs = [] as Bond[];

  public afterRender(node: ViewNode): void {
    super.afterRender(node);
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
        {/* begin page content */}
        <div class="oit-page-container framed">
          <div class="oit-flt-ops-sts-upper">
            <div class="oit-flt-ops-sts-line">
              <div class="oit-flt-ops-sts-line-left oit-label bigger">ACFT REGISTRATION</div>
              <div class="oit-flt-ops-sts-line-right oit-label bigger">
                <DropdownMenu
                  idPrefix={`${this.props.uiService.captOrFo}_OIT_acftRegistration`}
                  selectedIndex={Signal.of<number | null>(0)}
                  values={SignalList.of(['F-FBWA'])}
                  freeTextAllowed={false}
                  containerStyle="width: 600px;"
                  alignLabels="center"
                  numberOfDigitsForInputField={8}
                  tmpyActive={Signal.of(false)}
                  hEventConsumer={this.props.container.hEventConsumer}
                  interactionMode={this.props.container.interactionMode}
                />
                <div style="width: 40px;" />
                <Button
                  label="SYNCHRO<br />AVIONICS"
                  onClick={() => this.props.bus.outbox<OisInternalData>().post('synchroAvncs', true, true, false)}
                  containerStyle="width: 175px;"
                />
                <div style="flex-grow: 1" />
              </div>
            </div>
            <div class="oit-flt-ops-sts-line">
              <div class="oit-flt-ops-sts-line-left oit-label bigger">MSN</div>
              <div class="oit-flt-ops-sts-line-right oit-label bigger cyan">225</div>
            </div>
            <div class="oit-flt-ops-sts-line">
              <div class="oit-flt-ops-sts-line-left oit-label bigger">OIS VERSION</div>
              <div class="oit-flt-ops-sts-line-right oit-label bigger">20-JAN 25 V1.0</div>
            </div>
          </div>
          <div class="oit-flt-ops-sts-lower">
            <div class="oit-flt-ops-sts-line">
              <div class="oit-flt-ops-sts-line-left oit-label bigger">FLT NUMBER</div>
              <div class="oit-flt-ops-sts-line-right oit-label bigger">
                <InputField<string>
                  dataEntryFormat={new LongAlphanumericFormat()}
                  mandatory={Signal.of(true)}
                  value={this.props.container.laptopData.fltNumber}
                  containerStyle="width: 600px; margin-right: 5px;"
                  alignText="center"
                  hEventConsumer={this.props.container.hEventConsumer}
                  interactionMode={this.props.container.interactionMode}
                  overrideEmptyMandatoryPlaceholder="[]"
                  errorHandler={() => {}}
                />
                <div style="flex-grow: 1" />
              </div>
            </div>
            <div class="oit-flt-ops-sts-line">
              <div class="oit-flt-ops-sts-line-left oit-label bigger">FROM</div>
              <div class="oit-flt-ops-sts-line-right oit-label bigger">
                <InputField<string>
                  dataEntryFormat={new AirportFormat()}
                  mandatory={Signal.of(true)}
                  value={this.props.container.laptopData.fromAirport}
                  containerStyle="width: 250px; margin-right: 5px;"
                  alignText="center"
                  hEventConsumer={this.props.container.hEventConsumer}
                  interactionMode={this.props.container.interactionMode}
                  overrideEmptyMandatoryPlaceholder="[]"
                  errorHandler={() => {}}
                />
                <div class="oit-label bigger" style="width: 100px; padding-left: 40px;">
                  TO
                </div>
                <InputField<string>
                  dataEntryFormat={new AirportFormat()}
                  mandatory={Signal.of(true)}
                  value={this.props.container.laptopData.toAirport}
                  containerStyle="width: 250px; margin-right: 5px;"
                  alignText="center"
                  hEventConsumer={this.props.container.hEventConsumer}
                  interactionMode={this.props.container.interactionMode}
                  overrideEmptyMandatoryPlaceholder="[]"
                  errorHandler={() => {}}
                />
                <div style="flex-grow: 1" />
              </div>
            </div>
          </div>
        </div>
        {/* end page content */}
      </>
    );
  }
}
