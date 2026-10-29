// @ts-strict-ignore
import { ViewProps, View, Signal } from 'xplane-js/ui';

export abstract class NDPage<P extends ViewProps = ViewProps> extends View<P> {
  isVisible: Signal<boolean>;

  onShow(): void {
    // noop
  }

  onHide(): void {
    /// noop
  }
}
