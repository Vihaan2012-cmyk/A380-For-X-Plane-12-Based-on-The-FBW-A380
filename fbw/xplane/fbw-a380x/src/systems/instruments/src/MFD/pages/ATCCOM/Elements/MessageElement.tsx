import { ViewProps, View, UI, ViewNode } from 'xplane-js/ui';

interface MessageElementProps extends ViewProps {
  msgTime?: string;
  msgOriginDest?: string;
  msgStatus: string;
  msgBody?: string;
  onClick?: () => void;
}

export class MessageElement extends View<MessageElementProps> {
  private divRef = UI.ref<HTMLDivElement>();

  private onClick() {
    if (this.props.onClick !== undefined) {
      this.props.onClick();
    }
  }

  private onClickHandler = this.onClick.bind(this);

  afterRender(node: ViewNode): void {
    super.afterRender(node);
    this.divRef.target.addEventListener('click', this.onClickHandler);
  }

  render(): ViewNode {
    return (
      <div ref={this.divRef} class="msg-record-msg-element mfd-label green">
        <div>
          <span class="msg-time">{this.props.msgTime}</span>
          <span class="msg-origin-dest">FROM {this.props.msgOriginDest}</span>
          <span class="msg-status">{this.props.msgStatus}</span>
        </div>
        <div class="msg-body">{this.props.msgBody}</div>
        <div>
          <span class="msg-expand-button">...... &gt;&gt;&gt;</span>
        </div>
      </div>
    );
  }
}
