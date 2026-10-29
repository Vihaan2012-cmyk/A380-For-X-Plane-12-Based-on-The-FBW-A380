// @ts-strict-ignore
import { ViewProps, View, Bus, UI, PanelEventTopics, Signal, ViewNode } from 'xplane-js/ui';
import { debouncedTimeDelta } from '../shared/Utils';
import { ClockSimvars } from '../shared/ClockSimvarPublisher';
import { Vars } from 'xplane-js/data';

interface ChronoProps extends ViewProps {
  bus: Bus;
}

const getDisplayString = (seconds: number | null, running: boolean, ltsTest: boolean): string => {
  if (ltsTest) {
    return '88:88';
  }

  if (seconds !== null) {
    const elapsedTime = seconds % Chrono.MAX_DISPLAYABLE_TIME_SECONDS;
    return (
      Math.floor(elapsedTime / Chrono.SECONDS_PER_MINUTE)
        .toString()
        .padStart(2, '0') +
      (running ? ':' : ' ') +
      Math.floor(elapsedTime % Chrono.SECONDS_PER_MINUTE)
        .toString()
        .padStart(2, '0')
    );
  }
  return '';
};

export class Chrono extends View<ChronoProps> {
  static readonly SECONDS_PER_MINUTE = 60;

  /** Max time in seconds that can be displayed. The chrono displays rolls over from "99:59" to "00:00" on reaching this time. */
  static readonly MAX_DISPLAYABLE_TIME_SECONDS = 99 * 60 + 59 + 1;

  private readonly chronoText = Signal.of('');

  private readonly elapsedTime = Signal.of(null);

  private readonly running = Signal.of(false);

  private readonly ltsTest = Signal.of(false);

  private ltsState: number;

  private dc2IsPowered: boolean;

  private dcEssIsPowered: boolean;

  private prevTime: number;

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
      .listen((dcHot1IsPowered) => {
        if (!dcHot1IsPowered) {
          this.running.set(false);
          this.elapsedTime.set(null);
        }
      });

    sub
      .topic('dcEssIsPowered')
      .distinct()
      .listen((dcEssIsPowered) => (this.dcEssIsPowered = dcEssIsPowered));

    sub
      .topic('absTime')
      .limit(5)
      .listen((absTime) => {
        if (this.running.get()) {
          const newElapsedTime = (this.elapsedTime.get() || 0) + debouncedTimeDelta(absTime, this.prevTime);
          this.elapsedTime.set(newElapsedTime);
        }
        this.prevTime = absTime;
      });

    const hEventsSub = this.props.bus.inbox<PanelEventTopics>();
    hEventsSub.topic('hEvent').listen((eventName) => {
      switch (eventName) {
        case 'A32NX_CHRONO_RST':
          if (this.dcEssIsPowered) {
            if (this.running.get()) {
              this.elapsedTime.set(0);
            } else {
              this.elapsedTime.set(null);
            }
          }
          break;
        case 'A32NX_CHRONO_TOGGLE':
          if (this.dcEssIsPowered) {
            this.running.set(!this.running.get());
          }
          break;
        default:
          break;
      }
    });

    this.elapsedTime.watch((elapsedTime) =>
      Vars.write('L:A32NX_CHRONO_ELAPSED_TIME', 'number', elapsedTime ?? -1),
    ); // Simvar ist not nullable, so a -1 placeholder is used

    [this.elapsedTime, this.running, this.ltsTest].forEach((attr) =>
      attr.watch(() =>
        this.chronoText.set(getDisplayString(this.elapsedTime.get(), this.running.get(), this.ltsTest.get())),
      ),
    );
  }

  public render(): ViewNode {
    return (
      <text x="47" y="60" class="fontBig">
        {this.chronoText}
      </text>
    );
  }
}
