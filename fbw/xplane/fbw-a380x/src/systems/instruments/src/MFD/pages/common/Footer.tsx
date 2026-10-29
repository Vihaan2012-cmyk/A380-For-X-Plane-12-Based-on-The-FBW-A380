import { View, UI, Bond, Signal, ViewNode } from 'xplane-js/ui';
import { AbstractMfdPageProps } from '../../MFD';
import { Button } from '../../../MsfsAvionicsCommon/UiWidgets/Button';

export class Footer extends View<AbstractMfdPageProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  protected readonly subs = [] as Bond[];

  private readonly buttonRef = UI.ref<Button>();

  private readonly buttonText = Signal.of('MSG\nLIST');

  private readonly messageRef = UI.ref<HTMLSpanElement>();

  private readonly messageToBeCleared = Signal.of<boolean>(false);

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    if (this.props.fmcService.master) {
      this.subs.push(
        this.props.fmcService.master.fmsErrors.watch((_, __, ___, arr) => {
          const ind = arr.findIndex((el) => !el.cleared);

          if (ind > -1 && this.messageRef.peek()) {
            this.messageToBeCleared.set(true);
            this.messageRef.target.textContent = arr[ind].messageText;

            if (arr[ind].backgroundColor === 'white') {
              this.messageRef.target.style.backgroundColor = '#ffffff';
            } else if (arr[ind].backgroundColor === 'cyan') {
              this.messageRef.target.style.backgroundColor = '#00ffff';
            } else if (arr[ind].backgroundColor === 'amber') {
              this.messageRef.target.style.backgroundColor = '#e68000';
            }
            this.buttonText.set('CLEAR\nINFO');
          } else {
            this.messageToBeCleared.set(false);
            this.messageRef.target.textContent = '';
            this.messageRef.target.style.backgroundColor = 'none';
            this.buttonText.set('MSG\nLIST');
          }
        }, true),
      );
    }
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    this.subs.forEach((x) => x.destroy());

    super.destroy();
  }

  render(): ViewNode {
    return (
      <div class="mfd-footer">
        <Button
          ref={this.buttonRef}
          buttonStyle="width: 100px;"
          label={<span style="white-space: pre">{this.buttonText}</span>}
          onClick={() => {
            if (this.messageToBeCleared.get()) {
              this.props.fmcService.master.clearLatestFmsErrorMessage();
            } else {
              this.props.mfd.openMessageList();
            }
          }}
        />
        <div class="mfd-footer-message-area">
          <span ref={this.messageRef} />
        </div>
      </div>
    );
  }
}
