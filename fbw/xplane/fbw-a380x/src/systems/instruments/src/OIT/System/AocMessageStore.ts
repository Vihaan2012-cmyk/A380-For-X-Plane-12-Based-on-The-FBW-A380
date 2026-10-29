// Copyright (c) 2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { StateList as SignalList, TickTopics as ClockTopics, BusState as BusSignal, Exchange as Bus, makeBusState as $feed } from 'xplane-js/ui';

export type AocFolder =
  | 'INBOX'
  | 'OPS'
  | 'METAR'
  | 'SENT'
  | 'REQUESTS'
  | 'LOADSHEET'
  | 'REPORTS'
  | 'ADC DELAY'
  | 'DIVERSION';

export interface AocMessage {
  id: number;
  folder: AocFolder;
  title: string;
  from: string;
  zulu: string;
  timeMs: number;
  lines: string[];
  isNew: boolean;
}

export class AocMessageStore {
  private static readonly instances = new Map<Bus, AocMessageStore>();

  public readonly messages = SignalList.of<AocMessage>([]);

  private nextId = 1;

  private readonly zuluTime: BusSignal<number>;

  private constructor(private readonly bus: Bus) {
    const sub = this.bus.receiver<ClockTopics>();
    this.zuluTime = $feed(sub.topic('simTime'), Date.now());
  }

  static instance(bus: Bus): AocMessageStore {
    let inst = AocMessageStore.instances.get(bus);
    if (!inst) {
      inst = new AocMessageStore(bus);
      AocMessageStore.instances.set(bus, inst);
    }
    return inst;
  }

  private currentZulu(): string {
    const date = new Date(this.zuluTime.get());
    return `${String(date.getUTCHours()).padStart(2, '0')}${String(date.getUTCMinutes()).padStart(2, '0')}`;
  }

  add(folder: AocFolder, title: string, from: string, lines: string[]): AocMessage {
    const message: AocMessage = {
      id: this.nextId++,
      folder,
      title,
      from,
      zulu: this.currentZulu(),
      timeMs: this.zuluTime.get(),
      lines,
      isNew: true,
    };
    this.messages.insert(message, 0);
    return message;
  }

  markRead(id: number): void {
    const arr = this.messages.all();
    const index = arr.findIndex((m) => m.id === id);
    if (index >= 0 && arr[index].isNew) {
      const updated: AocMessage = { ...arr[index], isNew: false };
      this.messages.removeAt(index);
      this.messages.insert(updated, index);
    }
  }

  clear(folder: AocFolder): void {
    const arr = this.messages.all();
    for (let i = arr.length - 1; i >= 0; i--) {
      if (arr[i].folder === folder) {
        this.messages.removeAt(i);
      }
    }
  }
}
