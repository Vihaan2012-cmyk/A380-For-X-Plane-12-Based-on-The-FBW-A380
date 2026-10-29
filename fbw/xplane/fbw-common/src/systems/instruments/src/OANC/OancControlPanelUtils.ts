// Copyright (c) 2023-2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { SignalList, AngleFormat, Signal, UnitList } from 'xplane-js/ui';
import { AmdbAirportSearchResult, AmdbProperties } from '@flybywiresim/fbw-sdk';

export enum ControlPanelAirportSearchMode {
  Icao,
  Iata,
  City,
}

export enum ControlPanelMapDataSearchMode {
  Runway,
  Taxiway,
  Stand,
  Other,
}

export class ControlPanelUtils {
  static readonly LAT_FORMATTER = AngleFormat.make('{dd}°{mm}.{s}{+[N]-[S]}', UnitList.DEGREE, 0.1);

  static readonly LONG_FORMATTER = AngleFormat.make('{ddd}°{mm}.{s}{+[E]-[W]}', UnitList.DEGREE, 0.1);

  static readonly LETTERS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ'.split('');

  static getSearchModeProp(mode: ControlPanelAirportSearchMode): keyof AmdbAirportSearchResult {
    let prop: keyof AmdbAirportSearchResult;
    switch (mode) {
      default:
      case ControlPanelAirportSearchMode.Icao:
        prop = 'idarpt';
        break;
      case ControlPanelAirportSearchMode.Iata:
        prop = 'iata';
        break;
      case ControlPanelAirportSearchMode.City:
        prop = 'name';
        break;
    }
    return prop;
  }

  static getMapDataSearchModeProp(mode: ControlPanelMapDataSearchMode): keyof AmdbProperties {
    let prop: keyof AmdbProperties;
    switch (mode) {
      default:
      case ControlPanelMapDataSearchMode.Runway:
        prop = 'idthr';
        break;
      case ControlPanelMapDataSearchMode.Taxiway:
        prop = 'idlin';
        break;
      case ControlPanelMapDataSearchMode.Stand:
        prop = 'idstd';
        break;
      case ControlPanelMapDataSearchMode.Other:
        prop = 'ident';
        break;
    }
    return prop;
  }
}

export class ControlPanelStore {
  public readonly airports = SignalList.of<AmdbAirportSearchResult>();

  public readonly sortedAirports = SignalList.of<AmdbAirportSearchResult>();

  public readonly airportSearchMode = Signal.of<number | null>(ControlPanelAirportSearchMode.Icao);

  public readonly airportSearchData = SignalList.of<string>();

  public readonly airportSearchSelectedSearchLetterIndex = Signal.of<number | null>(null);

  public readonly airportSearchSelectedAirportIndex = Signal.of<number | null>(null);

  public readonly mapDataSearchMode = Signal.of<number | null>(ControlPanelMapDataSearchMode.Runway);

  public readonly mapDataSearchData = SignalList.of<string>();

  public readonly selectedAirport = Signal.of<AmdbAirportSearchResult | null>(null);

  public readonly loadedAirport = Signal.of<AmdbAirportSearchResult | null>(null);

  public readonly isAirportSelectionPending = Signal.of(false);
}
