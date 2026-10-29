// @ts-strict-ignore
import { Bus, BoardDevice, Outbox } from 'xplane-js/ui';
import { AircraftModel, PlaneState } from 'xplane-js/data';

export interface SimplaneValues {
  units: string;
  pressure: number;
  machActive: boolean;
  holdValue: number;
  airSpeedHoldValue: number;
  isSelectedSpeed: boolean;
  selectedHeading: number;
  selectedAltitude: number;
  baroMode: 'QNH' | 'QFE' | 'STD';
}
export class SimplaneValueProvider implements BoardDevice {
  private publisher: Outbox<SimplaneValues>;

  constructor(private readonly bus: Bus) {
    this.publisher = this.bus.outbox<SimplaneValues>();
  }

  /** @inheritdoc */
  public setup(): void {
    // noop
  }

  /** @inheritdoc */
  public tick(): void {
    const units = PlaneState.baroUnits();
    const pressure = PlaneState.baroSetting(units);
    const isSelected = PlaneState.apSpeedSelected();
    const isMach = PlaneState.apMachMode();
    const selectedHeading = PlaneState.apHeadingTarget(false) || 0;
    const selectedAltitude = PlaneState.apAltitudeTarget();
    const holdValue = isMach ? PlaneState.apMachTarget() : PlaneState.apSpeedTarget();
    const baroMode = PlaneState.baroMode(AircraftModel.A320) as 'QNH' | 'QFE' | 'STD';

    this.publisher.post('units', units);
    this.publisher.post('pressure', pressure);
    this.publisher.post('isSelectedSpeed', isSelected);
    this.publisher.post('machActive', isMach);
    this.publisher.post('holdValue', holdValue);
    this.publisher.post('selectedHeading', selectedHeading);
    this.publisher.post('selectedAltitude', selectedAltitude);
    this.publisher.post('baroMode', baroMode);
  }
}
