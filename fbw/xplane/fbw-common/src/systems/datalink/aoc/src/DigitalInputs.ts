// @ts-strict-ignore
//  Copyright (c) 2023 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { Bus, Inbox, Outbox } from 'xplane-js/ui';
import { AocFmsMessages, FmsAocMessages } from './databus/FmsBus';
import {
  FreetextMessage,
  WeatherMessage,
  AtsuMessage,
  WindUplinkMessage,
  AtisType,
  WindRequestMessage,
} from '../../common/src/messages';
import { AtsuStatusCodes } from '../../common/src/AtsuStatusCodes';
import { AtcAocMessages } from '../../atc/src/databus/AtcAocBus';
import { FwcDataBusTypes } from '../../common/src/databus/FwcBus';
import { ClockDataBusTypes } from '../../common/src/databus/ClockBus';
import { RouterAtcAocMessages } from '../../router/src';
import { Clock } from '../../common/src/types/Clock';
import { logTroubleshootingError } from '../../../shared/src';

export type AocDigitalInputCallbacks = {
  receivedFreetextMessage: (message: FreetextMessage) => void;
  ignoreIncomingMessage: (uid: number) => void;
  sendFreetextMessage: (message: FreetextMessage) => Promise<AtsuStatusCodes>;
  requestAtis: (icao: string, type: AtisType, sentCallback: () => void) => Promise<[AtsuStatusCodes, WeatherMessage]>;
  requestWeather: (
    icaos: string[],
    requestMetar: boolean,
    sentCallback: () => void,
  ) => Promise<[AtsuStatusCodes, WeatherMessage]>;
  registerMessages: (messages: AtsuMessage[]) => void;
  messageRead: (messageId: number) => void;
  removeMessage: (messageId: number) => void;
  requestWinds: (
    request: WindRequestMessage,
    sentCallback: () => void,
  ) => Promise<[AtsuStatusCodes, WindUplinkMessage | null]>;
};

export class DigitalInputs {
  private callbacks: AocDigitalInputCallbacks = {
    receivedFreetextMessage: null,
    ignoreIncomingMessage: null,
    sendFreetextMessage: null,
    requestAtis: null,
    requestWeather: null,
    registerMessages: null,
    messageRead: null,
    removeMessage: null,
    requestWinds: null,
  };

  private subscriber: Inbox<
    AtcAocMessages & ClockDataBusTypes & FmsAocMessages & FwcDataBusTypes & RouterAtcAocMessages
  > = null;

  private publisher: Outbox<AocFmsMessages> = null;

  private poweredUp: boolean = false;

  public UtcClock: Clock;

  public CompanyMessageCount: number = 0;

  private resetData(): void {
    this.UtcClock = new Clock(0, 0, 0, 0, 0, 0, 0);
    this.CompanyMessageCount = 0;
  }

  constructor(private readonly bus: Bus) {
    this.resetData();
  }

  public initialize(): void {
    this.subscriber = this.bus.inbox<
      AtcAocMessages & ClockDataBusTypes & FmsAocMessages & FwcDataBusTypes & RouterAtcAocMessages
    >();
    this.publisher = this.bus.outbox<AocFmsMessages>();

    this.subscriber.topic('utcYear').listen((year: number) => {
      if (this.poweredUp) this.UtcClock.year = year;
    });
    this.subscriber.topic('utcMonth').listen((month: number) => {
      if (this.poweredUp) this.UtcClock.month = month;
    });
    this.subscriber.topic('utcDayOfMonth').listen((dayOfMonth: number) => {
      if (this.poweredUp) this.UtcClock.dayOfMonth = dayOfMonth;
    });
    this.subscriber.topic('utcHour').listen((hour: number) => {
      if (this.poweredUp) this.UtcClock.hour = hour;
    });
    this.subscriber.topic('utcMinute').listen((minute: number) => {
      if (this.poweredUp) this.UtcClock.minute = minute;
    });
    this.subscriber.topic('utcSecond').listen((second: number) => {
      if (this.poweredUp) this.UtcClock.second = second;
    });
    this.subscriber.topic('utcSecondsOfDay').listen((seconds: number) => {
      if (this.poweredUp) this.UtcClock.secondsOfDay = seconds;
    });
    this.subscriber.topic('companyMessageCount').listen((count: number) => {
      if (this.poweredUp) this.CompanyMessageCount = count;
    });
    this.subscriber.topic('routerReceivedFreetextMessage').listen((data) => {
      if (this.callbacks.receivedFreetextMessage !== null) {
        this.callbacks.receivedFreetextMessage(data);
      }
    });
    this.subscriber.topic('ignoreIncomingAts623Message').listen((uid: number) => {
      if (this.callbacks.ignoreIncomingMessage !== null) {
        this.callbacks.ignoreIncomingMessage(uid);
      }
    });
    this.subscriber.topic('aocSendFreetextMessage').listen((data) => {
      if (this.callbacks.sendFreetextMessage !== null) {
        this.callbacks.sendFreetextMessage(data.message).then((status) => {
          this.publisher.post('aocTransmissionResponse', { requestId: data.requestId, status }, true, false);
        });
      }
    });
    this.subscriber.topic('aocRequestAtis').listen((data) => {
      if (this.callbacks.requestAtis !== null) {
        this.callbacks
          .requestAtis(data.icao, data.type, () =>
            this.publisher.post('aocRequestSentToGround', data.requestId, true, false),
          )
          .then((response) => {
            this.publisher.post('aocWeatherResponse', { requestId: data.requestId, data: response }, true, false);
          });
      }
    });
    this.subscriber.topic('aocRequestWeather').listen((data) => {
      if (this.callbacks.requestWeather !== null) {
        this.callbacks
          .requestWeather(data.icaos, data.requestMetar, () =>
            this.publisher.post('aocRequestSentToGround', data.requestId, true, false),
          )
          .then((response) => {
            this.publisher.post('aocWeatherResponse', { requestId: data.requestId, data: response }, true, false);
          });
      }
    });
    this.subscriber.topic('aocRegisterFreetextMessages').listen((messages) => {
      if (this.callbacks.registerMessages !== null) {
        this.callbacks.registerMessages(messages);
      }
    });
    this.subscriber.topic('aocRegisterWeatherMessages').listen((messages) => {
      if (this.callbacks.registerMessages !== null) {
        this.callbacks.registerMessages(messages);
      }
    });
    this.subscriber.topic('aocMessageRead').listen((messageId) => {
      if (this.callbacks.messageRead !== null) {
        this.callbacks.messageRead(messageId);
      }
    });
    this.subscriber.topic('aocRemoveMessage').listen((messageId) => {
      if (this.callbacks.removeMessage !== null) {
        this.callbacks.removeMessage(messageId);
      }
    });
    this.subscriber.topic('aocRequestWinds').listen((request) => {
      if (this.callbacks.requestWinds !== null) {
        this.callbacks
          .requestWinds(request, () => {})
          .then((data) => {
            this.publisher.post('aocWindsResponse', { requestId: request.requestId, data }, true, false);
          })
          .catch((error) => {
            this.logTroubleshootingError(`[AOC/WindUplink] Error during wind request: ${error}`);
            this.publisher.post(
              'aocWindsResponse',
              { requestId: request.requestId, data: [AtsuStatusCodes.ComFailed, null] },
              true,
              false,
            );
          });
      }
    });
  }

  public powerUp(): void {
    this.poweredUp = true;
  }

  public powerDown(): void {
    this.poweredUp = false;
    this.resetData();
  }

  public addDataCallback<K extends keyof AocDigitalInputCallbacks>(
    event: K,
    callback: AocDigitalInputCallbacks[K],
  ): void {
    this.callbacks[event] = callback;
  }

  private logTroubleshootingError(msg: any) {
    console.error(msg);
    logTroubleshootingError(this.bus, msg);
  }
}
