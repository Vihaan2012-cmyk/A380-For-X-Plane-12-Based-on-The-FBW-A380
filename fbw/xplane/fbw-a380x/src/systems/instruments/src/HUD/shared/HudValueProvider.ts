import { BoardDevice, ClockTopics, BusSignal, Derived } from 'xplane-js/ui';
import { getDisplayIndex } from '../HUD';
import { HUDSimvars } from './HUDSimvarPublisher';
import { HudMode, PitchscaleMode, HudElems } from '../HUDUtils';
import { FmgcFlightPhase } from '@shared/flightphase';
import {
  Arinc429ConsumerSubject,
  ArincEventBus,
  Arinc429RegisterSubject,
  Arinc429LocalVarConsumerSubject,
} from '@flybywiresim/fbw-sdk';
import { Arinc429Values } from './ArincValueProvider';
import { PrimFgBusBaseEvents } from '@shared/publishers/PrimFgPublisher';
import { Vars } from 'xplane-js/data';

export class HudValueProvider implements BoardDevice {
  private flightPhase = 0;
  private declutterMode = 0;
  private crosswindMode = false;

  private elems: HudElems = {
    spdTape: '',
    xWindSpdTape: '',
    altTape: '',
    xWindAltTape: '',
    attitudeIndicator: '',
    FMA: '',
    headingTrk: '',
    gndAcftRef: '',
    inAirAcftRef: '',
    flightPathDirector: '',
    flightPathVector: '',
    VSI: '',
    ra: '',
    IlsGS: '',
    IlsLoc: '',
    IlsHorizonTrk: '',
    syntheticRunwway: '',
    windIndicator: '',
    QFE: '',
    metricAlt: true,
    pitchScaleMode: -1,
    hudFlightPhaseMode: 0,
    cWndMode: true,
    decMode: -1,
    spdChevrons: 'block',
  };
  private logCase = '';

  private readonly sub = this.bus.getArincSubscriber<Arinc429Values & HUDSimvars & ClockTopics & PrimFgBusBaseEvents>();

  private readonly lmgc = BusSignal.from(this.sub.on('leftMainGearCompressed'), true);
  private readonly rmgc = BusSignal.from(this.sub.on('rightMainGearCompressed'), true);
  private readonly groundSpeed = Arinc429RegisterSubject.createEmpty();
  private readonly ra = Arinc429ConsumerSubject.create(this.sub.on('chosenRa'));

  private readonly prim_fg_ats_fma_discrete_word = Arinc429LocalVarConsumerSubject.create(
    this.sub.on('prim_fg_ats_fma_discrete_word'),
  );

  private readonly isToga = this.prim_fg_ats_fma_discrete_word.derive((word) => word.bitValueOr(11, false));
  private readonly isFlex = this.prim_fg_ats_fma_discrete_word.derive((word) => word.bitValueOr(13, false));

  private readonly hudMode = Derived.from(
    ([lmgc, rmgc, groundSpeed, ra, isFlex, isToga]) => {
      if ((lmgc || rmgc) && groundSpeed.value < 40 && !(isFlex || isToga)) {
        return HudMode.TAXI;
      } else if ((lmgc || rmgc || ra.value < 50) && (isFlex || isToga)) {
        return HudMode.TAKEOFF;
      } else if ((lmgc || rmgc) && groundSpeed.value >= 40 && !(isFlex || isToga)) {
        return HudMode.ROLLOUT_OR_RTO;
      } else {
        return HudMode.NORMAL;
      }
    },
    this.lmgc,
    this.rmgc,
    this.groundSpeed,
    this.ra,
    this.isToga,
    this.isFlex,
  );

  constructor(private readonly bus: ArincEventBus) {}

  /** @inheritdoc */
  public setup(): void {
    const publisher = this.bus.outbox<HudElems>();

    const isCaptainSide = getDisplayIndex() === 1;

    this.sub.on('groundSpeed').listen((value) => {
      this.groundSpeed.setWord(value);
    });

    this.sub
      .on('realTime')
      //.atFrequency(1) ///////////////
      .listen((_t) => {
        Vars.write('L:A380X_HUDMODE', 'number', this.hudMode.get());
        if (this.hudMode.get() === HudMode.TAXI) {
          // hudmode  Taxi     flightpahse  notApp  xwind  ANY  dec  0
          if (this.declutterMode === 0) {
            this.elems.spdTape = 'block';
            this.elems.xWindSpdTape = 'none';
            this.elems.altTape = 'block';
            this.elems.xWindAltTape = 'none';
            this.elems.attitudeIndicator = 'none';
            this.elems.FMA = 'block';
            this.elems.headingTrk = 'none';
            this.elems.gndAcftRef = 'block';
            this.elems.inAirAcftRef = 'none';
            this.elems.flightPathDirector = 'none';
            this.elems.flightPathVector = 'block';
            this.elems.VSI = 'none';
            this.elems.ra = 'none';
            this.elems.IlsGS = 'none';
            this.elems.IlsLoc = 'block';
            this.elems.IlsHorizonTrk = 'block';
            this.elems.syntheticRunwway = 'none';
            this.elems.windIndicator = 'none';
            this.elems.QFE = 'block';
            this.elems.metricAlt = true;
            this.elems.pitchScaleMode = PitchscaleMode.FULL;
            this.elems.hudFlightPhaseMode = this.hudMode.get();
            this.elems.cWndMode = false;
            this.elems.decMode = 0;
            this.elems.spdChevrons = 'block';
            this.logCase = ' A ';
          } else {
            // hudmode  Taxi     flightpahse  notApp  xwind  ANY  dec  1or2
            this.elems.spdTape = 'none';
            this.elems.xWindSpdTape = 'none';
            this.elems.altTape = 'none';
            this.elems.xWindAltTape = 'none';
            this.elems.attitudeIndicator = 'none';
            this.elems.FMA = 'none';
            this.elems.headingTrk = 'none';
            this.elems.gndAcftRef = 'block';
            this.elems.inAirAcftRef = 'none';
            this.elems.flightPathDirector = 'none';
            this.elems.flightPathVector = 'block';
            this.elems.VSI = 'none';
            this.elems.ra = 'none';
            this.elems.IlsGS = 'none';
            this.elems.IlsLoc = 'block';
            this.elems.IlsHorizonTrk = 'block';
            this.elems.syntheticRunwway = 'none';
            this.elems.windIndicator = 'none';
            this.elems.QFE = 'none';
            this.elems.metricAlt = false;
            this.elems.pitchScaleMode = PitchscaleMode.OFF;
            this.elems.hudFlightPhaseMode = this.hudMode.get();
            this.elems.cWndMode = false;
            this.elems.decMode = 2;
            this.elems.spdChevrons = 'block';
            this.logCase = ' B ';
          }
        } else if (this.hudMode.get() === HudMode.TAKEOFF) {
          // hudmode  takeoff  flightpahse  ANY  xwind  ANY  dec  ANY
          this.elems.spdTape = 'block';
          this.elems.xWindSpdTape = 'none';
          this.elems.altTape = 'block';
          this.elems.xWindAltTape = 'none';
          this.elems.attitudeIndicator = 'none';
          this.elems.FMA = 'block';
          this.elems.headingTrk = 'none';
          this.elems.gndAcftRef = 'block';
          this.elems.inAirAcftRef = 'none';
          this.elems.flightPathDirector = 'none';
          this.elems.flightPathVector = 'block';
          this.elems.VSI = 'block';
          this.elems.ra = 'none';
          this.elems.IlsGS = 'none';
          this.elems.IlsLoc = 'block';
          this.elems.IlsHorizonTrk = 'block';
          this.elems.syntheticRunwway = 'none';
          this.elems.windIndicator = 'none';
          this.elems.QFE = 'block';
          this.elems.metricAlt = true;
          this.elems.pitchScaleMode = PitchscaleMode.FULL;
          this.elems.hudFlightPhaseMode = this.hudMode.get();
          this.elems.cWndMode = false;
          this.elems.decMode = 0;
          this.elems.spdChevrons = 'block';
          this.logCase = ' C ';
        } else if (this.hudMode.get() === HudMode.ROLLOUT_OR_RTO) {
          // hudmode    RTO    flightpahse  ANY  xwind  ANY  dec  ANY
          this.elems.spdTape = 'block';
          this.elems.xWindSpdTape = 'none';
          this.elems.altTape = 'none';
          this.elems.xWindAltTape = 'none';
          this.elems.attitudeIndicator = 'none';
          this.elems.FMA = 'block';
          this.elems.headingTrk = 'none';
          this.elems.gndAcftRef = 'none';
          this.elems.inAirAcftRef = 'none';
          this.elems.flightPathDirector = 'none';
          this.elems.flightPathVector = 'block';
          this.elems.VSI = 'none';
          this.elems.ra = 'none';
          this.elems.IlsGS = 'none';
          this.elems.IlsLoc = 'block';
          this.elems.IlsHorizonTrk = 'block';
          this.elems.syntheticRunwway = 'none';
          this.elems.windIndicator = 'none';
          this.elems.QFE = 'none';
          this.elems.metricAlt = false;
          this.elems.pitchScaleMode = PitchscaleMode.OFF;
          this.elems.hudFlightPhaseMode = this.hudMode.get();
          this.elems.cWndMode = false;
          this.elems.decMode = 0;
          this.elems.spdChevrons = 'none';
          this.logCase = ' D ';
        } else {
          //HudMode Normal
          if (this.flightPhase === FmgcFlightPhase.Approach) {
            if (this.declutterMode === 0) {
              if (this.crosswindMode === false) {
                // flightPhase App dec 0 xwind 0
                this.elems.spdTape = 'block';
                this.elems.xWindSpdTape = 'none';
                this.elems.altTape = 'block';
                this.elems.xWindAltTape = 'none';
                this.elems.attitudeIndicator = 'block';
                this.elems.FMA = 'block';
                this.elems.headingTrk = 'block';
                this.elems.gndAcftRef = 'none';
                this.elems.inAirAcftRef = 'block';
                this.elems.flightPathDirector = 'block';
                this.elems.flightPathVector = 'block';
                this.elems.VSI = 'block';
                this.elems.ra = 'block';
                this.elems.IlsGS = 'block';
                this.elems.IlsLoc = 'block';
                this.elems.IlsHorizonTrk = 'block';
                this.elems.syntheticRunwway = 'block';
                this.elems.windIndicator = 'block';
                this.elems.QFE = 'block';
                this.elems.metricAlt = true;
                this.elems.pitchScaleMode = PitchscaleMode.FULL;
                this.elems.hudFlightPhaseMode = this.hudMode.get();
                this.elems.cWndMode = false;
                this.elems.decMode = 0;
                this.elems.spdChevrons = 'block';
                this.logCase = ' E ';
              } else {
                // flightPhase App dec 0 xwind 1
                this.elems.spdTape = 'none';
                this.elems.xWindSpdTape = 'block';
                this.elems.altTape = 'none';
                this.elems.xWindAltTape = 'block';
                this.elems.attitudeIndicator = 'block';
                this.elems.FMA = 'block';
                this.elems.headingTrk = 'block';
                this.elems.gndAcftRef = 'none';
                this.elems.inAirAcftRef = 'block';
                this.elems.flightPathDirector = 'block';
                this.elems.flightPathVector = 'block';
                this.elems.VSI = 'block';
                this.elems.ra = 'block';
                this.elems.IlsGS = 'block';
                this.elems.IlsLoc = 'block';
                this.elems.IlsHorizonTrk = 'block';
                this.elems.syntheticRunwway = 'block';
                this.elems.windIndicator = 'block';
                this.elems.QFE = 'block';
                this.elems.metricAlt = true;
                this.elems.pitchScaleMode = PitchscaleMode.FULL;
                this.elems.hudFlightPhaseMode = this.hudMode.get();
                this.elems.cWndMode = true;
                this.elems.decMode = 0;
                this.elems.spdChevrons = 'block';
                this.logCase = ' F ';
              }
            } else if (this.declutterMode === 1) {
              if (this.crosswindMode === false) {
                // flightPhase App dec 1 xwind 0
                this.elems.spdTape = 'block';
                this.elems.xWindSpdTape = 'none';
                this.elems.altTape = 'block';
                this.elems.xWindAltTape = 'none';
                this.elems.attitudeIndicator = 'block';
                this.elems.FMA = 'block';
                this.elems.headingTrk = 'block';
                this.elems.gndAcftRef = 'none';
                this.elems.inAirAcftRef = 'block';
                this.elems.flightPathDirector = 'block';
                this.elems.flightPathVector = 'block';
                this.elems.VSI = 'block';
                this.elems.ra = 'block';
                this.elems.IlsGS = 'block';
                this.elems.IlsLoc = 'block';
                this.elems.IlsHorizonTrk = 'block';
                this.elems.syntheticRunwway = 'none';
                this.elems.windIndicator = 'block';
                this.elems.QFE = 'block';
                this.elems.metricAlt = true;
                this.elems.pitchScaleMode = PitchscaleMode.FULL;
                this.elems.hudFlightPhaseMode = this.hudMode.get();
                this.elems.cWndMode = false;
                this.elems.decMode = 1;
                this.elems.spdChevrons = 'block';
                this.logCase = ' G ';
              } else {
                // flightPhase App dec 1 xwind 1
                this.elems.spdTape = 'none';
                this.elems.xWindSpdTape = 'block';
                this.elems.altTape = 'none';
                this.elems.xWindAltTape = 'block';
                this.elems.attitudeIndicator = 'block';
                this.elems.FMA = 'block';
                this.elems.headingTrk = 'block';
                this.elems.gndAcftRef = 'none';
                this.elems.inAirAcftRef = 'block';
                this.elems.flightPathDirector = 'block';
                this.elems.flightPathVector = 'block';
                this.elems.VSI = 'block';
                this.elems.ra = 'block';
                this.elems.IlsGS = 'block';
                this.elems.IlsLoc = 'block';
                this.elems.IlsHorizonTrk = 'block';
                this.elems.syntheticRunwway = 'none';
                this.elems.windIndicator = 'block';
                this.elems.QFE = 'block';
                this.elems.metricAlt = true;
                this.elems.pitchScaleMode = PitchscaleMode.FULL;
                this.elems.hudFlightPhaseMode = this.hudMode.get();
                this.elems.cWndMode = true;
                this.elems.decMode = 1;
                this.elems.spdChevrons = 'block';
                this.logCase = ' H ';
              }
            } else {
              // flightPhase App dec 2 xwind 0
              // flightPhase App dec 2 xwind 1    no xwind mode in dec 2 xwind tapes are forced
              this.elems.spdTape = 'none';
              this.elems.xWindSpdTape = 'block';
              this.elems.altTape = 'none';
              this.elems.xWindAltTape = 'block';
              this.elems.attitudeIndicator = 'none';
              this.elems.FMA = 'block';
              this.elems.headingTrk = 'none';
              this.elems.gndAcftRef = 'none';
              this.elems.inAirAcftRef = 'none';
              this.elems.flightPathDirector = 'block';
              this.elems.flightPathVector = 'block';
              this.elems.VSI = 'none';
              this.elems.ra = 'block';
              this.elems.IlsGS = 'none';
              this.elems.IlsLoc = 'none';
              this.elems.IlsHorizonTrk = 'none';
              this.elems.syntheticRunwway = 'none';
              this.elems.windIndicator = 'none';
              this.elems.QFE = 'none';
              this.elems.metricAlt = false;
              this.elems.pitchScaleMode = PitchscaleMode.FIVEDEG;
              this.elems.hudFlightPhaseMode = this.hudMode.get();
              this.elems.cWndMode = true;
              this.elems.decMode = 2;
              this.elems.spdChevrons = 'block';
              this.logCase = ' I ';
            }
          } else {
            if (this.declutterMode === 0) {
              if (this.crosswindMode === false) {
                // flightPhase NOTApp or Des dec 0 xwind 0
                this.elems.spdTape = 'block';
                this.elems.xWindSpdTape = 'none';
                this.elems.altTape = 'block';
                this.elems.xWindAltTape = 'none';
                this.elems.attitudeIndicator = 'block';
                this.elems.FMA = 'block';
                this.elems.headingTrk = 'block';
                this.elems.gndAcftRef = 'none';
                this.elems.inAirAcftRef = 'block';
                this.elems.flightPathDirector = 'block';
                this.elems.flightPathVector = 'block';
                this.elems.VSI = 'block';
                this.elems.ra = 'block';
                this.elems.IlsGS = 'block';
                this.elems.IlsLoc = 'block';
                this.elems.IlsHorizonTrk = 'block';
                this.flightPhase === FmgcFlightPhase.Descent
                  ? (this.elems.syntheticRunwway = 'block')
                  : (this.elems.syntheticRunwway = 'none');
                this.elems.windIndicator = 'block';
                this.elems.QFE = 'block';
                this.elems.metricAlt = true;
                this.elems.pitchScaleMode = PitchscaleMode.FULL;
                this.elems.hudFlightPhaseMode = this.hudMode.get();
                this.elems.cWndMode = false;
                this.elems.decMode = 0;
                this.elems.spdChevrons = 'block';
                this.logCase = ' J ';
              } else {
                // flightPhase NOTApp or Des dec 0 xwind 1
                this.elems.spdTape = 'none';
                this.elems.xWindSpdTape = 'block';
                this.elems.altTape = 'none';
                this.elems.xWindAltTape = 'block';
                this.elems.attitudeIndicator = 'block';
                this.elems.FMA = 'block';
                this.elems.headingTrk = 'block';
                this.elems.gndAcftRef = 'none';
                this.elems.inAirAcftRef = 'block';
                this.elems.flightPathDirector = 'block';
                this.elems.flightPathVector = 'block';
                this.elems.VSI = 'block';
                this.elems.ra = 'block';
                this.elems.IlsGS = 'block';
                this.elems.IlsLoc = 'block';
                this.elems.IlsHorizonTrk = 'block';
                this.flightPhase === FmgcFlightPhase.Descent
                  ? (this.elems.syntheticRunwway = 'block')
                  : (this.elems.syntheticRunwway = 'none');
                this.elems.windIndicator = 'block';
                this.elems.QFE = 'block';
                this.elems.metricAlt = true;
                this.elems.pitchScaleMode = PitchscaleMode.FULL;
                this.elems.hudFlightPhaseMode = this.hudMode.get();
                this.elems.cWndMode = true;
                this.elems.decMode = 0;
                this.elems.spdChevrons = 'block';
                this.logCase = ' K ';
              }
            } else if (!(this.declutterMode === 0)) {
              if (this.crosswindMode === false) {
                // flightPhase NOTApp or Des dec !0 xwind 0
                this.elems.spdTape = 'block';
                this.elems.xWindSpdTape = 'none';
                this.elems.altTape = 'block';
                this.elems.xWindAltTape = 'none';
                this.elems.attitudeIndicator = 'none';
                this.elems.FMA = 'block';
                this.elems.headingTrk = 'block';
                this.elems.gndAcftRef = 'none';
                this.elems.inAirAcftRef = 'none';
                this.elems.flightPathDirector = 'block';
                this.elems.flightPathVector = 'block';
                this.elems.VSI = 'none';
                this.elems.ra = 'block';
                this.elems.IlsGS = 'none';
                this.elems.IlsLoc = 'none';
                this.elems.IlsHorizonTrk = 'none';
                this.elems.syntheticRunwway = 'none';
                this.elems.windIndicator = 'none';
                this.elems.QFE = 'none';
                this.elems.metricAlt = false;
                this.elems.pitchScaleMode = PitchscaleMode.FIVEDEG;
                this.elems.hudFlightPhaseMode = this.hudMode.get();
                this.elems.cWndMode = false;
                this.elems.decMode = 2;
                this.elems.spdChevrons = 'block';
                this.logCase = ' L ';
              } else {
                // flightPhase NOTApp dec !0 xwind 1
                this.elems.spdTape = 'none';
                this.elems.xWindSpdTape = 'block';
                this.elems.altTape = 'none';
                this.elems.xWindAltTape = 'block';
                this.elems.attitudeIndicator = 'none';
                this.elems.FMA = 'block';
                this.elems.headingTrk = 'block';
                this.elems.gndAcftRef = 'none';
                this.elems.inAirAcftRef = 'none';
                this.elems.flightPathDirector = 'block';
                this.elems.flightPathVector = 'block';
                this.elems.VSI = 'none';
                this.elems.ra = 'block';
                this.elems.IlsGS = 'none';
                this.elems.IlsLoc = 'none';
                this.elems.IlsHorizonTrk = 'none';
                this.elems.syntheticRunwway = 'none';
                this.elems.windIndicator = 'none';
                this.elems.QFE = 'none';
                this.elems.metricAlt = false;
                this.elems.pitchScaleMode = PitchscaleMode.FIVEDEG;
                this.elems.hudFlightPhaseMode = this.hudMode.get();
                this.elems.cWndMode = true;
                this.elems.decMode = 2;
                this.elems.spdChevrons = 'block';
                this.logCase = ' M ';
              }
            }
          }
        }

        //console.log(this.logCase);
        publisher.post('cWndMode', this.elems.cWndMode, false, false);
        publisher.post('decMode', this.elems.decMode, false, false);
        publisher.post('hudFlightPhaseMode', this.elems.hudFlightPhaseMode, false, false);

        publisher.post('spdTape', this.elems.spdTape, false, false);
        publisher.post('xWindSpdTape', this.elems.xWindSpdTape, false, false);
        publisher.post('altTape', this.elems.altTape, false, false);
        publisher.post('xWindAltTape', this.elems.xWindAltTape, false, false);
        publisher.post('attitudeIndicator', this.elems.attitudeIndicator, false, false);
        publisher.post('FMA', this.elems.FMA, false, false);
        publisher.post('headingTrk', this.elems.headingTrk, false, false);
        publisher.post('gndAcftRef', this.elems.gndAcftRef, false, false);
        publisher.post('inAirAcftRef', this.elems.inAirAcftRef, false, false);
        publisher.post('flightPathDirector', this.elems.flightPathDirector, false, false);
        publisher.post('flightPathVector', this.elems.flightPathVector, false, false);
        publisher.post('VSI', this.elems.VSI, false, false);
        publisher.post('ra', this.elems.ra, false, false);
        publisher.post('IlsGS', this.elems.IlsGS, false, false);
        publisher.post('IlsLoc', this.elems.IlsLoc, false, false);
        publisher.post('IlsHorizonTrk', this.elems.IlsHorizonTrk, false, false);
        publisher.post('syntheticRunwway', this.elems.syntheticRunwway, false, false);
        publisher.post('windIndicator', this.elems.windIndicator, false, false);
        publisher.post('QFE', this.elems.QFE, false, false);
        publisher.post('metricAlt', this.elems.metricAlt, false, false);
        publisher.post('pitchScaleMode', this.elems.pitchScaleMode, false, false);
        publisher.post('spdChevrons', this.elems.spdChevrons, false, false);
      });

    this.sub.on('fmgcFlightPhase').listen((fp) => {
      this.flightPhase = fp;
      //this.setCrossWindMode(this.crosswindMode);
    });

    this.sub.on(isCaptainSide ? 'crosswindModeL' : 'crosswindModeR').listen((value) => {
      this.crosswindMode = value;
      //this.setCrossWindMode(value);
    });

    this.sub.on(isCaptainSide ? 'declutterModeL' : 'declutterModeR').listen((value) => {
      let side = '';
      getDisplayIndex() === 1 ? (side = 'L:A380X_HUD_L_DECLUTTER_MODE') : (side = 'L:A380X_HUD_R_DECLUTTER_MODE');

      if (this.flightPhase != FmgcFlightPhase.Approach) {
        if (!(value === 1)) {
          this.declutterMode = value;
        } else {
          Vars.write(side, 'number', 2);
          this.declutterMode = 2;
        }
      } else {
        this.declutterMode = value;
      }

      this.setCrossWindMode(this.crosswindMode);
    });
  }

  private setCrossWindMode(value: boolean) {
    if (this.hudMode.get() === HudMode.NORMAL) {
      if (this.flightPhase === FmgcFlightPhase.Approach) {
        if (this.declutterMode === 2) {
          this.crosswindMode = true;
        } else {
          this.crosswindMode = value;
        }
      } else {
        this.crosswindMode = value;
      }
    } else {
      this.crosswindMode = false;
    }
  }

  /** @inheritdoc */
  public tick(): void {
    // noop
  }
}
