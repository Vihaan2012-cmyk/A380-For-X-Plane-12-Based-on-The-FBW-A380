import {
  ClockTopics,
  BusSignal,
  View,
  Bus,
  UI,
  Signal,
  Readable,
  ViewNode,
} from 'xplane-js/ui';
import { FormattedFwcText } from './FormattedFwcText';
import { EwdSimvars } from '../shared/EwdSimvarPublisher';
import { EcamMemos } from '../../MsfsAvionicsCommon/EcamMessages';

interface WdMemosProps {
  bus: Bus;
  visible: Readable<boolean>;
}

const padEWDCode = (code: number) => code.toString().padStart(9, '0');

export class WdMemos extends View<WdMemosProps> {
  private readonly sub = this.props.bus.inbox<ClockTopics & EwdSimvars>();

  private readonly memosLeftSvgRef = UI.ref<SVGGraphicsElement>();

  private readonly memosRightSvgRef = UI.ref<SVGGraphicsElement>();

  private readonly memosLeft = Array.from(Array(10), (_, idx) =>
    BusSignal.from(this.sub.topic(`memo_left_${idx + 1}`).distinct(), 0),
  );

  private readonly memosRight = Array.from(Array(10), (_, idx) =>
    BusSignal.from(this.sub.topic(`memo_right_${idx + 1}`).distinct(), 0),
  );

  private readonly memosLeftFormatString = Signal.of('');

  private readonly memosRightFormatString = Signal.of('');

  private update() {
    this.memosLeftFormatString.set(
      this.memosLeft
        .filter((v) => !!v.get())
        .map((val) => EcamMemos[padEWDCode(val.get())])
        .join('\r'),
    );
    this.memosRightFormatString.set(
      this.memosRight
        .filter((v) => !!v.get())
        .map((val) => EcamMemos[padEWDCode(val.get())])
        .join('\r'),
    );

    this.memosLeftSvgRef.target.style.height = `${this.memosLeft.filter((v) => !!v.get()).length * 30 + 3}px`;
    this.memosRightSvgRef.target.style.height = `${this.memosRight.filter((v) => !!v.get()).length * 30 + 3}px`;
  }

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.memosLeft.forEach((el) => el.watch(() => this.update(), true));
    this.memosRight.forEach((el) => el.watch(() => this.update(), true));

    this.sub
      .topic('realTime')
      .limit(0.05)
      .listen(() => this.update());
  }

  render() {
    return (
      <>
        <div class="MemosContainer" style={{ display: this.props.visible.derive((it) => (it ? 'flex' : 'none')) }}>
          <div class="MemosDividedArea">
            <div class="MemosLeft">
              <svg ref={this.memosLeftSvgRef} version="1.1" xmlns="http://www.w3.org/2000/svg">
                <FormattedFwcText x={0} y={24} message={this.memosLeftFormatString} />
              </svg>
            </div>
            <div class="MemosRight">
              <svg ref={this.memosRightSvgRef} version="1.1" xmlns="http://www.w3.org/2000/svg">
                <FormattedFwcText x={0} y={24} message={this.memosRightFormatString} />
              </svg>
            </div>
          </div>
          <div class="FillArea" />
        </div>
      </>
    );
  }
}
