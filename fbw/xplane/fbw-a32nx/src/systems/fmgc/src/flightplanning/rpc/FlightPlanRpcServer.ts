// @ts-strict-ignore
// Copyright (c) 2021-2023 FlyByWire Simulations
//
// SPDX-License-Identifier: GPL-3.0

import { Bus, SessionState, Outbox, Bond } from 'xplane-js/ui';

import { FlightPlanRemoteClientRpcEvents } from '@fmgc/flightplanning/rpc/FlightPlanRpcClient';
import { FlightPlanService } from '@fmgc/flightplanning/FlightPlanService';
import { FlightPlanPerformanceData } from '@fmgc/flightplanning/plans/performance/FlightPlanPerformanceData';
import { SessionPhase } from 'xplane-js/data';

export interface FlightPlanServerRpcEvents {
  flightPlanServer_heartbeat: void;
  flightPlanServer_rpcCommandResponse: [string, any];
  flightPlanServer_rpcCommandErrorResponse: [string, unknown];
}

export class FlightPlanRpcServer<P extends FlightPlanPerformanceData = FlightPlanPerformanceData> {
  private readonly subscriptions: Bond[] = [];

  private readonly pub: Outbox<FlightPlanServerRpcEvents>;

  constructor(
    private readonly bus: Bus,
    private readonly localFlightPlanService: FlightPlanService,
  ) {
    const sub = bus.inbox<FlightPlanRemoteClientRpcEvents<P>>();
    this.pub = bus.outbox<FlightPlanServerRpcEvents>();

    const gameStateSub = SessionState.get().watch((state) => {
      if (state === SessionPhase.flying) {
        this.handleSendHeartbeat();
        setInterval(() => this.handleSendHeartbeat(), 2_500);

        gameStateSub.destroy();
      }
    });

    this.subscriptions.push(
      sub.topic('flightPlanRemoteClient_rpcCommand').listen(([command, id, ...args]) => {
        this.handleRpcCommand(command, id, ...args);
      }),
    );
  }

  public destroy(): void {
    for (const subscription of this.subscriptions) {
      subscription.destroy();
    }
  }

  private handleSendHeartbeat(): void {
    this.pub.post('flightPlanServer_heartbeat', undefined, true, false);
  }

  private async handleRpcCommand(command: string, id: string, ...args: any): Promise<void> {
    try {
      const returnValue = await this.localFlightPlanService[command](...(args as any[]));

      await this.respondToRpcCommand(id, returnValue);
    } catch (e: unknown) {
      console.error('Error while executing flight plan RPC command:');
      console.error(command, id, ...args);
      console.error(e);

      await this.respondToRpcCommandWithError(
        id,
        typeof e === 'object' && 'message' in e && typeof e.message === 'string' ? e.message : e,
      );
    }
  }

  private async respondToRpcCommand(id: string, response: any): Promise<void> {
    this.pub.post('flightPlanServer_rpcCommandResponse', [id, response], true, false);
  }

  private async respondToRpcCommandWithError(id: string, error: unknown): Promise<void> {
    this.pub.post('flightPlanServer_rpcCommandErrorResponse', [id, error], true, false);
  }
}
