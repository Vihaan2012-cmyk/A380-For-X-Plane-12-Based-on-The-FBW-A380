//  Copyright (c) 2023 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { Bus, Outbox } from 'xplane-js/ui';
import { CpdlcMessage, DatalinkModeCode, DatalinkStatusCode, FreetextMessage } from '../../common/src';
import { RouterAtcAocMessages, RouterFmsMessages } from './databus';

export class DigitalOutputs {
  private publisher: Outbox<RouterAtcAocMessages & RouterFmsMessages>;

  constructor(
    private readonly bus: Bus,
    private readonly synchronizedAtc: boolean,
    private readonly synchronizedAoc: boolean,
  ) {
    this.publisher = this.bus.outbox<RouterAtcAocMessages & RouterFmsMessages>();
  }

  public receivedFreetextMesage(message: FreetextMessage): void {
    this.publisher.post('routerReceivedFreetextMessage', message, this.synchronizedAoc || this.synchronizedAtc, false);
  }

  public receivedCpdlcMessage(message: CpdlcMessage): void {
    this.publisher.post('routerReceivedCpdlcMessage', message, this.synchronizedAtc, false);
  }

  public sendDatalinkStatus(status: {
    vhf: DatalinkStatusCode;
    satellite: DatalinkStatusCode;
    hf: DatalinkStatusCode;
  }): void {
    this.publisher.post('routerDatalinkStatus', status, true, false);
  }

  public sendDatalinkMode(mode: { vhf: DatalinkModeCode; satellite: DatalinkModeCode; hf: DatalinkModeCode }): void {
    this.publisher.post('routerDatalinkMode', mode, true, false);
  }
}
