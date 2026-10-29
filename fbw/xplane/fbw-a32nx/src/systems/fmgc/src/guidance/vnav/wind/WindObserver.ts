import { MathUtils } from '@flybywiresim/fbw-sdk';
import { WindEntry } from '../../../flightplanning/data/wind';
import { BusValue, Bus } from 'xplane-js/ui';
import { NavigationEvents } from '@fmgc/navigation/Navigation';
import { WindUtils } from './WindUtils';

export type WindMeasurement = WindEntry;

export class WindObserver {
  private readonly sub = this.bus.inbox<NavigationEvents>();

  private readonly windDirection = BusValue.from(this.sub.topic('fms_nav_wind_direction'), null);

  private readonly windSpeed = BusValue.from(this.sub.topic('fms_nav_wind_speed'), null);

  private readonly altitude = BusValue.from(this.sub.topic('fms_nav_pressure_altitude'), null);

  constructor(private readonly bus: Bus) {}

  get(result: WindMeasurement): WindMeasurement | null {
    const windDirection = this.windDirection.get();
    const windSpeed = this.windSpeed.get();
    const altitude = this.altitude.get();

    if (windDirection === null || windSpeed === null || altitude === null) {
      return null;
    }

    result.altitude = altitude;
    WindUtils.setValues(windSpeed, windDirection * MathUtils.DEGREES_TO_RADIANS, result.vector);
    return result;
  }
}
