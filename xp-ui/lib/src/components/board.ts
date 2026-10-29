import type { BoardFeeder } from 'xplane-js/ui';
import { PanelInstrument, SessionPhase, SoundId } from 'xplane-js/data';

export interface BoardDevice {
  setup(): void;
  tick(): void;
}

export class Board {
  private readonly feeds = new Map<string, BoardFeeder>();
  private readonly parts = new Map<string, BoardDevice>();

  attachFeeder(name: string, publisher: BoardFeeder, override = false): void {
    Board.place(this.feeds, name, publisher, override);
  }

  attachDevice(name: string, instrument: BoardDevice, override = false): void {
    Board.place(this.parts, name, instrument, override);
  }

  private static place<V extends object>(table: Map<string, V>, name: string, item: V, override: boolean): void {
    if (!override) {
      if (table.has(name)) {
        return;
      }
      for (const existing of table.values()) {
        if (existing.constructor === item.constructor) {
          return;
        }
      }
    }
    table.set(name, item);
  }

  setup(): void {
    this.feeds.forEach((p) => p.begin());
    this.parts.forEach((i) => i.setup());
  }

  tick(): void {
    this.feeds.forEach((p) => p.tick?.());
    this.parts.forEach((i) => i.tick());
  }
}

export interface HostedDevice {
  onFrame(): void;
  onPanelEvent(args: string[]): void;
  onSessionChange(oldState: SessionPhase, newState: SessionPhase): void;
  onFlightBegin(): void;
  onSoundDone(soundEventId: SoundId): void;
  onPowered?(): void;
  onUnpowered?(): void;
}

const HostBase: typeof PanelInstrument =
  typeof PanelInstrument !== 'undefined' ? PanelInstrument : (class {} as unknown as typeof PanelInstrument);

export abstract class HostedInstrument<T extends HostedDevice> extends HostBase {
  protected device!: T;

  private built = false;

  abstract createDevice(): T;

  private ensureBuilt(): void {
    if (!this.built) {
      this.device = this.createDevice();
      this.built = true;
    }
  }

  onAttach(): void {
    super.onAttach();
    this.ensureBuilt();
  }

  protected onPrepare(): void {
    super.onPrepare();
    this.ensureBuilt();
  }

  protected onFrame(): void {
    super.onFrame();
    this.ensureBuilt();
    this.device.onFrame();
  }

  onPanelEvent(args: string[]): void {
    super.onPanelEvent(args);
    if (this.built) {
      this.device.onPanelEvent(args);
    }
  }

  protected onSessionChange(oldState: SessionPhase, newState: SessionPhase): void {
    super.onSessionChange(oldState, newState);
    if (this.built) {
      this.device.onSessionChange(oldState, newState);
    }
  }

  protected onFlightBegin(): void {
    super.onFlightBegin();
    if (this.built) {
      this.device.onFlightBegin();
    }
  }

  onSoundDone(soundEventId: SoundId): void {
    super.onSoundDone(soundEventId);
    if (this.built) {
      this.device.onSoundDone(soundEventId);
    }
  }

  onPowered(): void {
    super.onPowered();
    this.ensureBuilt();
  }

  onShutdown(): void {
    super.onShutdown();
    this.ensureBuilt();
  }
}
