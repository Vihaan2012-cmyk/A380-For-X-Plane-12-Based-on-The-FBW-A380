import { Arinc429LocalVarConsumerSubject } from '@flybywiresim/fbw-sdk';
import { BusSignal, Bus, BoardDevice, Derived } from 'xplane-js/ui';
import { PrimFeBusBaseEvents, PrimFeBusEvents } from '@shared/publishers/PrimFePublisher';
import { PrimFctlBusEvents } from '@shared/publishers/PrimFctlPublisher';

const primFeSubjectsByKey = {
  prim_gamma_a: BusSignal.from(null, 0),
  prim_gamma_t: BusSignal.from(null, 0),
  prim_v_alpha_lim: BusSignal.from(null, 0),
  prim_v_alpha_prot: BusSignal.from(null, 0),
  prim_v_alpha_stall_warn: BusSignal.from(null, 0),
  prim_v_ls: BusSignal.from(null, 0),
  prim_v_stall: BusSignal.from(null, 0),
  prim_speed_trend: BusSignal.from(null, 0),
  prim_v_3: BusSignal.from(null, 0),
  prim_v_4: BusSignal.from(null, 0),
  prim_v_man: BusSignal.from(null, 0),
  prim_v_max: BusSignal.from(null, 0),
  prim_v_fe_next: BusSignal.from(null, 0),
} satisfies Record<keyof PrimFeBusBaseEvents, BusSignal<number>>;

export class PrimChoiceProvider implements BoardDevice {
  private readonly sub = this.bus.inbox<PrimFeBusEvents & PrimFctlBusEvents>();

  private readonly prim1FctlLawStatusWord = Arinc429LocalVarConsumerSubject.create(
    this.sub.topic('prim_fctl_law_status_word_1'),
  );

  private readonly prim2FctlLawStatusWord = Arinc429LocalVarConsumerSubject.create(
    this.sub.topic('prim_fctl_law_status_word_2'),
  );

  private readonly prim3FctlLawStatusWord = Arinc429LocalVarConsumerSubject.create(
    this.sub.topic('prim_fctl_law_status_word_3'),
  );

  private readonly selectedMasterPrim = Derived.from(
    ([prim1FctlLawWord, prim2FctlLawWord, prim3FctlLawWord]) => {
      if (prim1FctlLawWord.bitValueOr(21, false)) {
        return 1;
      } else if (prim2FctlLawWord.bitValueOr(21, false)) {
        return 2;
      } else if (prim3FctlLawWord.bitValueOr(21, false)) {
        return 3;
      } else {
        // No PRIM is selected as master PRIM (all PRIMs invalid). Default to PRIM 1
        return 1;
      }
    },
    this.prim1FctlLawStatusWord,
    this.prim2FctlLawStatusWord,
    this.prim3FctlLawStatusWord,
  );

  private readonly primFeSubjects = new Map(
    Object.entries(primFeSubjectsByKey) as [keyof PrimFeBusBaseEvents, BusSignal<number>][],
  );

  constructor(private readonly bus: Bus) {}

  /** @inheritdoc */
  public setup(): void {
    const publisher = this.bus.outbox<PrimFeBusBaseEvents>();

    this.selectedMasterPrim.watch((masterPrim) => {
      for (const [key, value] of this.primFeSubjects) {
        // The FE data over AFDX seems to update only at around 10Hz (see for example Vmax when moving)
        value.rebind(this.sub.topic(`${key}_${masterPrim}`).limit(10));
      }
    }, true);

    for (const [key, value] of this.primFeSubjects) {
      value.watch((word) => {
        publisher.post(key, word);
      }, true);
    }
  }

  /** @inheritdoc */
  public tick(): void {
    // noop
  }
}
