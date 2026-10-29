// @ts-strict-ignore
import { ViewProps, View, Bus, UI, Signal, ViewNode } from 'xplane-js/ui';
import { debouncedTimeDelta } from '../shared/Utils';
import { ClockSimvars } from '../shared/ClockSimvarPublisher';
import { Vars } from 'xplane-js/data';

const getDisplayString = (seconds: number | null, running: boolean, ltsTest: boolean): string => {
  if (ltsTest) {
    return '88:88';
  }

  if (seconds !== null) {
    return (
      Math.floor(Math.min(seconds, ElapsedTime.MAX_DISPLAYABLE_TIME_SECONDS) / ElapsedTime.SECONDS_PER_HOUR)
        .toString()
        .padStart(2, '0') +
      (running ? ':' : ' ') +
      Math.floor(
        (Math.min(seconds, ElapsedTime.MAX_DISPLAYABLE_TIME_SECONDS) % ElapsedTime.SECONDS_PER_HOUR) /
          ElapsedTime.SECONDS_PER_MINUTE,
      )
        .toString()
        .padStart(2, '0')
    );
  }
  return '';
};

interface ElapsedTimeProps extends ViewProps {
  bus: Bus;
}

export class ElapsedTime extends View<ElapsedTimeProps> {
  static readonly SECONDS_PER_MINUTE = 60;

  static readonly SECONDS_PER_HOUR = this.SECONDS_PER_MINUTE * 60;

  static readonly MAX_DISPLAYABLE_TIME_SECONDS = this.SECONDS_PER_HOUR * 99 + this.SECONDS_PER_MINUTE * 59; // 99 hours and 59 minutes in seconds

  private readonly timerText = Signal.of('');

  private readonly elapsedTime = Signal.of(null);

  private readonly ltsTest = Signal.of(false);

  private readonly elapsedKnobPos = Signal.of(1);

  private ltsState: number;

  private dc2IsPowered: boolean;

  private dcEssIsPowered: boolean;

  private prevTime: number;

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    const sub = this.props.bus.inbox<ClockSimvars>();
    sub
      .topic('ltsTest')
      .distinct()
      .listen((ltsTest) => {
        this.ltsState = ltsTest;
        this.ltsTest.set(ltsTest === 0 && this.dc2IsPowered);
      });

    sub
      .topic('dc2IsPowered')
      .distinct()
      .listen((dc2IsPowered) => {
        this.dc2IsPowered = dc2IsPowered;
        this.ltsTest.set(this.ltsState === 0 && dc2IsPowered);
      });

    sub
      .topic('dcHot1IsPowered')
      .distinct()
      .listen((dcHot1IsPowered) => {
        if (!dcHot1IsPowered) {
          this.elapsedTime.set(null);
        }
      });

    sub
      .topic('dcEssIsPowered')
      .distinct()
      .listen((dcEssIsPowered) => (this.dcEssIsPowered = dcEssIsPowered));

    sub
      .topic('elapsedKnobPos')
      .distinct()
      .listen((elapsedKnobPos) => this.elapsedKnobPos.set(elapsedKnobPos));

    sub
      .topic('absTime')
      .limit(5)
      .listen((absTime) => {
        if (this.dcEssIsPowered) {
          if (this.elapsedKnobPos.get() === 0) {
            const newElapsedTime = (this.elapsedTime.get() || 0) + debouncedTimeDelta(absTime, this.prevTime);
            this.elapsedTime.set(newElapsedTime);
          } else if (this.elapsedKnobPos.get() === 2) {
            this.elapsedTime.set(null);
          }
          this.prevTime = absTime;
        }
      });

    this.elapsedTime.watch((elapsedTime) =>
      Vars.write('L:A32NX_CHRONO_ET_ELAPSED_TIME', 'number', elapsedTime ?? -1),
    ); // Simvar ist not nullable, so a -1 placeholder is used

    [this.elapsedTime, this.ltsTest, this.elapsedKnobPos].forEach((attr) =>
      attr.watch(() =>
        this.timerText.set(
          getDisplayString(this.elapsedTime.get(), this.elapsedKnobPos.get() === 0, this.ltsTest.get()),
        ),
      ),
    );
  }

  render(): ViewNode {
    return (
      <text x="47" y="247" class="fontBig">
        {this.timerText}
      </text>
    );
  }
}
