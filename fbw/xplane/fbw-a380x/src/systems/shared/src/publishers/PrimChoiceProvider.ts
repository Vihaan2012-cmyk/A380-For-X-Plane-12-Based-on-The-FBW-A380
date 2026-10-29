// Copyright (c) 2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0
import { Arinc429LocalVarConsumerSubject } from '@flybywiresim/fbw-sdk';
import { BusSignal, Bus, Derived } from 'xplane-js/ui';
import { PrimFeBusBaseEvents, PrimFeBusEvents } from '@shared/publishers/PrimFePublisher';
import { PrimFctlBusEvents } from '@shared/publishers/PrimFctlPublisher';
import { PrimFgBusEvents, PrimFgBusBaseEvents } from '@shared/publishers/PrimFgPublisher';

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

const primFgSubjectsByKey = {
  prim_pfd_speed_target: BusSignal.from(null, 0),
  prim_pfd_short_term_managed_speed: BusSignal.from(null, 0),
  prim_presel_mach: BusSignal.from(null, 0),
  prim_presel_speed: BusSignal.from(null, 0),
  prim_selected_speed: BusSignal.from(null, 0),
  prim_selected_mach: BusSignal.from(null, 0),
  prim_selected_heading: BusSignal.from(null, 0),
  prim_selected_track: BusSignal.from(null, 0),
  prim_selected_altitude: BusSignal.from(null, 0),
  prim_selected_vertical_speed: BusSignal.from(null, 0),
  prim_selected_flight_path_angle: BusSignal.from(null, 0),
  prim_runway_hdg_memo: BusSignal.from(null, 0),
  prim_roll_fd_command_1: BusSignal.from(null, 0),
  prim_pitch_fd_command_1: BusSignal.from(null, 0),
  prim_yaw_fd_command_1: BusSignal.from(null, 0),
  prim_roll_fd_command_2: BusSignal.from(null, 0),
  prim_pitch_fd_command_2: BusSignal.from(null, 0),
  prim_yaw_fd_command_2: BusSignal.from(null, 0),
  prim_fm_alt_constraint: BusSignal.from(null, 0),
  prim_speed_margin_high: BusSignal.from(null, 0),
  prim_speed_margin_low: BusSignal.from(null, 0),
  prim_fg_discrete_word_1: BusSignal.from(null, 0),
  prim_fg_discrete_word_2: BusSignal.from(null, 0),
  prim_fg_discrete_word_3: BusSignal.from(null, 0),
  prim_fg_discrete_word_4: BusSignal.from(null, 0),
  prim_fg_discrete_word_5: BusSignal.from(null, 0),
  prim_fg_discrete_word_6: BusSignal.from(null, 0),
  prim_fg_ats_discrete_word: BusSignal.from(null, 0),
  prim_fg_ats_fma_discrete_word: BusSignal.from(null, 0),
} satisfies Record<keyof PrimFgBusBaseEvents, BusSignal<number>>;

export class PrimChoiceProvider {
  private readonly sub = this.bus.inbox<PrimFeBusEvents & PrimFctlBusEvents & PrimFgBusEvents>();

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

  private readonly primFgSubjects = new Map(
    Object.entries(primFgSubjectsByKey) as [keyof PrimFgBusBaseEvents, BusSignal<number>][],
  );

  constructor(private readonly bus: Bus) {}

  public init(): void {
    const publisher = this.bus.outbox<PrimFeBusBaseEvents & PrimFgBusBaseEvents>();

    this.selectedMasterPrim.watch((masterPrim) => {
      for (const [key, value] of this.primFeSubjects) {
        // The FE data over AFDX seems to update only at around 10Hz (see for example Vmax when moving)
        value.rebind(this.sub.topic(`${key}_${masterPrim}`).limit(10));
      }

      for (const [key, value] of this.primFgSubjects) {
        // TODO check frequency
        value.rebind(this.sub.topic(`${key}_${masterPrim}`).limit(10));
      }
    }, true);

    for (const [key, value] of this.primFeSubjects) {
      value.watch((word) => {
        publisher.post(key, word);
      }, true);
    }

    for (const [key, value] of this.primFgSubjects) {
      value.watch((word) => {
        publisher.post(key, word);
      }, true);
    }
  }
}
