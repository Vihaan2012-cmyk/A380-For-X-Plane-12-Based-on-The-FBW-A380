import type { Readable } from 'xplane-js/ui';

export class Waits {
  public static delay(delay: number): Promise<void> {
    return new Promise<void>((resolve) => {
      setTimeout(resolve, delay);
    });
  }

  public static frames(frames: number): Promise<void> {
    return new Promise<void>((resolve) => {
      let left = Math.max(0, Math.floor(frames));
      const step = () => {
        if (left <= 0) {
          resolve();
          return;
        }
        left--;
        requestAnimationFrame(step);
      };
      step();
    });
  }

  public static until(predicate: () => boolean, interval = 0, timeout = 0): Promise<void> {
    return new Promise<void>((resolve, reject) => {
      const startedAt = Date.now();
      const poll = () => {
        if (predicate()) {
          resolve();
        } else if (timeout > 0 && Date.now() - startedAt >= timeout) {
          reject(new Error('awaitCondition timed out'));
        } else {
          setTimeout(poll, interval);
        }
      };
      poll();
    });
  }

  public static forValue<T>(
    subscribable: Readable<T>,
    predicate?: (value: T) => boolean,
    takeInitialValue = false,
    timeout = 0,
  ): Promise<T> {
    const accepts = predicate ?? ((v: T) => !!v);
    return new Promise<T>((resolve, reject) => {
      let timer: ReturnType<typeof setTimeout> | undefined;
      let done = false;
      let sub: { destroy(): void } | undefined;
      const finish = (value: T) => {
        done = true;
        if (timer !== undefined) {
          clearTimeout(timer);
        }
        sub?.destroy();
        resolve(value);
      };
      sub = subscribable.watch(
        (value) => {
          if (!done && accepts(value)) {
            finish(value);
          }
        },
        takeInitialValue,
      );
      if (done) {
        sub.destroy();
      } else if (timeout > 0) {
        timer = setTimeout(() => {
          if (!done) {
            done = true;
            sub?.destroy();
            reject(new Error('awaitSubscribable timed out'));
          }
        }, timeout);
      }
    });
  }
}

export class Debouncer {
  private handle: ReturnType<typeof setTimeout> | undefined = undefined;

  public waiting(): boolean {
    return this.handle !== undefined;
  }

  public arm(task: () => void, delay: number): void {
    this.cancel();
    this.handle = setTimeout(() => {
      this.handle = undefined;
      task();
    }, delay);
  }

  public cancel(): void {
    if (this.handle !== undefined) {
      clearTimeout(this.handle);
      this.handle = undefined;
    }
  }
}
