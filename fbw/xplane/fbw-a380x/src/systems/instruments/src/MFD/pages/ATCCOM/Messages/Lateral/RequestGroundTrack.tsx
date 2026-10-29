import { View, UI, Signal, ViewNode } from 'xplane-js/ui';
import { InputField } from '../../../../../MsfsAvionicsCommon/UiWidgets/InputField';
import { HeadingFormat } from '../../../common/DataEntryFormats';
import { IconButton } from '../../../../../MsfsAvionicsCommon/UiWidgets/IconButton';
import { MessageVisualizationProps } from '../Registry';

export class RequestGroundTrack extends View<MessageVisualizationProps> {
  render(): ViewNode {
    return (
      <div class="request-block request-block-stackable">
        <IconButton icon="trashbin" onClick={this.props.onDelete} disabled={Signal.of(false)} />

        <div class="request-block-body">
          <div class="request-block-line">
            <div class="mfd-label request-block-input-label" style="position:relative;">
              REQUEST GROUND TRACK
            </div>
            <InputField<number>
              dataEntryFormat={new HeadingFormat()}
              mandatory={Signal.of(true)}
              value={Signal.of(null)}
              containerStyle="width: 150px; position:relative;"
              alignText="center"
              errorHandler={(e) => this.props.atcService.showAtcErrorMessage(e.type, e.details)}
              hEventConsumer={this.props.mfd.hEventConsumer}
              interactionMode={this.props.mfd.interactionMode}
            />
          </div>
          <div class="request-block-line"></div>
          <div class="request-block-line"></div>
        </div>
      </div>
    );
  }
}
