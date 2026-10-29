// Copyright (c) 2023-2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { UI, Derived } from 'xplane-js/ui';
import { FmcInterface } from '../FMC/FmcInterface';
import { FlightPlanIndex } from '@fmgc/flightplanning/FlightPlanManager';

export class CpnyFplnButtonUtils {
  public static cpnyFplnButtonDisabled(fmc: FmcInterface | null, flightPlanIndex: number) {
    return fmc
      ? Derived.from(
          ([uplinkInProgress, forPlan, enginesStarted, hasActiveFlightPlan]) =>
            uplinkInProgress ||
            (flightPlanIndex === FlightPlanIndex.Active &&
              (enginesStarted ||
                hasActiveFlightPlan ||
                (forPlan !== null && forPlan >= FlightPlanIndex.FirstSecondary))),
          fmc.getUplinkInProgress(),
          fmc.getCpnyFplnRequestedForPlan(),
          fmc.enginesWereStarted,
          fmc.hasActiveFlightPlanWithCityPair,
        )
      : Derived.from(() => true);
  }

  public static cpnyFplnButtonLabel(fmc: FmcInterface | null) {
    return fmc
      ? Derived.from(
          ([avail, uplinkInProgress]) => {
            if (uplinkInProgress) {
              return 'REQUEST\nPENDING...';
            } else if (!avail) {
              return 'CPNY F-PLN\nREQUEST';
            } else {
              return 'RECEIVED\nCPNY F-PLN';
            }
          },
          fmc.getCpnyFplnAvailable(),
          fmc.getCpnyFplnUplinkInProgress(),
        )
      : Derived.from(() => <></>);
  }

  public static cpnyFplnButtonMenuItems(fmc: FmcInterface | null, flightPlanIndex: number) {
    return fmc
      ? Derived.from(
          ([avail]) =>
            avail
              ? [
                  {
                    label: 'INSERT*',
                    action: () => fmc.insertCpnyFpln(flightPlanIndex),
                  },
                  {
                    label: 'CLEAR*',
                    action: () => {
                      fmc.deleteCpnyFpln();
                    },
                  },
                ]
              : [],
          fmc.getCpnyFplnAvailable(),
        )
      : Derived.from(() => []);
  }
}
