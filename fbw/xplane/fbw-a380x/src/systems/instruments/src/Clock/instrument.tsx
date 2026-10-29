import { Bus, UI, PanelEventPublisher } from 'xplane-js/ui';
import { ClockRoot, ClockSimvarPublisher } from '@flybywiresim/clock';

import './Clock.scss';
import { PanelInstrument, defineInstrument } from 'xplane-js/data';

// eslint-disable-next-line camelcase
class A380X_Clock extends PanelInstrument {
  private bus: Bus;

  private readonly hEventPublisher: PanelEventPublisher;

  private simVarPublisher: ClockSimvarPublisher;

  /**
   * "mainmenu" = 0
   * "loading" = 1
   * "briefing" = 2
   * "ingame" = 3
   */
  private gameState = 0;

  constructor() {
    super();
    this.bus = new Bus();
    this.simVarPublisher = new ClockSimvarPublisher(this.bus);
    this.hEventPublisher = new PanelEventPublisher(this.bus);
  }

  get templateId(): string {
    return 'A380X_Clock';
  }

  public onPanelEvent(args: string[]): void {
    this.hEventPublisher.dispatchPanelEvent(args[0]);
  }

  public onAttach(): void {
    super.onAttach();

    this.hEventPublisher.begin();

    UI.mount(<ClockRoot bus={this.bus} />, document.getElementById('Clock_CONTENT'));

    // Remove "instrument didn't load" text
    document?.getElementById('Clock_CONTENT')?.querySelector(':scope > h1')?.remove();
  }

  public onFrame(): void {
    super.onFrame();

    if (this.gameState !== 3) {
      const gamestate = this.sessionState();
      if (gamestate === 3) {
        this.simVarPublisher.begin();
      }
      this.gameState = gamestate;
    } else {
      this.simVarPublisher.tick();
    }
  }
}

defineInstrument('a380x-clock', A380X_Clock);
