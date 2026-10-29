// Copyright (c) 2022-2025 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { SimClock, Bus, PanelEventPublisher, Board, VarUnit } from 'xplane-js/ui';

import {
  BaroUnitSelector,
  ExtrasSimVarPublisher,
  FlightDeckBounds,
  GPUManagement,
  GsxSimVarPublisher,
  GsxSyncA380X,
  GroundSupportPublisher,
  MsfsElectricsPublisher,
  MsfsFlightModelPublisher,
  MsfsMiscPublisher,
  NotificationManager,
  PilotSeatManager,
  TelexCheck,
  PilotSeatPublisher,
  MsfsVersionPopupMonitor,
} from '@flybywiresim/fbw-sdk';

import { PushbuttonCheck } from './modules/pushbutton_check/PushbuttonCheck';
import { A380XKeyInterceptor } from './modules/key_interceptor/KeyInterceptor';
import { VersionCheck } from './modules/version_check/VersionCheck';
import { AircraftSync } from './modules/aircraft_sync/AircraftSync';
import { LightSync } from './modules/light_sync/LightSync';
import { NavigationDatabaseService } from '@fmgc/flightplanning/NavigationDatabaseService';
import { NavigationDatabase, NavigationDatabaseBackend } from '@fmgc/NavigationDatabase';
import { PanelInstrument, SessionPhase, Vars, defineInstrument } from 'xplane-js/data';

/**
 * This is the main class for the extras-host instrument.
 *
 * It provides an environment for non-aircraft non-wasm systems/modules to run in.
 *
 * Usage:
 *  - Add new modules as private readonly members of this class.
 *  - Add the modules to the constructor if not constructed in the definition.
 *  - If the modules do implement Instrument or Publisher (preferred):
 *    - Add the modules to the backplane, init and update will be taken care of by the backplane.
 *  - If the modules do not implement Instrument or Publisher:
 *    - Add the modules to the connectedCallback() method.
 *    - Add the modules to the Update() method.
 *
 * Each module not on the backplane must implement the following methods:
 * - `constructor` to get access to the system-wide EventBus
 * - `connectedCallback` which is called after the simulator set up everything. These functions will also add the subscribtion to special events.
 * - `startPublish` which is called as soon as the simulator starts running. It will also start publishing the simulator variables onto the EventBus
 * - `update` is called in every update call of the simulator, but only after `startPublish` is called
 */
class ExtrasHost extends PanelInstrument {
  private static readonly flightDeckBounds: FlightDeckBounds = {
    minX: -0.92,
    maxX: 0.92,
    minY: 2,
    maxY: 3.5,
    minZ: 30.8,
    maxZ: 32.85,
  };

  private readonly bus = new Bus();

  private readonly backplane = new Board();

  private readonly clock = new SimClock(this.bus);

  private readonly msfsVersionPopup = new MsfsVersionPopupMonitor(this.bus, 'A380X');

  private readonly notificationManager: NotificationManager;

  private readonly hEventPublisher: PanelEventPublisher;

  private readonly simVarPublisher: ExtrasSimVarPublisher;

  private readonly msfsElectricsPublisher: MsfsElectricsPublisher;

  private readonly msfsFlightModelPublisher: MsfsFlightModelPublisher;

  private readonly msfsMiscPublisher: MsfsMiscPublisher;

  private readonly groundSupportPublisher: GroundSupportPublisher;

  private readonly gsxSimVarPublusher: GsxSimVarPublisher;

  private readonly pushbuttonCheck: PushbuttonCheck;

  private readonly versionCheck: VersionCheck;

  private readonly keyInterceptor: A380XKeyInterceptor;

  private readonly aircraftSync: AircraftSync;

  private readonly pilotSeatManager = new PilotSeatManager(ExtrasHost.flightDeckBounds);

  private readonly telexCheck = new TelexCheck();
  /**interaction point 19 is GPU connection and 4 GPUs in total */
  private readonly gpuManagement = new GPUManagement(this.bus, 19, 4);

  private readonly lightSync: LightSync = new LightSync(this.bus);

  private readonly baroUnitSelector = new BaroUnitSelector((isHpa) => {
    Vars.write('L:XMLVAR_Baro_Selector_HPA_1', VarUnit.Bool, isHpa);
    Vars.write('L:XMLVAR_Baro_Selector_HPA_2', VarUnit.Bool, isHpa);
  });

  private readonly gsxSync = new GsxSyncA380X(this.bus);

  /**
   * "mainmenu" = 0
   * "loading" = 1
   * "briefing" = 2
   * "ingame" = 3
   */
  private gameState = 0;

  constructor() {
    super();

    const aircraftProjectPrefix = process.env.AIRCRAFT_PROJECT_PREFIX;

    if (aircraftProjectPrefix === undefined) {
      throw new Error('[ExtrasHost] AIRCRAFT_PROJECT_PREFIX was undefined');
    }

    this.hEventPublisher = new PanelEventPublisher(this.bus);
    this.simVarPublisher = new ExtrasSimVarPublisher(this.bus);
    this.msfsElectricsPublisher = new MsfsElectricsPublisher(this.bus);
    this.msfsFlightModelPublisher = new MsfsFlightModelPublisher(this.bus);
    this.msfsMiscPublisher = new MsfsMiscPublisher(this.bus);
    this.groundSupportPublisher = new GroundSupportPublisher(this.bus);
    this.gsxSimVarPublusher = new GsxSimVarPublisher(this.bus);

    this.notificationManager = new NotificationManager(this.bus);

    this.pushbuttonCheck = new PushbuttonCheck(this.bus, this.notificationManager);
    this.keyInterceptor = new A380XKeyInterceptor(this.bus, this.notificationManager);
    this.versionCheck = new VersionCheck(aircraftProjectPrefix, this.bus);
    this.aircraftSync = new AircraftSync(aircraftProjectPrefix, this.bus);

    NavigationDatabaseService.activeDatabase = new NavigationDatabase(this.bus, NavigationDatabaseBackend.Msfs);

    this.backplane.attachFeeder('SimvarPublisher', this.simVarPublisher);
    this.backplane.attachFeeder('MsfsElectricsPublisher', this.msfsElectricsPublisher);
    this.backplane.attachFeeder('MsfsFlightModelPublisher', this.msfsFlightModelPublisher);
    this.backplane.attachFeeder('MsfsMiscPublisher', this.msfsMiscPublisher);
    this.backplane.attachFeeder('GroundSupportPublisher', this.groundSupportPublisher);
    this.backplane.attachFeeder('GsxSimVarPublisher', this.gsxSimVarPublusher);
    this.backplane.attachFeeder('PilotSeatPublisher', new PilotSeatPublisher(this.bus));
    this.backplane.attachDevice('PilotSeatManager', this.pilotSeatManager);
    this.backplane.attachDevice('GPUManagement', this.gpuManagement);
    this.backplane.attachDevice('Clock', this.clock);
    this.backplane.attachDevice('LightSync', this.lightSync);
    this.backplane.attachDevice('GsxSync', this.gsxSync);

    console.log('A380X_EXTRASHOST: Created');
  }

  get templateId(): string {
    return 'A380X_EXTRASHOST';
  }

  public getDeltaTime() {
    return this.frameDelta;
  }

  public onPanelEvent(args: string[]): void {
    this.hEventPublisher.dispatchPanelEvent(args[0]);
  }

  public onAttach(): void {
    super.onAttach();

    this.versionCheck.connectedCallback();
    this.pushbuttonCheck.connectedCallback();
    this.aircraftSync.connectedCallback();

    this.backplane.setup();

    this.baroUnitSelector.performSelection();
  }

  public readPanelConfig(): void {
    super.readPanelConfig();
    this.aircraftSync.parseXMLConfig(this.panelConfig);
  }

  public onFrame(): void {
    super.onFrame();

    if (this.gameState !== SessionPhase.flying) {
      const gs = this.sessionState();
      if (gs === SessionPhase.flying) {
        // Start the modules
        this.hEventPublisher.begin();
        this.versionCheck.startPublish();
        this.keyInterceptor.startPublish();
        this.aircraftSync.startPublish();
        this.telexCheck.showPopup();

        // Signal that the aircraft is ready via L:A32NX_IS_READY
        Vars.write('L:A32NX_IS_READY', 'number', 1);
        console.log('A380X_EXTRASHOST: Aircraft is ready');
      }
      this.gameState = gs;
    }

    this.backplane.tick();
  }
}

defineInstrument('extras-host', ExtrasHost);
