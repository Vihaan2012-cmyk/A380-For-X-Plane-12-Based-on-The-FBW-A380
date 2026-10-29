// @ts-strict-ignore
//  Copyright (c) 2023 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { Bus, Inbox, Outbox } from 'xplane-js/ui';
import { Arinc429Word } from '../../../shared/src/arinc429';
import {
  AtisType,
  AtsuStatusCodes,
  CpdlcMessage,
  DclMessage,
  FmgcDataBusTypes,
  FreetextMessage,
  OclMessage,
  WeatherMessage,
  RmpDataBusTypes,
  Conversion,
  WindUplinkMessage,
  WindRequestMessage,
} from '../../common/src';
import { AtcAocRouterMessages, FmsRouterMessages } from './databus';
import { AtsuFlightPhase } from '../../common/src/types/AtsuFlightPhase';
import { logTroubleshootingError } from '../../../shared/src';
import { VhfRadioInterface } from './Router';

export type RouterDigitalInputCallbacks = {
  sendFreetextMessage: (message: FreetextMessage, force: boolean) => Promise<AtsuStatusCodes>;
  sendCpdlcMessage: (message: CpdlcMessage, force: boolean) => Promise<AtsuStatusCodes>;
  sendDclMessage: (message: DclMessage, force: boolean) => Promise<AtsuStatusCodes>;
  sendOclMessage: (message: OclMessage, force: boolean) => Promise<AtsuStatusCodes>;
  requestAtis: (icao: string, type: AtisType, requestSent: () => void) => Promise<[AtsuStatusCodes, WeatherMessage]>;
  requestWeather: (
    icaos: string[],
    metar: boolean,
    requestSent: () => void,
  ) => Promise<[AtsuStatusCodes, WeatherMessage]>;
  connect: (callsign: string) => Promise<AtsuStatusCodes>;
  disconnect: () => Promise<AtsuStatusCodes>;
  stationAvailable: (callsign: string) => Promise<AtsuStatusCodes>;
  requestWinds: (
    request: WindRequestMessage,
    sentCallback: () => void,
  ) => Promise<[AtsuStatusCodes, WindUplinkMessage | null]>;
};

export class DigitalInputs {
  private subscriber: Inbox<AtcAocRouterMessages & FmgcDataBusTypes & FmsRouterMessages & RmpDataBusTypes> =
    null;

  private publisher: Outbox<AtcAocRouterMessages & FmsRouterMessages>;

  private poweredUp: boolean = false;

  private callbacks: RouterDigitalInputCallbacks = {
    sendFreetextMessage: null,
    sendCpdlcMessage: null,
    sendDclMessage: null,
    sendOclMessage: null,
    requestAtis: null,
    requestWeather: null,
    connect: null,
    disconnect: null,
    stationAvailable: null,
    requestWinds: null,
  };

  public FlightPhase: AtsuFlightPhase = AtsuFlightPhase.Preflight;

  public Vhf3Powered: boolean = false;

  public Vhf3DataMode: boolean = false;

  private resetData(): void {
    this.FlightPhase = AtsuFlightPhase.Preflight;
    this.Vhf3Powered = false;
    this.Vhf3DataMode = false;
  }

  constructor(
    private readonly bus: Bus,
    private readonly synchronizedAtc: boolean,
    private readonly synchronizedAoc: boolean,
    private readonly vhfRadios: VhfRadioInterface,
  ) {
    this.resetData();
  }

  public initialize(): void {
    this.subscriber = this.bus.inbox<
      AtcAocRouterMessages & FmgcDataBusTypes & FmsRouterMessages & RmpDataBusTypes
    >();
    this.publisher = this.bus.outbox<AtcAocRouterMessages & FmsRouterMessages>();
  }

  public connectedCallback(): void {
    this.subscriber.topic('routerSendFreetextMessage').listen(async (request) => {
      if (this.callbacks.sendFreetextMessage !== null) {
        this.callbacks
          .sendFreetextMessage(Conversion.messageDataToMessage(request.message) as FreetextMessage, request.force)
          .then((status) => {
            this.publisher.post(
              'routerSendMessageResponse',
              { requestId: request.requestId, status },
              this.synchronizedAoc,
              false,
            );
          });
      } else {
        this.publisher.post(
          'routerSendMessageResponse',
          { requestId: request.requestId, status: AtsuStatusCodes.ComFailed },
          this.synchronizedAoc,
          false,
        );
      }
    });
    this.subscriber.topic('routerSendCpdlcMessage').listen(async (request) => {
      if (this.callbacks.sendCpdlcMessage !== null) {
        this.callbacks
          .sendCpdlcMessage(Conversion.messageDataToMessage(request.message) as CpdlcMessage, request.force)
          .then((status) => {
            this.publisher.post(
              'routerSendMessageResponse',
              { requestId: request.requestId, status },
              this.synchronizedAtc,
              false,
            );
          });
      } else {
        this.publisher.post(
          'routerSendMessageResponse',
          { requestId: request.requestId, status: AtsuStatusCodes.ComFailed },
          this.synchronizedAtc,
          false,
        );
      }
    });
    this.subscriber.topic('routerSendDclMessage').listen(async (request) => {
      if (this.callbacks.sendDclMessage !== null) {
        this.callbacks
          .sendDclMessage(Conversion.messageDataToMessage(request.message) as DclMessage, request.force)
          .then((status) => {
            this.publisher.post(
              'routerSendMessageResponse',
              { requestId: request.requestId, status },
              this.synchronizedAtc,
              false,
            );
          });
      } else {
        this.publisher.post(
          'routerSendMessageResponse',
          { requestId: request.requestId, status: AtsuStatusCodes.ComFailed },
          this.synchronizedAtc,
          false,
        );
      }
    });
    this.subscriber.topic('routerSendOclMessage').listen(async (request) => {
      if (this.callbacks.sendOclMessage !== null) {
        this.callbacks
          .sendOclMessage(Conversion.messageDataToMessage(request.message) as OclMessage, request.force)
          .then((status) => {
            this.publisher.post(
              'routerSendMessageResponse',
              { requestId: request.requestId, status },
              this.synchronizedAtc,
              false,
            );
          });
      } else {
        this.publisher.post(
          'routerSendMessageResponse',
          { requestId: request.requestId, status: AtsuStatusCodes.ComFailed },
          this.synchronizedAtc,
          false,
        );
      }
    });
    this.subscriber.topic('routerRequestAtis').listen(async (request) => {
      if (this.callbacks.requestAtis !== null) {
        const synchronized = this.synchronizedAoc || this.synchronizedAtc;
        this.callbacks
          .requestAtis(request.icao, request.type, () =>
            this.publisher.post('routerRequestSent', request.requestId, synchronized, false),
          )
          .then((response) => {
            this.publisher.post(
              'routerReceivedWeather',
              { requestId: request.requestId, response },
              synchronized,
              false,
            );
          });
      } else {
        this.publisher.post(
          'routerReceivedWeather',
          { requestId: request.requestId, response: [AtsuStatusCodes.ComFailed, null] },
          this.synchronizedAtc,
          false,
        );
      }
    });
    this.subscriber.topic('routerRequestMetar').listen(async (request) => {
      if (this.callbacks.requestAtis !== null) {
        const synchronized = this.synchronizedAoc || this.synchronizedAtc;
        this.callbacks
          .requestWeather(request.icaos, true, () =>
            this.publisher.post('routerRequestSent', request.requestId, synchronized, false),
          )
          .then((response) => {
            this.publisher.post(
              'routerReceivedWeather',
              { requestId: request.requestId, response },
              synchronized,
              false,
            );
          });
      } else {
        this.publisher.post(
          'routerReceivedWeather',
          { requestId: request.requestId, response: [AtsuStatusCodes.ComFailed, null] },
          this.synchronizedAtc,
          false,
        );
      }
    });
    this.subscriber.topic('routerRequestTaf').listen(async (request) => {
      if (this.callbacks.requestAtis !== null) {
        const synchronized = this.synchronizedAoc || this.synchronizedAtc;
        this.callbacks
          .requestWeather(request.icaos, false, () =>
            this.publisher.post('routerRequestSent', request.requestId, synchronized, false),
          )
          .then((response) => {
            this.publisher.post(
              'routerReceivedWeather',
              { requestId: request.requestId, response },
              synchronized,
              false,
            );
          });
      } else {
        this.publisher.post(
          'routerReceivedWeather',
          { requestId: request.requestId, response: [AtsuStatusCodes.ComFailed, null] },
          this.synchronizedAtc,
          false,
        );
      }
    });
    this.subscriber.topic('routerConnect').listen(async (data) => {
      if (this.callbacks.connect !== null) {
        this.callbacks.connect(data.callsign).then((code) => {
          this.publisher.post('routerManagementResponse', { requestId: data.requestId, status: code }, true, false);
        });
      } else {
        this.publisher.post(
          'routerManagementResponse',
          { requestId: data.requestId, status: AtsuStatusCodes.ComFailed },
          true,
          false,
        );
      }
    });
    this.subscriber.topic('routerDisconnect').listen(async (data) => {
      if (this.callbacks.disconnect !== null) {
        this.callbacks.disconnect().then((code) => {
          this.publisher.post('routerManagementResponse', { requestId: data, status: code }, true, false);
        });
      } else {
        this.publisher.post(
          'routerManagementResponse',
          { requestId: data, status: AtsuStatusCodes.ComFailed },
          true,
          false,
        );
      }
    });
    this.subscriber.topic('routerRequestStationAvailable').listen(async (data) => {
      if (this.callbacks.stationAvailable !== null) {
        this.callbacks.stationAvailable(data.callsign).then((code) => {
          this.publisher.post('routerManagementResponse', { requestId: data.requestId, status: code }, true, false);
        });
      } else {
        this.publisher.post(
          'routerManagementResponse',
          { requestId: data.requestId, status: AtsuStatusCodes.ComFailed },
          true,
          false,
        );
      }
    });
    this.subscriber.topic('flightPhase').listen((phase: Arinc429Word) => {
      if (this.poweredUp) {
        if (phase.isNormalOperation()) {
          this.FlightPhase = phase.value as AtsuFlightPhase;
        } else {
          this.FlightPhase = AtsuFlightPhase.Preflight;
        }
      }
    });
    this.subscriber.topic('vhf3Powered').listen((powered: boolean) => (this.Vhf3Powered = powered));
    this.subscriber.topic('routerRequestWinds').listen(async (request) => {
      if (this.callbacks.requestWinds !== null) {
        const synchronized = this.synchronizedAoc || this.synchronizedAtc;
        this.callbacks
          .requestWinds(request, () => this.publisher.post('routerRequestSent', request.requestId, synchronized, false))
          .then((response) => {
            this.publisher.post('routerReceivedWinds', { requestId: request.requestId, response }, synchronized, false);
          })
          .catch((error) => {
            this.logError(`[Router/WindUplink] Error during wind request: ${error}`);
            this.publisher.post(
              'routerReceivedWinds',
              { requestId: request.requestId, response: [AtsuStatusCodes.ComFailed, null] },
              synchronized,
              false,
            );
          });
      } else {
        this.publisher.post(
          'routerReceivedWinds',
          { requestId: request.requestId, response: [AtsuStatusCodes.ComFailed, null] },
          this.synchronizedAtc,
          false,
        );
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

  public addDataCallback<K extends keyof RouterDigitalInputCallbacks>(
    event: K,
    callback: RouterDigitalInputCallbacks[K],
  ): void {
    this.callbacks[event] = callback;
  }

  public setVhf3Datamode(dataMode: boolean): void {
    this.Vhf3DataMode = dataMode;
  }

  private logError(msg: any) {
    console.error(msg);
    logTroubleshootingError(this.bus, msg);
  }
}
