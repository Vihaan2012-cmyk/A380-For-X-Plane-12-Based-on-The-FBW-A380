import { Arinc429LocalVarConsumerSubject } from '@flybywiresim/fbw-sdk';
import { BusSignal, Bus, BoardDevice, Derived, Signal } from 'xplane-js/ui';
import { FcdcBusBaseEvents, FcdcBusEvents } from '@shared/publishers/FcdcPublisher';
import { getDisplayIndex } from '../PFD';

const fcdcSubjectsByKey = {
  fcdc_discrete_word_1: BusSignal.from(null, 0),
  fcdc_discrete_word_2: BusSignal.from(null, 0),
  fcdc_discrete_word_3: BusSignal.from(null, 0),
  fcdc_discrete_word_4: BusSignal.from(null, 0),
  fcdc_discrete_word_5: BusSignal.from(null, 0),
  fcdc_fg_discrete_word_1: BusSignal.from(null, 0),
  fcdc_fg_discrete_word_2: BusSignal.from(null, 0),
  fcdc_fg_discrete_word_3: BusSignal.from(null, 0),
  fcdc_landing_fct_discrete_word: BusSignal.from(null, 0),
} satisfies Record<keyof FcdcBusBaseEvents, BusSignal<number>>;

export class FcdcChoiceProvider implements BoardDevice {
  private readonly sub = this.bus.inbox<FcdcBusEvents>();

  // We have to initialize this in init, as otherwise the content will not be ready and the getDisplayIndex method will fail
  private readonly isSide2 = Signal.of(false);

  private readonly fcdc1StatusWord1 = Arinc429LocalVarConsumerSubject.create(this.sub.topic('fcdc_discrete_word_1_1'));

  private readonly fcdc2StatusWord1 = Arinc429LocalVarConsumerSubject.create(this.sub.topic('fcdc_discrete_word_1_2'));

  private readonly useFcdc2 = Derived.from(
    ([fcdc1StatusWord1, fcdc2StatusWord1, isSide2]) => {
      const fcdc1Fault = fcdc1StatusWord1.isFailureWarning();
      const fcdc2Fault = fcdc2StatusWord1.isFailureWarning();

      return isSide2 ? fcdc2Fault && !fcdc1Fault : !(fcdc1Fault && !fcdc2Fault);
    },
    this.fcdc1StatusWord1,
    this.fcdc2StatusWord1,
    this.isSide2,
  );

  private readonly fcdcSubjects = new Map(
    Object.entries(fcdcSubjectsByKey) as [keyof FcdcBusBaseEvents, BusSignal<number>][],
  );

  constructor(private readonly bus: Bus) {}

  /** @inheritdoc */
  public setup(): void {
    const publisher = this.bus.outbox<FcdcBusBaseEvents>();

    this.isSide2.set(getDisplayIndex() === 2);

    this.useFcdc2.watch((useFcdc2) => {
      for (const [key, value] of this.fcdcSubjects) {
        const selectedFcdc = useFcdc2 ? 2 : 1;

        value.rebind(this.sub.topic(`${key}_${selectedFcdc}`));
      }
    }, true);

    for (const [key, value] of this.fcdcSubjects) {
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
