// @ts-strict-ignore
import { Bus, Inbox, Outbox } from 'xplane-js/ui';
import { Arinc429Register, Arinc429SignStatusMatrix } from '@flybywiresim/fbw-sdk';
import {
  AtisMessage,
  AtisType,
  AtsuMessage,
  AtsuMessageSerializationFormat,
  AtsuMessageType,
  AtsuStatusCodes,
  CpdlcMessage,
  DclMessage,
  FansMode,
  OclMessage,
  SimVarSources,
  WeatherMessage,
} from '../../common/src';
import { AtcAocRouterMessages } from '../../router/src';
import { AtcAocMessages } from './databus/AtcAocBus';
import { AtcFmsMessages } from './databus/FmsBus';
import { Bridge, Vars } from 'xplane-js/data';

export class DigitalOutputs {
  private requestId: number = 0;

  private subscriber: Inbox<AtcAocRouterMessages> = null;

  private publisher: Outbox<AtcAocMessages & AtcAocRouterMessages & AtcFmsMessages> = null;

  private sendMessageCallbacks: ((requestId: number, code: AtsuStatusCodes) => boolean)[] = [];

  private requestSentCallbacks: ((requestId: number) => boolean)[] = [];

  private weatherResponseCallbacks: ((requestId: number, response: [AtsuStatusCodes, WeatherMessage]) => boolean)[] =
    [];

  private static rmpFrequencyArinc: Arinc429Register = Arinc429Register.empty();

  constructor(
    private readonly bus: Bus,
    private readonly synchronizedAoc: boolean,
    private readonly synchronizedRouter: boolean,
  ) {
    this.subscriber = this.bus.inbox<AtcAocRouterMessages>();
    this.publisher = this.bus.outbox<AtcAocMessages & AtcAocRouterMessages & AtcFmsMessages>();

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
  }

  public powerDown(): void {
    this.publisher.post('atcResetData', true, true, false);
    DigitalOutputs.rmpFrequencyArinc = Arinc429Register.empty();
  }

  private async sendCpdlcMessage(message: CpdlcMessage, force: boolean): Promise<AtsuStatusCodes> {
    return new Promise<AtsuStatusCodes>((resolve, _reject) => {
      const requestId = this.requestId++;
      this.publisher.post('routerSendCpdlcMessage', { requestId, message, force }, this.synchronizedRouter, false);
      this.sendMessageCallbacks.push((id: number, code: AtsuStatusCodes) => {
        if (id === requestId) resolve(code);
        return id === requestId;
      });
    });
  }

  private async sendDclMessage(message: DclMessage, force: boolean): Promise<AtsuStatusCodes> {
    return new Promise<AtsuStatusCodes>((resolve, _reject) => {
      const requestId = this.requestId++;
      this.publisher.post('routerSendDclMessage', { requestId, message, force }, this.synchronizedRouter, false);
      this.sendMessageCallbacks.push((id: number, code: AtsuStatusCodes) => {
        if (id === requestId) resolve(code);
        return id === requestId;
      });
    });
  }

  private async sendOclMessage(message: OclMessage, force: boolean): Promise<AtsuStatusCodes> {
    return new Promise<AtsuStatusCodes>((resolve, _reject) => {
      const requestId = this.requestId++;
      this.publisher.post('routerSendOclMessage', { requestId, message, force }, this.synchronizedRouter, false);
      this.sendMessageCallbacks.push((id: number, code: AtsuStatusCodes) => {
        if (id === requestId) resolve(code);
        return id === requestId;
      });
    });
  }

  public async sendMessage(message: AtsuMessage, force: boolean): Promise<AtsuStatusCodes> {
    switch (message.Type) {
      case AtsuMessageType.CPDLC:
        return this.sendCpdlcMessage(message as CpdlcMessage, force);
      case AtsuMessageType.DCL:
        return this.sendDclMessage(message as DclMessage, force);
      case AtsuMessageType.OCL:
        return this.sendOclMessage(message as OclMessage, force);
      default:
        return new Promise<AtsuStatusCodes>((resolve, _reject) => resolve(AtsuStatusCodes.UnknownMessage));
    }
  }

  public sendRmpFrequency(frequency: number): void {
    DigitalOutputs.rmpFrequencyArinc.setSsm(Arinc429SignStatusMatrix.NormalOperation);
    DigitalOutputs.rmpFrequencyArinc.setValue(frequency);
    DigitalOutputs.rmpFrequencyArinc.writeToSimVar('L:A32NX_ATSU_RMP_FREQUENCY');
  }

  public resetRmpFrequency(): void {
    DigitalOutputs.rmpFrequencyArinc.setSsm(Arinc429SignStatusMatrix.NoComputedData);
    DigitalOutputs.rmpFrequencyArinc.writeToSimVar('L:A32NX_ATSU_RMP_FREQUENCY');
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

  public sendAocIgnoreMessageId(uid: number): void {
    if (this.publisher !== null) {
      this.publisher.post('ignoreIncomingAts623Message', uid, this.synchronizedAoc, false);
    }
  }

  public activateAtcRing(): void {
    Bridge.call('sound.play', 'cpdlc_ring');
    // ensure that the timeout is longer than the sound
    setTimeout(() => Vars.write('W:cpdlc_ring', 'boolean', 0), 2000);
  }

  public sendSystemStatus(status: AtsuStatusCodes): void {
    this.publisher.post('atcSystemStatus', status, true, false);
  }

  public sendMessageModify(message: CpdlcMessage): void {
    this.publisher.post('atcMessageModify', message, true, false);
  }

  // FIXME when sending the object via the event bus, the prototype is lost, so AtsuMessage methods are not available anymores
  // String works, but we should find a better way to do this
  public sendPrintMessage(message: AtsuMessage): void {
    this.publisher.post('atcPrintMessage', message.serialize(AtsuMessageSerializationFormat.Printer), true, false);
  }

  public sendActiveAtisAutoUpdates(icaos: string[]): void {
    this.publisher.post('atcActiveAtisAutoUpdates', icaos, true, false);
  }

  public sendAtcAtisReports(reports: AtisMessage[]): void {
    this.publisher.post('atcAtisReports', reports, true, false);
  }

  public sendPrintAtisReportsPrint(active: boolean): void {
    this.publisher.post('atcPrintAtisReportsPrint', active, true, false);
  }

  public sendAtcConnectionStatus(status: {
    current: string;
    next: string;
    notificationTime: number;
    mode: FansMode;
    logonInProgress: boolean;
  }): void {
    this.publisher.post('atcStationStatus', status, true, false);
  }

  public sendMonitoredMessages(messages: CpdlcMessage[]): void {
    this.publisher.post('atcMonitoredMessages', messages, true, false);
  }

  public sendMaxUplinkDelay(delay: number): void {
    this.publisher.post('atcMaxUplinkDelay', delay, true, false);
  }

  public sendAutomaticPositionReportActive(active: boolean): void {
    this.publisher.post('atcAutomaticPositionReportActive', active, true, false);
  }

  public deleteMessage(uid: number): void {
    this.publisher.post('atcDeleteMessage', uid, true, false);
  }

  public resynchronizeAtcMessage(message: CpdlcMessage): void {
    if (message.Type === AtsuMessageType.DCL) {
      this.publisher.post('atcResynchronizeDclMessage', message as DclMessage, true, false);
    } else if (message.Type === AtsuMessageType.OCL) {
      this.publisher.post('atcResynchronizeOclMessage', message as OclMessage, true, false);
    } else {
      this.publisher.post('atcResynchronizeCpdlcMessage', message, true, false);
    }
  }

  public activateButton(): void {
    Vars.write(SimVarSources.atcMessageButtonActive, 'Bool', true);
  }

  public resetButton(): void {
    Vars.write(SimVarSources.atcMessageButtonActive, 'Bool', false);
    Vars.write(SimVarSources.atcMessageButtonPressed, 'Number', 0);
  }
}
