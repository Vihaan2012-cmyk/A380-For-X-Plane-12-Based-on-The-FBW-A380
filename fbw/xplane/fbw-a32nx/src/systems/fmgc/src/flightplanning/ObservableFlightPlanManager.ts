import { Bus, Lifespan, Signal, Readable, Bond } from 'xplane-js/ui';
import { FlightPlanEvents } from '@fmgc/flightplanning/sync/FlightPlanEvents';
import { FlightPlanInterface } from '@fmgc/flightplanning/FlightPlanInterface';
import { FlightPlanIndex } from '@fmgc/flightplanning/FlightPlanManager';
import { FlightPlan } from './plans/FlightPlan';
import { FlightPlanPerformanceData } from './plans/performance/FlightPlanPerformanceData';

export class ObservableFlightPlanManager implements Bond {
  private readonly subscriptions: Bond[] = [];

  private readonly flightPlanEventsSubscriber = this.bus.inbox<FlightPlanEvents>();

  constructor(
    private readonly bus: Bus,
    private readonly flightPlanInterface: FlightPlanInterface,
  ) {
    this.initialize();
  }

  public isLive = true;

  public isHeld = false;

  public readonly canInitialNotify = true;

  private initialize(): void {
    this._temporaryPlanExists.set(this.flightPlanInterface.hasTemporary);

    this.subscriptions.push(
      this.flightPlanEventsSubscriber.topic('flightPlanManager.create').listen((event) => {
        if (event.planIndex === FlightPlanIndex.Active) {
          this._activePlan.set(
            this.flightPlanInterface.hasActive ? this.flightPlanInterface.get(FlightPlanIndex.Active) : null,
          );
        } else if (event.planIndex === FlightPlanIndex.Temporary) {
          this._temporaryPlanExists.set(true);
        }
      }),
      this.flightPlanEventsSubscriber.topic('flightPlanManager.copy').listen((event) => {
        if (event.targetPlanIndex === FlightPlanIndex.Active) {
          this._activePlan.set(
            this.flightPlanInterface.hasActive ? this.flightPlanInterface.get(FlightPlanIndex.Active) : null,
          );
        } else if (event.targetPlanIndex === FlightPlanIndex.Temporary) {
          this._temporaryPlanExists.set(true);
        }
      }),
      this.flightPlanEventsSubscriber.topic('flightPlanManager.swap').listen((event) => {
        if (event.targetPlanIndex === FlightPlanIndex.Active) {
          this._activePlan.set(
            this.flightPlanInterface.hasActive ? this.flightPlanInterface.get(FlightPlanIndex.Active) : null,
          );
        }
      }),
      this.flightPlanEventsSubscriber.topic('flightPlanManager.delete').listen((event) => {
        if (event.planIndex === FlightPlanIndex.Active) {
          this._activePlan.set(
            this.flightPlanInterface.hasActive ? this.flightPlanInterface.get(FlightPlanIndex.Active) : null,
          );
        } else if (event.planIndex === FlightPlanIndex.Temporary) {
          this._temporaryPlanExists.set(false);
        }
      }),
      this.flightPlanEventsSubscriber.topic('flightPlanManager.deleteAll').listen(() => {
        this._activePlan.set(
          this.flightPlanInterface.hasActive ? this.flightPlanInterface.get(FlightPlanIndex.Active) : null,
        );
        this._temporaryPlanExists.set(false);
      }),
    );
  }

  private readonly _activePlan = Signal.of<FlightPlan<FlightPlanPerformanceData> | null>(null);
  /**
   * A reference to the active flight plan if it exists, or null if it does not. This can be used to be notified of when
   * subscriptions to e.g performance data have to be re-connected to a *new* active flight plan because the old ones
   * have been destroyed.
   */
  public readonly activePlan: Readable<FlightPlan<FlightPlanPerformanceData> | null> = this._activePlan;

  private readonly _temporaryPlanExists = Signal.of(false);
  public readonly temporaryPlanExists: Readable<boolean> = this._temporaryPlanExists;

  public hold(): this {
    this.isHeld = true;

    for (const sub of this.subscriptions) {
      sub.hold();
    }

    return this;
  }

  public release(initialNotify?: boolean): this {
    this.isHeld = false;

    for (const sub of this.subscriptions) {
      sub.release(initialNotify);
    }

    return this;
  }

  destroy(): void {
    this.isLive = false;

    for (const sub of this.subscriptions) {
      sub.destroy();
    }
  }

  /** @inheritdoc */
  public withLifecycle(lifecycle: Lifespan): this {
    lifecycle.adopt(this);
    return this;
  }
}
