// @ts-strict-ignore
import { View, UI, Bond, ViewNode } from 'xplane-js/ui';

export abstract class DestroyableComponent<T> extends View<T> {
  /** Make sure to collect all subscriptions (Consumer, MappedSubjects, ...) here, so we can destroy them when destroying the page */
  protected readonly subscriptions: Bond[] = [];

  destroy(): void {
    for (const s of this.subscriptions) {
      s.destroy();
    }

    super.destroy();
  }

  public afterRender(node: ViewNode): void {
    super.afterRender(node);
  }

  render(): ViewNode | null {
    return <></>;
  }

  public pauseSubscriptions() {
    for (const s of this.subscriptions) {
      s.hold();
    }
  }

  public resumeSubscriptions() {
    for (const s of this.subscriptions) {
      s.release();
    }
  }
}

export function isSubscription(object: any): object is Bond {
  return (
    object !== undefined &&
    object.isLive !== undefined &&
    object.isHeld !== undefined &&
    object.canInitialNotify !== undefined &&
    object.hold !== undefined &&
    object.release !== undefined &&
    object.destroy !== undefined &&
    typeof object.isLive === 'boolean' &&
    typeof object.isHeld === 'boolean' &&
    typeof object.canInitialNotify === 'boolean' &&
    typeof object.hold === 'function' &&
    typeof object.release === 'function' &&
    typeof object.destroy === 'function'
  );
}
