// @ts-strict-ignore
//  Copyright (c) 2023-2026 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { Arinc429Word, Arinc429SignStatusMatrix } from '@flybywiresim/fbw-sdk';
import {
  Bus,
  Inbox,
  Outbox,
  VarSpec,
  VarPublisher,
  VarUnit,
} from 'xplane-js/ui';
import { AtcMessageButtonBusMessages } from '../../../atc/src';
import { ClockDataBusTypes, FmgcDataBusTypes, FwcDataBusTypes, RmpDataBusTypes } from '../databus';

interface SimVars {
  msfsUtcYear: number;
  msfsUtcMonth: number;
  msfsUtcDayOfMonth: number;
  msfsUtcSeconds: number;
  msfsPresentPositionLatitude: number;
  msfsPresentPositionLongitude: number;
  msfsPresentAltitude: number;
  msfsPresentHeading: number;
  msfsPresentTrack: number;
  msfsComputedAirspeed: number;
  msfsPresentMach: number;
  msfsGroundSpeed: number;
  msfsVerticalSpeed: number;
  msfsAutopilotActive: boolean;
  msfsAutothrustMode: number;
  msfsAutothrustSelectedMach: number;
  msfsAutothrustSelectedKnots: number;
  msfsWindDirection: number;
  msfsWindSpeed: number;
  msfsStaticAirTemperature: number;
  msfsFlightPhase: number;
  msfsVhf3Powered: number;
  msfsTransponderCode: number;
  msfsCompanyMessageCount: number;
  msfsAtcMessageButtonActive: boolean;
  msfsAtcMessageButtonPressed: number;
}

export enum SimVarSources {
  utcYear = 'E:fbw/ZULU_YEAR',
  utcMonth = 'E:fbw/ZULU_MONTH_OF_YEAR',
  utcDayOfMonth = 'E:fbw/ZULU_DAY_OF_MONTH',
  utcSeconds = 'E:fbw/ZULU_TIME',
  presentPositionLatitude = 'L:A32NX_ADIRS_IR_1_LATITUDE',
  presentPositionLongitude = 'L:A32NX_ADIRS_IR_1_LONGITUDE',
  presentAltitude = 'L:A32NX_ADIRS_ADR_1_ALTITUDE',
  presentHeading = 'L:A32NX_ADIRS_IR_1_HEADING',
  presentTrack = 'L:A32NX_ADIRS_IR_1_TRACK',
  computedAirspeed = 'L:A32NX_ADIRS_ADR_1_COMPUTED_AIRSPEED',
  presentMach = 'L:A32NX_ADIRS_ADR_1_MACH',
  groundSpeed = 'L:A32NX_ADIRS_IR_1_GROUND_SPEED',
  verticalSpeed = 'L:A32NX_ADIRS_IR_1_VERTICAL_SPEED',
  autopilotActive = 'L:A32NX_AUTOPILOT_ACTIVE',
  autothrustMode = 'L:A32NX_AUTOTHRUST_MODE',
  autothrustSelectedMach = 'L:A32NX_MachPreselVal',
  autothrustSelectedKnots = 'L:A32NX_SpeedPreselVal',
  windDirection = 'L:A32NX_ADIRS_IR_1_WIND_DIRECTION',
  windSpeed = 'L:A32NX_ADIRS_IR_1_WIND_SPEED',
  staticAirTemperature = 'L:A32NX_ADIRS_ADR_1_STATIC_AIR_TEMPERATURE',
  flightPhase = 'L:A32NX_FMGC_FLIGHT_PHASE',
  vhf3Powered = 'L:A32NX_ELEC_DC_1_BUS_IS_POWERED',
  transponderCode = 'fbw/TRANSPONDER_CODE_1',
  companyMessageCount = 'L:A32NX_COMPANY_MSG_COUNT',
  atcMessageButtonActive = 'L:A32NX_DCDU_ATC_MSG_WAITING',
  atcMessageButtonPressed = 'L:A32NX_DCDU_ATC_MSG_ACK',
}

export class SimVarHandling extends VarPublisher<SimVars> {
  private subscriber: Inbox<SimVars> = null;

  private datalinkPublisher: Outbox<
    AtcMessageButtonBusMessages & ClockDataBusTypes & FmgcDataBusTypes & RmpDataBusTypes & FwcDataBusTypes
  > = null;

  private static simvars = new Map<keyof SimVars, VarSpec>([
    ['msfsUtcYear', { name: SimVarSources.utcYear, type: VarUnit.Number }],
    ['msfsUtcMonth', { name: SimVarSources.utcMonth, type: VarUnit.Number }],
    ['msfsUtcDayOfMonth', { name: SimVarSources.utcDayOfMonth, type: VarUnit.Number }],
    ['msfsUtcSeconds', { name: SimVarSources.utcSeconds, type: VarUnit.Number }],
    ['msfsPresentPositionLatitude', { name: SimVarSources.presentPositionLatitude, type: VarUnit.Number }],
    ['msfsPresentPositionLongitude', { name: SimVarSources.presentPositionLongitude, type: VarUnit.Number }],
    ['msfsPresentAltitude', { name: SimVarSources.presentAltitude, type: VarUnit.Number }],
    ['msfsPresentHeading', { name: SimVarSources.presentHeading, type: VarUnit.Number }],
    ['msfsPresentTrack', { name: SimVarSources.presentTrack, type: VarUnit.Number }],
    ['msfsComputedAirspeed', { name: SimVarSources.computedAirspeed, type: VarUnit.Number }],
    ['msfsPresentMach', { name: SimVarSources.presentMach, type: VarUnit.Number }],
    ['msfsGroundSpeed', { name: SimVarSources.groundSpeed, type: VarUnit.Number }],
    ['msfsAutopilotActive', { name: SimVarSources.autopilotActive, type: VarUnit.Number }],
    ['msfsAutothrustMode', { name: SimVarSources.autothrustMode, type: VarUnit.Number }],
    ['msfsAutothrustSelectedMach', { name: SimVarSources.autothrustSelectedMach, type: VarUnit.Number }],
    ['msfsAutothrustSelectedKnots', { name: SimVarSources.autothrustSelectedKnots, type: VarUnit.Knots }],
    ['msfsWindDirection', { name: SimVarSources.windDirection, type: VarUnit.Number }],
    ['msfsWindSpeed', { name: SimVarSources.windSpeed, type: VarUnit.Number }],
    ['msfsStaticAirTemperature', { name: SimVarSources.staticAirTemperature, type: VarUnit.Number }],
    ['msfsFlightPhase', { name: SimVarSources.flightPhase, type: VarUnit.Number }],
    ['msfsVhf3Powered', { name: SimVarSources.vhf3Powered, type: VarUnit.Number }],
    ['msfsTransponderCode', { name: SimVarSources.transponderCode, type: VarUnit.Number }],
    ['msfsCompanyMessageCount', { name: SimVarSources.companyMessageCount, type: VarUnit.Number }],
    ['msfsAtcMessageButtonActive', { name: SimVarSources.atcMessageButtonActive, type: VarUnit.Bool }],
    ['msfsAtcMessageButtonPressed', { name: SimVarSources.atcMessageButtonPressed, type: VarUnit.Number }],
  ]);

  public constructor(private readonly eventBus: Bus) {
    super(SimVarHandling.simvars, eventBus);
  }

  public initialize(): void {
    this.datalinkPublisher = this.eventBus.outbox<
      AtcMessageButtonBusMessages & ClockDataBusTypes & FmgcDataBusTypes & RmpDataBusTypes & FwcDataBusTypes
    >();
    this.subscriber = this.eventBus.inbox<SimVars>();

    this.subscriber
      .topic('msfsUtcYear')
      .listen((year: number) => this.datalinkPublisher.post('utcYear', year, false, false));
    this.subscriber
      .topic('msfsUtcMonth')
      .listen((month: number) => this.datalinkPublisher.post('utcMonth', month, false, false));
    this.subscriber
      .topic('msfsUtcDayOfMonth')
      .listen((day: number) => this.datalinkPublisher.post('utcDayOfMonth', day, false, false));
    this.subscriber.topic('msfsUtcSeconds').listen((seconds: number) => {
      const hours = Math.floor(seconds / 3600);
      const minutes = Math.floor(seconds / 60) % 60;
      const secondsOfMinute = Math.floor(seconds) - hours * 3600 + minutes * 60;

      this.datalinkPublisher.post('utcHour', hours, false, false);
      this.datalinkPublisher.post('utcMinute', minutes, false, false);
      this.datalinkPublisher.post('utcSecond', secondsOfMinute, false, false);
      this.datalinkPublisher.post('utcSecondsOfDay', seconds, false, false);
    });
    this.subscriber.topic('msfsPresentPositionLatitude').listen((latitude: number) => {
      this.datalinkPublisher.post('presentPositionLatitude', new Arinc429Word(latitude), false, false);
    });
    this.subscriber.topic('msfsPresentPositionLongitude').listen((longitude: number) => {
      this.datalinkPublisher.post('presentPositionLongitude', new Arinc429Word(longitude), false, false);
    });
    this.subscriber.topic('msfsPresentAltitude').listen((altitude: number) => {
      this.datalinkPublisher.post('presentAltitude', new Arinc429Word(altitude), false, false);
    });
    this.subscriber.topic('msfsPresentHeading').listen((heading: number) => {
      this.datalinkPublisher.post('presentHeading', new Arinc429Word(heading), false, false);
    });
    this.subscriber.topic('msfsPresentTrack').listen((track: number) => {
      this.datalinkPublisher.post('presentTrack', new Arinc429Word(track), false, false);
    });
    this.subscriber.topic('msfsComputedAirspeed').listen((cas: number) => {
      this.datalinkPublisher.post('computedAirspeed', new Arinc429Word(cas), false, false);
    });
    this.subscriber.topic('msfsPresentMach').listen((mach: number) => {
      this.datalinkPublisher.post('presentMach', new Arinc429Word(mach), false, false);
    });
    this.subscriber.topic('msfsGroundSpeed').listen((groundSpeed: number) => {
      this.datalinkPublisher.post('groundSpeed', new Arinc429Word(groundSpeed), false, false);
    });
    this.subscriber.topic('msfsVerticalSpeed').listen((verticalSpeed: number) => {
      this.datalinkPublisher.post('verticalSpeed', new Arinc429Word(verticalSpeed), false, false);
    });
    this.subscriber.topic('msfsAutopilotActive').listen((active: boolean) => {
      this.datalinkPublisher.post('autopilotActive', new Arinc429Word(active === true ? 1 : 0), false, false);
    });
    this.subscriber.topic('msfsAutothrustMode').listen((mode: number) => {
      this.datalinkPublisher.post('autothrustMode', new Arinc429Word(mode), false, false);
    });
    this.subscriber.topic('msfsAutothrustSelectedMach').listen((mach: number) => {
      const word = new Arinc429Word(0.0);
      word.ssm = Arinc429SignStatusMatrix.NormalOperation;
      word.value = mach;

      this.datalinkPublisher.post('autothrustSelectedMach', word, false, false);
    });
    this.subscriber.topic('msfsAutothrustSelectedKnots').listen((knots: number) => {
      const word = new Arinc429Word(0.0);
      word.ssm = Arinc429SignStatusMatrix.NormalOperation;
      word.value = knots;

      this.datalinkPublisher.post('autothrustSelectedKnots', word, false, false);
    });
    this.subscriber.topic('msfsWindDirection').listen((direction: number) => {
      this.datalinkPublisher.post('windDirection', new Arinc429Word(direction), false, false);
    });
    this.subscriber.topic('msfsWindSpeed').listen((speed: number) => {
      this.datalinkPublisher.post('windSpeed', new Arinc429Word(speed), false, false);
    });
    this.subscriber.topic('msfsStaticAirTemperature').listen((sat: number) => {
      this.datalinkPublisher.post('staticAirTemperature', new Arinc429Word(sat), false, false);
    });
    this.subscriber.topic('msfsFlightPhase').listen((phase: number) => {
      this.datalinkPublisher.post('flightPhase', new Arinc429Word(phase), false, false);
    });
    this.subscriber
      .topic('msfsVhf3Powered')
      .listen((powered: number) => this.datalinkPublisher.post('vhf3Powered', powered !== 0, false, false));
    this.subscriber
      .topic('msfsTransponderCode')
      .listen((code: number) => this.datalinkPublisher.post('transponderCode', code, false, false));
    this.subscriber
      .topic('msfsCompanyMessageCount')
      .listen((count: number) => this.datalinkPublisher.post('companyMessageCount', count, false, false));
    this.subscriber
      .topic('msfsAtcMessageButtonActive')
      .listen((active: boolean) => this.datalinkPublisher.post('atcMessageButtonActive', active, false, false));
    this.subscriber
      .topic('msfsAtcMessageButtonPressed')
      .listen((pressed: number) => this.datalinkPublisher.post('atcMessageButtonPressed', pressed !== 0, false, false));
  }

  public begin(): void {
    super.begin();
  }

  public update(): void {
    super.tick();
  }
}
