// Copyright (c) 2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  ChartOrigin,
  Charts,
  FaaChart,
  IcaoCodes,
  LidoChart,
  RunwayId,
} from 'xplane-js/ui';
import { MsfsChartCategory } from '../../../Store/features/navigationPage';

export interface MsfsChartPage {
  lightUrl: string;
  darkUrl?: string;
}

export type MsfsProviders = ChartOrigin.Faa | ChartOrigin.Lido;

export interface MsfsChartData<T extends ChartOrigin.Faa | ChartOrigin.Lido> {
  guid: string;
  name: string;
  type: T extends ChartOrigin.Faa ? FaaChart : T extends ChartOrigin.Lido ? LidoChart : never;
  runways: RunwayId[];
}

const faaCategoryMap: Record<string, MsfsChartCategory> = {
  [FaaChart.Apd]: 'ARPT',
  [FaaChart.Dau]: 'ARPT',
  [FaaChart.Dp]: 'SID',
  [FaaChart.Hot]: 'ARPT',
  [FaaChart.Iap]: 'APPR',
  [FaaChart.Lah]: 'ARPT',
  [FaaChart.Min]: 'ARPT',
  [FaaChart.Odp]: 'SID',
  [FaaChart.Star]: 'STAR',
  [FaaChart.Unknown]: 'ARPT',
};

const lidoCategoryMap: Record<string, MsfsChartCategory> = {
  [LidoChart.Afc]: 'ARPT',
  [LidoChart.Agc]: 'ARPT',
  [LidoChart.Aoi]: 'ARPT',
  [LidoChart.Apc]: 'ARPT',
  [LidoChart.Iac]: 'APPR',
  [LidoChart.Lvc]: 'ARPT',
  [LidoChart.Mrc]: 'ARPT',
  [LidoChart.ObstDep]: 'SID',
  [LidoChart.Sid]: 'SID',
  [LidoChart.SidInitialClimb]: 'SID',
  [LidoChart.SidPt]: 'SID',
  [LidoChart.Star]: 'STAR',
  [LidoChart.Unknown]: 'ARPT',
  [LidoChart.Vac]: 'APPR',
};

export const runwayDesignatorPriority: Record<string, number> = {
  L: 0,
  C: 1,
  R: 2,
  T: 3,
};

export class MsfsChartUtils {
  public static async getChartsForAirport<T extends MsfsProviders>(
    airportIcao: string,
    msfsProvider: T,
  ): Promise<Record<MsfsChartCategory, MsfsChartData<T>[]>> {
    const icao = IcaoCodes.make('A', '', '', airportIcao);

    const chartIndex = await Charts.listFor(icao, msfsProvider);

    const charts: Record<MsfsChartCategory, MsfsChartData<T>[]> = {
      SID: [],
      STAR: [],
      APPR: [],
      ARPT: [],
    };

    const categoryMap = msfsProvider === ChartOrigin.Faa ? faaCategoryMap : lidoCategoryMap;

    for (const chartCategory of chartIndex.charts) {
      for (const chart of chartCategory.charts) {
        let category = categoryMap[chart.type];
        if (!category) {
          console.warn('[MsfsChartUtils::getChartsForAirport] Encountered an unknown chart type', chart.type);
          category = 'ARPT';
        }

        charts[categoryMap[chart.type]].push({
          guid: chart.guid,
          name: chart.name,
          type: chart.type as MsfsChartData<T>['type'],
          runways: chart.runways
            .slice()
            .sort((a, b) =>
              a.number === b.number
                ? runwayDesignatorPriority[a.designator] - runwayDesignatorPriority[b.designator]
                : a.number.localeCompare(b.number),
            ),
        });
      }
    }

    charts['SID'].sort(MsfsChartUtils.sortChart);
    charts['STAR'].sort(MsfsChartUtils.sortChart);
    charts['APPR'].sort(MsfsChartUtils.sortChart);
    charts['ARPT'].sort(MsfsChartUtils.sortChart);

    return charts;
  }

  public static async getChartPages(guid: string): Promise<MsfsChartPage[]> {
    const chartPages = await Charts.pagesOf(guid);

    return chartPages.pages.map((p) => {
      const lightUrl = p.urls.find((u) => u.name === 'light_png')?.url;
      const darkUrl = p.urls.find((u) => u.name === 'dark_png')?.url;
      return {
        lightUrl: (lightUrl ?? darkUrl)!,
        darkUrl,
      } satisfies MsfsChartPage;
    });
  }

  private static chartOrder: Record<FaaChart | LidoChart, number> = {
    [FaaChart.Apd]: 0,
    [FaaChart.Dau]: 4,
    [FaaChart.Dp]: 0,
    [FaaChart.Hot]: 1,
    [FaaChart.Iap]: 0,
    [FaaChart.Lah]: 2,
    [FaaChart.Min]: 3,
    [FaaChart.Odp]: 1,
    [FaaChart.Star]: 0,
    [FaaChart.Unknown]: 9999,
    [LidoChart.Afc]: 0,
    [LidoChart.Agc]: 1,
    [LidoChart.Aoi]: 2,
    [LidoChart.Apc]: 3,
    [LidoChart.Iac]: 0,
    [LidoChart.Lvc]: 5,
    [LidoChart.Mrc]: 4,
    [LidoChart.ObstDep]: 3,
    [LidoChart.Sid]: 0,
    [LidoChart.SidInitialClimb]: 1,
    [LidoChart.SidPt]: 2,
    [LidoChart.Vac]: 1,
  };

  private static isLidoSidChart = (a: MsfsChartData<ChartOrigin>): boolean => {
    return a.type === LidoChart.Sid || a.type === LidoChart.SidInitialClimb || a.type === LidoChart.SidPt;
  };

  private static sortChart = (
    a: MsfsChartData<ChartOrigin>,
    b: MsfsChartData<ChartOrigin>,
  ): number => {
    // group SID/SIDPT
    if (MsfsChartUtils.isLidoSidChart(a) && MsfsChartUtils.isLidoSidChart(b) && a.name !== b.name) {
      return a.name.localeCompare(b.name);
    }

    const order = MsfsChartUtils.chartOrder[a.type] - MsfsChartUtils.chartOrder[b.type];
    if (order !== 0) {
      return order;
    }

    return a.name.localeCompare(b.name);
  };
}
