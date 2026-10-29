import { View, UI, Signal, ViewNode } from 'xplane-js/ui';
import { InputField } from '../../../../../MsfsAvionicsCommon/UiWidgets/InputField';
import { WaypointFormat } from '../../../common/DataEntryFormats';
import { IconButton } from '../../../../../MsfsAvionicsCommon/UiWidgets/IconButton';
import { MessageVisualizationProps } from '../Registry';

export class RequestDirect extends View<MessageVisualizationProps> {
  render(): ViewNode {
    return (
      <div class="request-block request-block-stackable">
        <IconButton icon="trashbin" onClick={this.props.onDelete} disabled={Signal.of(false)} />

        <div class="request-block-body">
          <div class="request-block-line">
            <div class="mfd-label request-block-input-label" style="position:relative;">
              REQUEST DIR TO
            </div>
          </div>
          <div class="request-block-line">
            <InputField<string>
              dataEntryFormat={new WaypointFormat()}
              mandatory={Signal.of(true)}
              value={Signal.of('')}
              containerStyle="width: 353px; position:relative; left:200px;"
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
