// @ts-strict-ignore
//  Copyright (c) 2023 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { Bus, Inbox, Outbox } from 'xplane-js/ui';
import { AtcAocRouterMessages } from '../../router/src';
import {
  AtisType,
  AtsuMessage,
  AtsuMessageSerializationFormat,
  AtsuMessageType,
  AtsuStatusCodes,
  FreetextMessage,
  SimVarSources,
  WeatherMessage,
  WindRequestMessage,
  WindUplinkMessage,
} from '../../common/src';
import { AocFmsMessages } from './databus/FmsBus';
import { Vars } from 'xplane-js/data';

export class DigitalOutputs {
  private requestId: number = 0;

  private subscriber: Inbox<AtcAocRouterMessages> = null;

  private publisher: Outbox<AtcAocRouterMessages & AocFmsMessages> = null;

  private sendMessageCallbacks: ((requestId: number, code: AtsuStatusCodes) => boolean)[] = [];

  private requestSentCallbacks: ((requestId: number) => boolean)[] = [];

  private weatherResponseCallbacks: ((requestId: number, response: [AtsuStatusCodes, WeatherMessage]) => boolean)[] =
    [];

  private windRequestCallbacks: ((
    requestId: number,
    response: [AtsuStatusCodes, WindUplinkMessage | null],
  ) => boolean)[] = [];

  constructor(
    private readonly bus: Bus,
    private readonly synchronizedRouter: boolean,
  ) {
    this.subscriber = this.bus.inbox<AtcAocRouterMessages>();
    this.publisher = this.bus.outbox<AtcAocRouterMessages & AocFmsMessages>();

    this.subscriber.topic('routerSendMessageResponse').listen((response) => {
      this.sendMessageCallbacks.every((callback, index) => {
        if (callback(response.requestId, response.status)) {
          this.sendMessageCallbacks.splice(index, 1);
          return false;
        }
        return true;
      });
    });

    this.subscriber.topic('routerRequestSent').listen((response) => {
      this.requestSentCallbacks.every((callback, index) => {
        if (callback(response)) {
          this.requestSentCallbacks.splice(index, 1);
          return false;
        }
        return true;
      });
    });

    this.subscriber.topic('routerReceivedWeather').listen((response) => {
      this.weatherResponseCallbacks.every((callback, index) => {
        if (callback(response.requestId, response.response)) {
          this.weatherResponseCallbacks.splice(index, 1);
          return false;
        }
        return true;
      });
    });

    this.subscriber.topic('routerReceivedWinds').listen((response) => {
      this.windRequestCallbacks.every((callback, index) => {
        if (callback(response.requestId, response.response)) {
          this.windRequestCallbacks.splice(index, 1);
          return false;
        }
        return true;
      });
    });
  }

  public powerDown(): void {
    this.publisher.post('aocResetData', true, true, false);
  }

  public async sendMessage(message: FreetextMessage, force: boolean): Promise<AtsuStatusCodes> {
    return new Promise<AtsuStatusCodes>((resolve, _reject) => {
      const requestId = this.requestId++;
      this.publisher.post('routerSendFreetextMessage', { requestId, message, force }, this.synchronizedRouter, false);
      this.sendMessageCallbacks.push((id: number, code: AtsuStatusCodes) => {
        if (id === requestId) resolve(code);
        return id === requestId;
      });
    });
  }

  public async receiveAtis(
    icao: string,
    type: AtisType,
    sentCallback: () => void,
  ): Promise<[AtsuStatusCodes, WeatherMessage]> {
    return new Promise<[AtsuStatusCodes, WeatherMessage]>((resolve, _reject) => {
      const requestId = this.requestId++;
      this.publisher.post('routerRequestAtis', { requestId, icao, type }, this.synchronizedRouter, false);
      this.requestSentCallbacks.push((id: number) => {
        if (id === requestId) sentCallback();
        return id === requestId;
      });
      this.weatherResponseCallbacks.push((id, response) => {
        if (id === requestId) resolve(response);
        return requestId === id;
      });
    });
  }

  public async receiveMetar(icaos: string[], sentCallback: () => void): Promise<[AtsuStatusCodes, WeatherMessage]> {
    return new Promise<[AtsuStatusCodes, WeatherMessage]>((resolve, _reject) => {
      const requestId = this.requestId++;
      this.publisher.post('routerRequestMetar', { requestId, icaos }, this.synchronizedRouter, false);
      this.requestSentCallbacks.push((id: number) => {
        if (id === requestId) sentCallback();
        return id === requestId;
      });
      this.weatherResponseCallbacks.push((id, response) => {
        if (id === requestId) resolve(response);
        return requestId === id;
      });
    });
  }

  public async receiveTaf(icaos: string[], sentCallback: () => void): Promise<[AtsuStatusCodes, WeatherMessage]> {
    return new Promise<[AtsuStatusCodes, WeatherMessage]>((resolve, _reject) => {
      const requestId = this.requestId++;
      this.publisher.post('routerRequestTaf', { requestId, icaos }, this.synchronizedRouter, false);
      this.requestSentCallbacks.push((id: number) => {
        if (id === requestId) sentCallback();
        return id === requestId;
      });
      this.weatherResponseCallbacks.push((id, response) => {
        if (id === requestId) resolve(response);
        return requestId === id;
      });
    });
  }

  public sendAtsuSystemStatus(status: AtsuStatusCodes): void {
    this.publisher.post('aocSystemStatus', status, true, false);
  }

  // FIXME when sending the object via the event bus, the prototype is lost, so AtsuMessage methods are not available anymores
  public sendPrintMessage(message: AtsuMessage): void {
    this.publisher.post('aocPrintMessage', message.serialize(AtsuMessageSerializationFormat.Printer), true, false);
  }

  public deleteMessage(uid: number): void {
    this.publisher.post('aocDeleteMessage', uid, true, false);
  }

  public resynchronizeAocMessage(message: AtsuMessage): void {
    if (message.Type === AtsuMessageType.Freetext) {
      this.publisher.post('aocResynchronizeFreetextMessage', message as FreetextMessage, true, false);
    } else {
      this.publisher.post('aocResynchronizeWeatherMessage', message as WeatherMessage, true, false);
    }
  }

  public setCompanyMessageCount(count: number): void {
    Vars.write(SimVarSources.companyMessageCount, 'number', count);
  }

  public requestWinds(
    request: WindRequestMessage,
    sentCallback: () => void,
  ): Promise<[AtsuStatusCodes, WindUplinkMessage | null]> {
    return new Promise<[AtsuStatusCodes, WindUplinkMessage | null]>((resolve, _reject) => {
      const requestId = this.requestId++;
      this.publisher.post('routerRequestWinds', { ...request, requestId }, this.synchronizedRouter, false);
      this.requestSentCallbacks.push((id: number) => {
        if (id === requestId) sentCallback();
        return id === requestId;
      });
      this.windRequestCallbacks.push((id, response) => {
        if (id === requestId) resolve(response);
        return requestId === id;
      });
    });
  }
}
