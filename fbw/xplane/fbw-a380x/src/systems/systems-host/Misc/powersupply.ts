// @ts-strict-ignore
import {
  BoardFeeder,
  Bus,
  Inbox,
  Outbox,
  VarSpec,
  VarPublisher,
  VarUnit,
} from 'xplane-js/ui';

// FIXME no need for the extra indirection of the second publisher.
// Just set types to SimVarValueType.Bool and the SimVarPublisher takes care of types

interface PowerSupplySimvars {
  msfsAcBus1: number;
  msfsAcBus2: number;
  msfsAcBus3: number;
  msfsAcBus4: number;
  msfsAcBusEss: number;
  msfsDcBus1: number;
  msfsDcBus2: number;
  msfsDcBusEss: number;
  msfsDcBusEssInFlight: number;
}

enum PowerSupplySimvarSources {
  acBus1 = 'L:A32NX_ELEC_AC_1_BUS_IS_POWERED',
  acBus2 = 'L:A32NX_ELEC_AC_2_BUS_IS_POWERED',
  acBus3 = 'L:A32NX_ELEC_AC_3_BUS_IS_POWERED',
  acBus4 = 'L:A32NX_ELEC_AC_4_BUS_IS_POWERED',
  acBusEss = 'L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED',
  dcBus1 = 'L:A32NX_ELEC_DC_1_BUS_IS_POWERED',
  dcBus2 = 'L:A32NX_ELEC_DC_2_BUS_IS_POWERED',
  dcBusEss = 'L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED',
  dcBusEssInFlight = 'L:A32NX_ELEC_108PH_BUS_IS_POWERED',
}

export class PowerSupplySimvarPublisher extends VarPublisher<PowerSupplySimvars> {
  private static simvars = new Map<keyof PowerSupplySimvars, VarSpec>([
    ['msfsAcBus1', { name: PowerSupplySimvarSources.acBus1, type: VarUnit.Number }],
    ['msfsAcBus2', { name: PowerSupplySimvarSources.acBus2, type: VarUnit.Number }],
    ['msfsAcBus3', { name: PowerSupplySimvarSources.acBus3, type: VarUnit.Number }],
    ['msfsAcBus4', { name: PowerSupplySimvarSources.acBus4, type: VarUnit.Number }],
    ['msfsAcBusEss', { name: PowerSupplySimvarSources.acBusEss, type: VarUnit.Number }],
    ['msfsDcBus1', { name: PowerSupplySimvarSources.dcBus1, type: VarUnit.Number }],
    ['msfsDcBus2', { name: PowerSupplySimvarSources.dcBus2, type: VarUnit.Number }],
    ['msfsDcBusEss', { name: PowerSupplySimvarSources.dcBusEss, type: VarUnit.Number }],
    ['msfsDcBusEssInFlight', { name: PowerSupplySimvarSources.dcBusEssInFlight, type: VarUnit.Number }],
  ]);

  public constructor(bus: Bus) {
    super(PowerSupplySimvarPublisher.simvars, bus);
  }
}

export interface PowerSupplyBusTypes {
  acBus1: boolean;
  acBus2: boolean;
  acBus3: boolean;
  acBus4: boolean;
  acBusEss: boolean;
  dcBus1: boolean;
  dcBus2: boolean;
  dcBusEss: boolean;
  dcBusEssInFlight: boolean;
}

export class PowerSupplyBusses implements BoardFeeder {
  private simVarPublisher: PowerSupplySimvarPublisher = null;

  private subscriber: Inbox<PowerSupplySimvars> = null;

  private publisher: Outbox<PowerSupplyBusTypes> = null;

  constructor(private readonly bus: Bus) {
    this.simVarPublisher = new PowerSupplySimvarPublisher(this.bus);
  }

  private initialize(): void {
    this.publisher = this.bus.outbox<PowerSupplyBusTypes>();
    this.subscriber = this.bus.inbox<PowerSupplySimvars>();

    this.subscriber
      .topic('msfsAcBus1')
      .distinct()
      .listen((powered: number) => this.publisher.post('acBus1', powered !== 0, false, false));
    this.subscriber
      .topic('msfsAcBus2')
      .distinct()
      .listen((powered: number) => this.publisher.post('acBus2', powered !== 0, false, false));
    this.subscriber
      .topic('msfsAcBus3')
      .distinct()
      .listen((powered: number) => this.publisher.post('acBus3', powered !== 0, false, false));
    this.subscriber
      .topic('msfsAcBus4')
      .distinct()
      .listen((powered: number) => this.publisher.post('acBus4', powered !== 0, false, false));
    this.subscriber
      .topic('msfsAcBusEss')
      .distinct()
      .listen((powered: number) => this.publisher.post('acBusEss', powered !== 0, false, false));
    this.subscriber
      .topic('msfsDcBus1')
      .distinct()
      .listen((powered: number) => this.publisher.post('dcBus1', powered !== 0, false, false));
    this.subscriber
      .topic('msfsDcBus2')
      .distinct()
      .listen((powered: number) => this.publisher.post('dcBus2', powered !== 0, false, false));
    this.subscriber
      .topic('msfsDcBusEss')
      .distinct()
      .listen((powered: number) => this.publisher.post('dcBusEss', powered !== 0, false, false));
    this.subscriber
      .topic('msfsDcBusEssInFlight')
      .distinct()
      .listen((powered: number) => this.publisher.post('dcBusEssInFlight', powered !== 0, false, false));
  }

  public begin(): void {
    this.initialize();
    this.simVarPublisher.begin();
  }

  public tick(): void {
    this.simVarPublisher.tick();
  }
}
