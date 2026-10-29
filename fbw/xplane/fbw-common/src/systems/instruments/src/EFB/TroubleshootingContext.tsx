// @ts-strict-ignore
// Copyright (c) 2025 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { Bus } from 'xplane-js/ui';
import React, { useContext, useEffect, useState } from 'react';
import { logTroubleshootingError, TroubleshootingEvents } from '../../../shared/src/Troubleshooting';
import { useViewListenerEvent } from './Utils/listener';

const TroubleshootingContext = React.createContext<string[]>(undefined as any);

interface TroubleshootingContextProps {
  eventBus: Bus;
}

export const TroubleshootingContextProvider: React.FC<TroubleshootingContextProps> = ({ eventBus, children }) => {
  const [errorLog, setErrorLog] = useState([] as string[]);

  useEffect(() => {
    let log = [];
    const sub = eventBus
      .inbox<TroubleshootingEvents>()
      .topic('troubleshooting_log_error')
      .listen((err) => {
        log = [`${new Date().toISOString()}: ${err}`, ...log];
        setErrorLog(log);
      });
    return () => {
      sub.destroy();
    };
  }, []);

  useViewListenerEvent('JS_LISTENER_COMM_BUS', 'FBW_SYSTEMS_TROUBLESHOOTING_LOG', (message: string) =>
    logTroubleshootingError(eventBus, message),
  );

  return <TroubleshootingContext.Provider value={errorLog}>{children}</TroubleshootingContext.Provider>;
};

export const useTroubleshooting = () => useContext(TroubleshootingContext);
