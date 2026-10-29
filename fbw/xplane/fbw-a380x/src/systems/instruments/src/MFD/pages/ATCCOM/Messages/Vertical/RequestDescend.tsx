import { View, UI, Signal, ViewNode } from 'xplane-js/ui';
import { InputField } from '../../../../../MsfsAvionicsCommon/UiWidgets/InputField';
import { AirportFormat, ShortAlphanumericFormat } from '../../../common/DataEntryFormats';
import { IconButton } from '../../../../../MsfsAvionicsCommon/UiWidgets/IconButton';
import { MessageVisualizationProps } from '../Registry';

export class RequestDescend extends View<MessageVisualizationProps> {
  render(): ViewNode {
    return (
      <div class="request-block request-block-stackable">
        <IconButton icon="trashbin" onClick={this.props.onDelete} disabled={Signal.of(false)} />

        <div class="request-block-body">
          <div class="request-block-line">
            <div class="mfd-label request-block-input-label">REQUEST DES TO</div>
            <InputField<string>
              dataEntryFormat={new AirportFormat()}
              value={Signal.of('')}
              containerStyle="width: 130px; margin-right: 5px;"
              alignText="center"
              errorHandler={(e) => this.props.atcService.showAtcErrorMessage(e.type, e.details)}
              hEventConsumer={this.props.mfd.hEventConsumer}
              interactionMode={this.props.mfd.interactionMode}
            />
          </div>
          <div class="request-block-line">
            <div class="mfd-label request-block-input-label" style="margin-left: 168px;">
              AT
            </div>
            <InputField<string>
              dataEntryFormat={new ShortAlphanumericFormat()}
              value={Signal.of('')}
              containerStyle="width: 337px;"
              alignText="center"
              errorHandler={(e) => this.props.atcService.showAtcErrorMessage(e.type, e.details)}
              hEventConsumer={this.props.mfd.hEventConsumer}
              interactionMode={this.props.mfd.interactionMode}
            />
          </div>
          <div class="request-block-line"></div>
        </div>
      </div>
    );
  }
}
