import {
  ClockTopics,
  BusSignal,
  View,
  Bus,
  UI,
  Derived,
  Signal,
  Readable,
  ViewNode,
} from 'xplane-js/ui';
import { EwdSimvars } from '../shared/EwdSimvarPublisher';
import { FcdcBusEvents } from '@shared/publishers/FcdcPublisher';
import { FwsEvents } from '../../MsfsAvionicsCommon/providers/FwsPublisher';
import { ChecklistLineStyle, WD_NUM_LINES, WdLineData, WdSpecialLine } from '../../MsfsAvionicsCommon/EcamMessages';
import { DestroyableComponent } from '@flybywiresim/msfs-avionics-common';
import { FormattedFwcText } from './FormattedFwcText';
import { EclSoftKeys } from './EclSoftKeys';
import { AdrBusEvents, IrBusEvents } from '@flybywiresim/fbw-sdk';
import { CpiomEwdAvailabilityChecker } from '../EWD';

interface WdAbstractChecklistComponentProps {
  bus: Bus;
  visible: Readable<boolean>;
  abnormal: boolean;
  fwsAvail?: Readable<boolean>;
  cpiomAvailChecker?: CpiomEwdAvailabilityChecker;
}

export class WdAbstractChecklistComponent extends DestroyableComponent<WdAbstractChecklistComponentProps> {
  protected readonly sub = this.props.bus.inbox<
    AdrBusEvents & ClockTopics & EwdSimvars & FcdcBusEvents & FwsEvents & IrBusEvents
  >();

  protected readonly lineData: WdLineData[] = [];

  protected readonly activeLine = BusSignal.from(this.sub.topic('fws_active_item'), 0);

  protected readonly showFromLine = BusSignal.from(this.sub.topic('fws_show_from_line'), 0);
  protected readonly totalLines = Signal.of(0);

  // Overflow indicators
  protected readonly overflowTopVisibility = this.showFromLine.derive((l) => (l > 0 ? 'visible' : 'hidden'));
  protected readonly overflowBottomVisibility = Derived.from(
    ([sf, tl]) => (tl > WD_NUM_LINES && tl - WD_NUM_LINES > sf ? 'visible' : 'hidden'),
    this.showFromLine,
    this.totalLines,
  );

  static DefaultLineData: WdLineData = {
    activeProcedure: false,
    sensed: false,
    checked: false,
    text: '',
    style: ChecklistLineStyle.Standard,
    firstLine: false,
    lastLine: false,
  };

  // Subjects for rendering the WD lines
  private readonly lineDataSubject = Array.from(Array(WD_NUM_LINES), () =>
    Signal.of<WdLineData>(WdAbstractChecklistComponent.DefaultLineData),
  );
  private readonly lineSelected = Array.from(Array(WD_NUM_LINES), () => Signal.of<boolean>(false));

  public updateChecklists(notifyIndexes?: number[]) {
    let lineIdx = 0;
    this.totalLines.set(this.lineData.length);

    this.lineData.forEach((ld, index) => {
      if (index >= this.showFromLine.get() && lineIdx < WD_NUM_LINES) {
        if (notifyIndexes && notifyIndexes.findIndex((i) => i === index) !== -1) {
          this.lineDataSubject[lineIdx].emit();
        }
        this.lineDataSubject[lineIdx].set(ld);
        this.lineSelected[lineIdx].set(
          ld.originalItemIndex !== undefined && ld.activeProcedure
            ? ld.originalItemIndex === this.activeLine.get()
            : false,
        );
        lineIdx++;
      }
    });

    // Fill remaining lines blank
    while (lineIdx < WD_NUM_LINES) {
      this.lineDataSubject[lineIdx].set({
        activeProcedure: false,
        sensed: true,
        checked: false,
        text: '',
        style: ChecklistLineStyle.ChecklistItem,
        firstLine: true,
        lastLine: true,
        specialLine: WdSpecialLine.Empty,
      });
      this.lineSelected[lineIdx].set(false);
      lineIdx++;
    }
  }

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.activeLine.watch(() => this.updateChecklists());
    this.showFromLine.watch(() => this.updateChecklists());
  }

  // 18 lines
  render() {
    return (
      <div class="ProceduresContainer" style={{ display: this.props.visible.derive((it) => (it ? 'flex' : 'none')) }}>
        <div class="OverflowIndicatorColumn">
          <div style={{ visibility: this.overflowTopVisibility }}>
            <OverflowArrowSymbol facingUp={true} />
          </div>
          <div style={{ visibility: this.overflowBottomVisibility }}>
            <OverflowArrowSymbol facingUp={false} />
          </div>
        </div>
        <div class="WarningsColumn">
          {Array.from(Array(WD_NUM_LINES), () => '').map((_, index) => (
            <EclLine data={this.lineDataSubject[index]} selected={this.lineSelected[index]} />
          ))}
        </div>
        <EclSoftKeys />
      </div>
    );
  }
}

interface EclLineProps {
  data: Readable<WdLineData>;
  selected: Readable<boolean>;
}

export class EclLine extends View<EclLineProps> {
  render() {
    return (
      <div class="EclLineContainer">
        <div
          class={{
            EclLine: true,
            Selected: this.props.selected,
            ChecklistItem: this.props.data.derive(
              (d) => !d.abnormalProcedure && d.style === ChecklistLineStyle.ChecklistItem,
            ),
            Inactive: this.props.data.derive((d) => d.inactive === true),
            AbnormalItem: this.props.data.derive((d) => d.abnormalProcedure === true),
            Headline: this.props.data.derive((d) =>
              [
                ChecklistLineStyle.Headline,
                ChecklistLineStyle.SubHeadline,
                ChecklistLineStyle.CenteredSubHeadline,
              ].includes(d.style),
            ),
            Underline: this.props.data.derive(
              (d) =>
                d.style === ChecklistLineStyle.CenteredSubHeadline ||
                d.style === ChecklistLineStyle.SubHeadline ||
                d.style === ChecklistLineStyle.GreenTable,
            ),
            Centered: this.props.data.derive((d) => d.style === ChecklistLineStyle.CenteredSubHeadline),
            Checked: this.props.data.derive((d) => d.checked),
            ChecklistCompleted: this.props.data.derive((d) => d.style === ChecklistLineStyle.CompletedChecklist),
            DeferredProcedure: this.props.data.derive((d) => d.style === ChecklistLineStyle.DeferredProcedure),
            CompletedDeferredProcedure: this.props.data.derive(
              (d) => d.style === ChecklistLineStyle.CompletedDeferredProcedure,
            ),
            ChecklistCondition: this.props.data.derive((d) => d.style === ChecklistLineStyle.ChecklistCondition),
            Green: this.props.data.derive(
              (d) =>
                d.style === ChecklistLineStyle.Green ||
                (d.style === ChecklistLineStyle.GreenTable && d.activeProcedure && !d.inactive),
            ),
            Cyan: this.props.data.derive((d) => d.style === ChecklistLineStyle.Cyan),
            Amber: this.props.data.derive((d) => d.style === ChecklistLineStyle.Amber),
            White: this.props.data.derive((d) => d.style === ChecklistLineStyle.White),
            Red: this.props.data.derive((d) => d.style === ChecklistLineStyle.Red),
            OmissionDots: this.props.data.derive((d) => d.style === ChecklistLineStyle.OmissionDots),
            LandAnsa: this.props.data.derive((d) => d.style === ChecklistLineStyle.LandAnsa),
            LandAsap: this.props.data.derive((d) => d.style === ChecklistLineStyle.LandAsap),
          }}
          style={{
            display: this.props.data.derive((d) => (d.style === ChecklistLineStyle.SeparationLine ? 'none' : 'flex')),
          }}
        >
          <div
            class={{
              EclLineCheckboxArea: true,
              AbnormalItem: this.props.data.derive((d) => d.abnormalProcedure === true),
              ChecklistCondition: this.props.data.derive((d) => d.style === ChecklistLineStyle.ChecklistCondition),
              Inactive: this.props.data.derive((d) => d.inactive === true),
              Checked: this.props.data.derive((d) => d.checked),
              HiddenElement: this.props.data.derive(
                (d) =>
                  d.style === ChecklistLineStyle.Headline ||
                  d.style === ChecklistLineStyle.GreenTable ||
                  d.style === ChecklistLineStyle.OmissionDots ||
                  d.style === ChecklistLineStyle.LandAnsa ||
                  d.style === ChecklistLineStyle.LandAsap,
              ),
              Invisible: this.props.data.derive(
                (d) => d.sensed || (d.firstLine && d.lastLine) || d.specialLine === WdSpecialLine.Empty,
              ),
            }}
          >
            {this.props.data.derive((d) => (d.checked ? '\u25A0' : '\u25A1'))}
          </div>
          <span
            class="EclLineText"
            style={{
              display: this.props.data.derive((d) =>
                (d.abnormalProcedure === true && d.style === ChecklistLineStyle.Headline) ||
                d.style === ChecklistLineStyle.GreenTable
                  ? 'none'
                  : 'block',
              ),
            }}
          >
            {this.props.data.derive((d) => d.text.substring(0, 39))}
          </span>
          <span
            class="EclTableRow EclLineText"
            style={{
              display: this.props.data.derive((d) => (d.style === ChecklistLineStyle.GreenTable ? 'flex' : 'none')),
            }}
          >
            <span class="EclTableLeft">
              {this.props.data.derive((d) => {
                const idx = d.text.indexOf('|');
                return idx >= 0 ? d.text.substring(0, idx) : d.text;
              })}
            </span>
            <span
              class={{
                EclTableRight: this.props.data.derive((d) => !d.text.startsWith('|')),
                EclTableRightHeader: this.props.data.derive((d) => d.text.startsWith('|')),
              }}
            >
              {this.props.data.derive((d) => {
                const idx = d.text.indexOf('|');
                return idx >= 0 ? d.text.substring(idx + 1) : '';
              })}
            </span>
          </span>
          <svg
            version="1.1"
            xmlns="http://www.w3.org/2000/svg"
            width="680"
            height="33"
            style={{
              display: this.props.data.derive((d) =>
                d.abnormalProcedure === true && d.style === ChecklistLineStyle.Headline ? 'block' : 'none',
              ),
            }}
          >
            <FormattedFwcText x={10} y={24} message={this.props.data.derive((d) => d.text)} />
          </svg>
        </div>
        <div
          class={{
            EclSeparationLine: true,
            HiddenElement: this.props.data.derive((d) => d.style !== ChecklistLineStyle.SeparationLine),
          }}
          style={{
            display: this.props.data.derive((d) => (d.style === ChecklistLineStyle.SeparationLine ? 'block' : 'none')),
          }}
        />
        <div
          class={{
            EclLineEndMarker: true,
            Selected: this.props.selected,
            First: this.props.data.derive((d) => d.firstLine),
            Last: this.props.data.derive((d) => d.lastLine),
            HiddenElement: this.props.data.derive(
              (d) => (d.firstLine && d.lastLine) || d.specialLine === WdSpecialLine.Empty,
            ),
          }}
        />
      </div>
    );
  }
}

class OverflowArrowSymbol extends View<{ facingUp: boolean }> {
  render() {
    return (
      <svg xmlns="http://www.w3.org/2000/svg" width="30" viewBox="0 0 36 40" preserveAspectRatio="meet">
        <g transform={this.props.facingUp ? 'rotate (180, 18, 20)' : ''}>
          <rect
            stroke="#000"
            stroke-width="0"
            height="17.42185"
            width="9.53173"
            y="1.62429"
            x="13.25006"
            fill="#00ff00"
          />
          <path
            stroke="#000"
            d="m0.13726,14.20717l17.68697,24.02628c6.01023,-8.01338 12.02045,-16.02675 18.03068,-24.04013"
            stroke-width="0"
            fill="#00ff00"
          />
        </g>
      </svg>
    );
  }
}
