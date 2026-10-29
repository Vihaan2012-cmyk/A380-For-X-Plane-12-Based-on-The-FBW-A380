// @ts-strict-ignore
import { ViewProps, View, UI, Bus, ViewNode, Signal, PanelEventTopics } from 'xplane-js/ui';
import { ClockSimvars } from '../shared/ClockSimvarPublisher';

enum DisplayTime {
  Hours,
  Minutes,
  Seconds,
}

const secondsToDisplay = (seconds: number): number[] => {
  const displayTime = [0, 0, 0];

  displayTime[DisplayTime.Hours] = Math.floor(seconds / Clock.SECONDS_PER_HOUR);
  displayTime[DisplayTime.Minutes] = Math.floor((seconds % Clock.SECONDS_PER_HOUR) / Clock.SECONDS_PER_MINUTE);
  displayTime[DisplayTime.Seconds] = Math.floor(seconds % Clock.SECONDS_PER_MINUTE);

  return displayTime;
};

interface ClockProps extends ViewProps {
  bus: Bus;
}

export class Clock extends View<ClockProps> {
  static readonly SECONDS_PER_MINUTE = 60;

  static readonly SECONDS_PER_HOUR = this.SECONDS_PER_MINUTE * 60;

  private readonly clockTextBig = Signal.of('');

  private readonly clockTextSmall = Signal.of('');

  private readonly ltsTest = Signal.of(false);

  private ltsState: number;

  private dc2IsPowered: boolean;

  private readonly dcHot1IsPowered = Signal.of(true);

  private readonly dateMode = Signal.of(false);

  private readonly currentUTC = Signal.of(0);

  private readonly currentDate = Signal.of({ dayOfMonth: 1, monthOfYear: 1, year: 1970 });

  private updateClockText(): void {
    if (!this.dcHot1IsPowered.get()) {
      this.clockTextBig.set('');
      this.clockTextSmall.set('');
      return;
    }

    if (this.ltsTest.get()) {
      this.clockTextBig.set('88:88');
      this.clockTextSmall.set('88');
      return;
    }

    if (!this.dateMode.get()) {
      const displayTime = secondsToDisplay(this.currentUTC.get());
      this.clockTextBig.set(
        `${displayTime[DisplayTime.Hours].toString().padStart(2, '0')}:${displayTime[DisplayTime.Minutes].toString().padStart(2, '0')}`,
      );
      this.clockTextSmall.set(displayTime[DisplayTime.Seconds].toString().padStart(2, '0'));
    } else {
      this.clockTextBig.set(
        `${this.currentDate.get().monthOfYear.toString().padStart(2, '0')}:${this.currentDate.get().dayOfMonth.toString().padStart(2, '0')}`,
      );
      this.clockTextSmall.set(this.currentDate.get().year.toString().substring(2, 4));
    }
  }

  public afterRender(node: ViewNode): void {
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
      .listen((dcHot1IsPowered) => this.dcHot1IsPowered.set(dcHot1IsPowered));

    sub
      .topic('currentUTC')
      .rounded(0)
      .listen((currentUTC) => this.currentUTC.set(currentUTC));
    sub
      .topic('dayOfMonth')
      .distinct()
      .listen((dayOfMonth) => this.currentDate.set({ ...this.currentDate.get(), dayOfMonth }));
    sub
      .topic('monthOfYear')
      .distinct()
      .listen((monthOfYear) => this.currentDate.set({ ...this.currentDate.get(), monthOfYear }));
    sub
      .topic('year')
      .distinct()
      .listen((year) => this.currentDate.set({ ...this.currentDate.get(), year }));

    const hEventsSub = this.props.bus.inbox<PanelEventTopics>();
    hEventsSub.topic('hEvent').listen((eventName) => {
      switch (eventName) {
        case 'A32NX_CHRONO_DATE':
          this.dateMode.set(!this.dateMode.get());
          break;
        default:
          break;
      }
    });

    [this.ltsTest, this.dcHot1IsPowered, this.dateMode, this.currentUTC].forEach((attr) =>
      attr.watch(() => this.updateClockText()),
    );
  }

  public render(): ViewNode {
    return (
      <>
        <text x="6" y="153" class="fontBig">
          {this.clockTextBig}
        </text>
        <text x="190" y="147" class="fontSmall">
          {this.clockTextSmall}
        </text>
      </>
    );
  }
}
