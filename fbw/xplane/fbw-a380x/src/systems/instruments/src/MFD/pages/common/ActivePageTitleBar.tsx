import {
  ViewProps,
  View,
  UI,
  Signal,
  Readable,
  Bond,
  ViewNode,
} from 'xplane-js/ui';
import './style.scss';

interface ActivePageTitleBarProps extends ViewProps {
  activePage: Readable<string>;
  offset: Readable<string>;
  eoIsActive?: Readable<boolean>;
  tmpyIsActive?: Readable<boolean>;
  /** Whether to display the PENALTY seciton on the title bar */
  penaltyIsActive?: Readable<boolean>;
  /** Whether this is an FMS subsystem page. If it is, title section for EO AND TMPY are visible */
  isFmsSubsystemPage?: boolean;
}

/*
 * Displays the title bar, with optional markers for lateral offsets, engine out and temporary flight plan
 */
export class ActivePageTitleBar extends View<ActivePageTitleBarProps> {
  //FIXME: We should have a generic title bar component without subsystem specific code here.
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  private readonly subs = [] as Bond[];

  private readonly offsetText = this.props.offset.derive((it) => (it ? `     OFFSET${this.props.offset.get()}` : ''));

  private readonly engineOutVisibility = (this.props.eoIsActive ?? Signal.of(false)).derive((v) =>
    v && this.props.isFmsSubsystemPage ? 'visible' : 'hidden',
  );

  private readonly temporaryVisibility = (this.props.tmpyIsActive ?? Signal.of(false)).derive((v) =>
    v && this.props.isFmsSubsystemPage ? 'visible' : 'hidden',
  );

  private readonly penaltyVisibility = (this.props.penaltyIsActive ?? Signal.of(false)).derive((v) =>
    v && this.props.isFmsSubsystemPage ? 'visible' : 'hidden',
  );

  private readonly titleBarMargin = this.props.isFmsSubsystemPage ? '2px' : '0px'; // As the bars are still rendered, removing the margin removes the black bars.

  public afterRender(node: ViewNode): void {
    super.afterRender(node);
    this.subs.push(this.offsetText, this.penaltyVisibility, this.engineOutVisibility, this.temporaryVisibility);
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    this.subs.forEach((x) => x.destroy());

    super.destroy();
  }

  render(): ViewNode {
    return (
      <div class="mfd-title-bar-container">
        <div class="mfd-title-bar-title fr space-between">
          <span class="mfd-label mfd-title-bar-text">
            {this.props.activePage}
            {this.offsetText}
          </span>
          <span class="mfd-title-bar-text label penalty" style={{ visibility: this.penaltyVisibility }}>
            PENALTY
          </span>
        </div>
        <div class="mfd-title-bar-section eo" style={{ 'margin-left': this.titleBarMargin }}>
          <span class="mfd-label mfd-title-bar-text label eo" style={{ visibility: this.engineOutVisibility }}>
            EO
          </span>
        </div>
        <div class="mfd-title-bar-section tmpy" style={{ 'margin-left': this.titleBarMargin }}>
          <span class="mfd-label mfd-title-bar-text label tmpy" style={{ visibility: this.temporaryVisibility }}>
            TMPY
          </span>
        </div>
      </div>
    );
  }
}
