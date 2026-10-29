import {
  ClockTopics,
  View,
  Bus,
  UI,
  Signal,
  Bond,
  ViewNode,
} from 'xplane-js/ui';

import './MfdMsgList.scss';
import { Button } from '../../../MsfsAvionicsCommon/UiWidgets/Button';
import { ActivePageTitleBar } from '../common/ActivePageTitleBar';
import { FmcServiceInterface } from '../../FMC/FmcServiceInterface';

interface MfdMsgListProps {
  visible: Signal<boolean>;
  bus: Bus;
  fmcService: FmcServiceInterface;
}

export class MfdMsgList extends View<MfdMsgListProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  private readonly subs = [] as Bond[];

  private readonly eoActive = Signal.of<boolean>(false);

  private readonly topRef = UI.ref<HTMLDivElement>();

  private readonly msgListContainer = UI.ref<HTMLDivElement>();

  protected onNewData = () => {};

  // Yeah, it's expensive, but rn I won't find a better way
  private renderMessageList() {
    const arr = this.props.fmcService.master.fmsErrors.snapshot();

    if (arr && arr.length > 5) {
      console.warn('More than 5 FMS messages, truncating.');
    }

    // Clear all items
    if (this.msgListContainer.peek()) {
      while (this.msgListContainer.target.firstChild) {
        this.msgListContainer.target.removeChild(this.msgListContainer.target.firstChild);
      }
    }

    // Render them
    arr?.forEach((it, idx) => {
      if (idx < 4) {
        UI.mount(
          <div class="mfd-label msg-list-element">{it.messageText}</div>,
          this.msgListContainer.target,
        );
      }
    });
  }

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subs.push(
      this.props.visible.watch((vis) => {
        if (this.topRef.peek()) {
          this.topRef.target.style.display = vis ? 'block' : 'none';
        }
      }, true),
    );

    const sub = this.props.bus.inbox<ClockTopics>();
    this.subs.push(
      sub
        .topic('realTime')
        .limit(1)
        .listen((_t) => this.renderMessageList()),
    );

    this.subs.push(
      this.props.fmcService.masterFmcChanged.watch(() => {
        // FIXME the previous pipe leaks...
        this.props.fmcService.master.fmgc.data.engineOut.forward(this.eoActive);
      }),
    );
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    this.subs.forEach((x) => x.destroy());

    super.destroy();
  }

  render(): ViewNode {
    return (
      <div ref={this.topRef} class="mfd-fms-fpln-dialog-outer">
        <div class="mfd-fms-fpln-dialog-inner">
          <ActivePageTitleBar
            activePage={Signal.of('MESSAGE LIST')}
            offset={Signal.of('')}
            eoIsActive={this.eoActive}
            isFmsSubsystemPage={true}
          />
          {/* begin page content */}
          <div class="mfd-page-container">
            <div ref={this.msgListContainer} class="mfd-msg-list-element-container" />
            <div style="flex-grow: 1;" />
            <div style="display: flex; justify-content: flex-start;">
              <Button label="CLOSE" onClick={() => this.props.visible.set(false)} />
            </div>
          </div>
          {/* end page content */}
        </div>
      </div>
    );
  }
}
