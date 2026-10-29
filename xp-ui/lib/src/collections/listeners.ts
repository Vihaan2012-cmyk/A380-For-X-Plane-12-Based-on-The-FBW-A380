import type { Bond } from 'xplane-js/ui';

export class Listener<H> implements Bond {
  private live = true;
  private idle: boolean;

  constructor(
    readonly fn: H,
    private readonly roster: Set<Listener<H>>,
    private readonly catchUp: (target: Listener<H>) => void,
    startIdle: boolean,
  ) {
    this.idle = startIdle;
  }

  get isLive(): boolean {
    return this.live;
  }

  get isHeld(): boolean {
    return this.idle;
  }

  hold(): this {
    this.idle = true;
    return this;
  }

  release(initialNotify = false): this {
    if (this.live && this.idle) {
      this.idle = false;
      if (initialNotify) {
        this.catchUp(this);
      }
    }
    return this;
  }

  destroy(): void {
    this.live = false;
    this.roster.delete(this);
  }
}

export class ListenerSet<H> {
  private readonly roster = new Set<Listener<H>>();

  join(fn: H, initialNotify: boolean, paused: boolean, catchUp: (target: Listener<H>) => void): Listener<H> {
    const entry = new Listener<H>(fn, this.roster, catchUp, paused);
    this.roster.add(entry);
    if (initialNotify && !paused) {
      catchUp(entry);
    }
    return entry;
  }

  dispatch(call: (fn: H) => void): void {
    for (const entry of Array.from(this.roster)) {
      if (entry.isLive && !entry.isHeld) {
        try {
          call(entry.fn);
        } catch (err) {
          console.error(err);
        }
      }
    }
  }
}
