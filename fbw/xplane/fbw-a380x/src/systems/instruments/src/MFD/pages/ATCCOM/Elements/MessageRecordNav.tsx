import { ViewProps, View, UI, ViewNode } from 'xplane-js/ui';
import { IconButton } from '../../../../MsfsAvionicsCommon/UiWidgets/IconButton';

interface MessageRecordNavProps extends ViewProps {}

export class MessageRecordNav extends View<MessageRecordNavProps> {
  render(): ViewNode {
    return (
      <div id="msg-record-nav">
        <IconButton icon={'double-up'} containerStyle="width:42px; height:42px; padding:6px;" />
        <IconButton icon={'single-up'} containerStyle="width:42px; height:42px; padding:10px;" />
        <IconButton icon={'single-down'} containerStyle="width:42px; height:42px; padding:10px;" />
        <IconButton icon={'double-down'} containerStyle="width:42px; height:42px; padding:6px;" />
      </div>
    );
  }
}
