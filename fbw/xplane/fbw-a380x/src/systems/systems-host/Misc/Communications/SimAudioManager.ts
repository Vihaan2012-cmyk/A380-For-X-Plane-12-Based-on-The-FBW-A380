// @ts-strict-ignore
// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { PilotSeat, PilotSeatEvents } from '@flybywiresim/fbw-sdk';
import {
  BusValue,
  Bus,
  BoardDevice,
  KeyControl,
  Writable,
  VarUnit,
  Signal,
  Bond,
} from 'xplane-js/ui';

import { AudioManagementUnit, ComIndex } from './AudioManagementUnit';
import { RadioNavSelectedNavaid, RmpAmuBusEvents } from './RmpAmuBusPublisher';
import { Vars } from 'xplane-js/data';

interface NavaidDefinition {
  setVolume: (volume: number, amuIndex: number) => Promise<unknown>;
  setIdent: (on: boolean, amuIndex: number) => Promise<unknown>;
}

interface NavaidState extends NavaidDefinition {
  isIdentOn: Writable<boolean>;
  volume: Writable<number>;
  subs: Bond[];
}

interface ComDefinition {
  setVolume: (volume: number) => Promise<unknown>;
  setReceive: (on: boolean) => Promise<unknown>;
  setTransmit: (on: boolean, amuIndex: number) => Promise<unknown>;
}

interface ComState extends ComDefinition {
  isReceiveOn: Writable<boolean>;
  isTransmitOn: Writable<boolean>;
  volume: Writable<number>;
  subs: Bond[];
}

enum SimTransmitStates {
  Com1 = 0,
  Com2 = 1,
  Com3 = 2,
  None = 4,
}

/**
 * Manages the sim's audio events to match the state of the AMUs.
 */
export class SimAudioManager implements BoardDevice {
  private readonly navaidDefinitions: Record<RadioNavSelectedNavaid, NavaidDefinition> = {
    [RadioNavSelectedNavaid.Adf1]: {
      setVolume: (volume) => this.keyEventManager?.fire('ADF_VOLUME_SET', true, volume),
      setIdent: (on) => this.keyEventManager?.fire('RADIO_ADF_IDENT_SET', true, on ? 1 : 0),
    },
    [RadioNavSelectedNavaid.Adf2]: {
      setVolume: (volume) => this.keyEventManager?.fire('ADF2_VOLUME_SET', true, volume),
      setIdent: (on) => this.keyEventManager?.fire('RADIO_ADF2_IDENT_SET', true, on ? 1 : 0),
    },
    [RadioNavSelectedNavaid.Ls]: {
      setVolume: (volume, amuIndex) => {
        return Promise.all([
          this.keyEventManager?.fire('NAV3_VOLUME_SET_EX1', true, amuIndex === 2 ? 0 : volume),
          this.keyEventManager?.fire('NAV4_VOLUME_SET_EX1', true, amuIndex === 2 ? volume : 0),
        ]);
      },
      setIdent: (on, amuIndex) => {
        return Promise.all([
          this.keyEventManager?.fire('RADIO_VOR3_IDENT_SET', true, amuIndex === 2 ? 0 : on ? 1 : 0),
          this.keyEventManager?.fire('RADIO_VOR4_IDENT_SET', true, amuIndex === 2 ? (on ? 1 : 0) : 0),
        ]);
      },
    },
    [RadioNavSelectedNavaid.Vor1]: {
      setVolume: (volume) => this.keyEventManager?.fire('NAV1_VOLUME_SET_EX1', true, volume),
      setIdent: (on) => this.keyEventManager?.fire('RADIO_VOR1_IDENT_SET', true, on ? 1 : 0),
    },
    [RadioNavSelectedNavaid.Vor2]: {
      setVolume: (volume) => this.keyEventManager?.fire('NAV2_VOLUME_SET_EX1', true, volume),
      setIdent: (on) => this.keyEventManager?.fire('RADIO_VOR2_IDENT_SET', true, on ? 1 : 0),
    },
    [RadioNavSelectedNavaid.Mkr]: {
      setVolume: () => Promise.resolve(),
      setIdent: (on) => {
        const wasOn = Vars.read('fbw/MARKER_SOUND', VarUnit.Bool) > 0;
        if (on !== wasOn) {
          return this.keyEventManager?.fire('MARKER_SOUND_TOGGLE', true);
        }
        return Promise.resolve();
      },
    },
  };

  private readonly comDefinitions: Record<ComIndex, ComDefinition> = {
    [ComIndex.Vhf1]: {
      setVolume: (volume) => this.keyEventManager?.fire('COM1_VOLUME_SET', true, volume),
      setReceive: (on) => this.keyEventManager?.fire('COM1_RECEIVE_SELECT', true, on ? 1 : 0),
      setTransmit: (on, amuIndex) => {
        return Promise.all([
          this.keyEventManager?.fire(
            amuIndex === 2 ? 'COPILOT_TRANSMITTER_SET' : 'PILOT_TRANSMITTER_SET',
            true,
            on ? SimTransmitStates.Com1 : SimTransmitStates.None,
          ),
          this.keyEventManager?.fire(
            amuIndex === 2 ? 'PILOT_TRANSMITTER_SET' : 'COPILOT_TRANSMITTER_SET',
            true,
            SimTransmitStates.None,
          ),
        ]);
      },
    },
    [ComIndex.Vhf2]: {
      setVolume: (volume) => this.keyEventManager?.fire('COM2_VOLUME_SET', true, volume),
      setReceive: (on) => this.keyEventManager?.fire('COM2_RECEIVE_SELECT', true, on ? 1 : 0),
      setTransmit: (on, amuIndex) => {
        return Promise.all([
          this.keyEventManager?.fire(
            amuIndex === 2 ? 'COPILOT_TRANSMITTER_SET' : 'PILOT_TRANSMITTER_SET',
            true,
            on ? SimTransmitStates.Com2 : SimTransmitStates.None,
          ),
          this.keyEventManager?.fire(
            amuIndex === 2 ? 'PILOT_TRANSMITTER_SET' : 'COPILOT_TRANSMITTER_SET',
            true,
            SimTransmitStates.None,
          ),
        ]);
      },
    },
    [ComIndex.Vhf3]: {
      setVolume: (volume) => this.keyEventManager?.fire('COM3_VOLUME_SET', true, volume),
      setReceive: (on) => this.keyEventManager?.fire('COM3_RECEIVE_SELECT', true, on ? 1 : 0),
      setTransmit: (on, amuIndex) => {
        return Promise.all([
          this.keyEventManager?.fire(
            amuIndex === 2 ? 'COPILOT_TRANSMITTER_SET' : 'PILOT_TRANSMITTER_SET',
            true,
            on ? SimTransmitStates.Com3 : SimTransmitStates.None,
          ),
          this.keyEventManager?.fire(
            amuIndex === 2 ? 'PILOT_TRANSMITTER_SET' : 'COPILOT_TRANSMITTER_SET',
            true,
            SimTransmitStates.None,
          ),
        ]);
      },
    },
  };

  private readonly sub = this.bus.inbox<PilotSeatEvents & RmpAmuBusEvents>();

  private activeAmuIndex = 1;

  private readonly navaidStates: Map<RadioNavSelectedNavaid, NavaidState> = new Map(
    Object.entries(this.navaidDefinitions).map(([key, def]) => {
      const isIdentOn = Signal.of<boolean>(false);
      const volume = Signal.of<number>(0);

      const subs = [
        isIdentOn.watch((v) => def.setIdent(v === true, this.activeAmuIndex), true, true),
        volume.watch((v) => def.setVolume(v, this.activeAmuIndex), true, true),
      ];

      return [parseInt(key), { ...def, isIdentOn, volume, subs }];
    }),
  );

  private readonly comStates: Map<ComIndex, ComState> = new Map(
    Object.entries(this.comDefinitions).map(([key, def]) => {
      const isReceiveOn = Signal.of<boolean>(false);
      const isTransmitOn = Signal.of<boolean>(false);
      const volume = Signal.of<number>(0);

      const subs = [
        isReceiveOn.watch((v) => def.setReceive(v === true), true, true),
        isTransmitOn.watch((v) => def.setTransmit(v === true, this.activeAmuIndex), true, true),
        volume.watch((v) => def.setVolume(v), true, true),
      ];

      return [parseInt(key), { ...def, isReceiveOn, isTransmitOn, volume, subs }];
    }),
  );

  private readonly pilotSeat = BusValue.from(this.sub.topic('pilot_seat'), PilotSeat.Left);

  private keyEventManager?: KeyControl;

  private isInit = false;

  constructor(
    private readonly bus: Bus,
    private readonly amu1: AudioManagementUnit,
    private readonly amu2: AudioManagementUnit,
  ) {
    KeyControl.connect(bus).then((manager) => {
      this.keyEventManager = manager;
      if (this.isInit) {
        this.resume();
      }
    });
  }

  setup(): void {
    if (this.keyEventManager) {
      this.resume();
    }
    this.isInit = true;
  }

  private resume(): void {
    for (const navaid of this.navaidStates.values()) {
      for (const sub of navaid.subs) {
        sub.release(true);
      }
    }

    for (const com of this.comStates.values()) {
      for (const sub of com.subs) {
        sub.release(true);
      }
    }
  }

  tick(): void {
    this.activeAmuIndex = !this.amu1.isHealthy.get() || this.pilotSeat.get() === PilotSeat.Right ? 2 : 1;
    const activeAmu = this.activeAmuIndex === 2 ? this.amu2 : this.amu1;
    const isAmuHealthy = activeAmu.isHealthy.get();

    const activeNavOutput = isAmuHealthy ? activeAmu.getActiveNavAudio() : null;

    for (const [navaid, state] of this.navaidStates.entries()) {
      if (navaid === activeNavOutput) {
        state.isIdentOn.set(activeAmu.isNavOutputOn() && !activeAmu.isNavVoiceFiltered());
        state.volume.set(activeAmu.getNavVolume());
      } else {
        state.isIdentOn.set(false);
        state.volume.set(0);
      }
    }

    for (const [index, state] of this.comStates.entries()) {
      if (!isAmuHealthy || !activeAmu.isComReceiveOn(index)) {
        state.isReceiveOn.set(false);
      }
      if (!isAmuHealthy || activeAmu.getSelectedComTransmitter() !== index) {
        state.isTransmitOn.set(false);
      }
      state.volume.set(isAmuHealthy ? activeAmu.getComVolume(index) : 0);
    }

    // need to set on states after to ensure correct event ordering
    if (isAmuHealthy) {
      for (const [index, state] of this.comStates.entries()) {
        if (activeAmu.isComReceiveOn(index)) {
          state.isReceiveOn.set(true);
        }
        if (activeAmu.getSelectedComTransmitter() === index) {
          state.isTransmitOn.set(true);
        }
      }
    }
  }
}
