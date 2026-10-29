import {
  ClockTopics,
  BusSignal,
  Bus,
  UI,
  Derived,
  Readable,
  ViewNode,
} from 'xplane-js/ui';
import { DestroyableComponent } from '@flybywiresim/msfs-avionics-common';
import { FormattedFwcText } from './FormattedFwcText';
import { EwdSimvars } from '../shared/EwdSimvarPublisher';
import { EcamLimitations } from '../../MsfsAvionicsCommon/EcamMessages';
import { FwsEvents } from '../../MsfsAvionicsCommon/providers/FwsPublisher';

interface WdLimitationsProps {
  bus: Bus;
  visible: Readable<boolean>;
}

export class WdLimitations extends DestroyableComponent<WdLimitationsProps> {
  private readonly sub = this.props.bus.inbox<ClockTopics & EwdSimvars & FwsEvents>();

  private readonly limitationsLeft = BusSignal.from(this.sub.topic(`fws_limitations_all_phases`), []);
  private readonly limitationsRight = BusSignal.from(this.sub.topic(`fws_limitations_appr_ldg`), []);

  private readonly limitationsLeftFormatString = this.limitationsLeft.derive((limits) =>
    limits.map((val) => EcamLimitations[val]).join('\r'),
  );

  private readonly limitationsRightFormatString = this.limitationsRight.derive((limits) =>
    limits.map((val) => EcamLimitations[val]).join('\r'),
  );

  private readonly limitationsLines = Derived.from(
    ([all, apprLdg]) => (all.length > 0 || apprLdg.length > 0 ? Math.max(all.length, apprLdg.length) : 0),
    this.limitationsLeft,
    this.limitationsRight,
  );
  private readonly limitationsDisplay = Derived.from(
    ([lines, visible]) => (lines > 0 && visible ? 'flex' : 'none'),
    this.limitationsLines,
    this.props.visible,
  );
  private readonly limitationsHeight = this.limitationsLines.derive((lines) => `${lines * 30 + 3}px`);

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subscriptions.push(
      this.limitationsLeft,
      this.limitationsRight,
      this.limitationsLeftFormatString,
      this.limitationsRightFormatString,
      this.limitationsLines,
      this.limitationsDisplay,
      this.limitationsHeight,
    );
  }

  render() {
    return (
      <>
        <div
          class="LimitationsContainer"
          style={{
            display: this.limitationsDisplay,
          }}
        >
          <span class="LimitationsHeading Underline">LIMITATIONS</span>
          <div class="MemosDividedArea">
            <div class="MemosLeft">
              <span class="LimitationsHeading">ALL PHASES</span>
              <svg version="1.1" xmlns="http://www.w3.org/2000/svg" style={{ height: this.limitationsHeight }}>
                <FormattedFwcText x={0} y={24} message={this.limitationsLeftFormatString} />
              </svg>
            </div>
            <div class="MemosRight">
              <span class="LimitationsHeading">APPR & LDG</span>
              <svg version="1.1" xmlns="http://www.w3.org/2000/svg" style={{ height: this.limitationsHeight }}>
                <FormattedFwcText x={0} y={24} message={this.limitationsRightFormatString} />
              </svg>
            </div>
          </div>
        </div>
      </>
    );
  }
}
