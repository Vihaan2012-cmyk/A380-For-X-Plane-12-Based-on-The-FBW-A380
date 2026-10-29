//  Copyright (c) 2023 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { MetarReport } from 'xplane-js/ui';
import { AtsuStatusCodes, WeatherMessage } from '../../../common/src';
import { Bridge } from 'xplane-js/data';

export class MsfsConnector {
  public static async receiveMsfsMetar(icao: string, message: WeatherMessage): Promise<AtsuStatusCodes> {
    try {
      const metar: MetarReport = await Bridge.call('wx.metar', icao);
      let report = metar.metarString.toUpperCase();
      if (!report || metar.icao !== icao) {
        report = 'NO METAR AVAILABLE';
      }
      message.Reports.push({ airport: icao, report });
      return AtsuStatusCodes.Ok;
    } catch {
      message.Reports.push({ airport: icao, report: 'NO METAR AVAILABLE' });
      return AtsuStatusCodes.Ok;
    }
  }

  public static async receiveMsfsTaf(icao: string, message: WeatherMessage): Promise<AtsuStatusCodes> {
    let report: string | undefined;
    try {
      const taf: { tafString: string; icao: string } = await Bridge.call('wx.taf', icao);
      if (taf.tafString && taf.icao === icao) {
        report = taf.tafString;
      }
    } catch (e) {
      console.log('receiveMsfsTaf: Failed', e);
    }

    message.Reports.push({ airport: icao, report: report ?? 'NO TAF AVAILABLE' });
    return AtsuStatusCodes.Ok;
  }
}
