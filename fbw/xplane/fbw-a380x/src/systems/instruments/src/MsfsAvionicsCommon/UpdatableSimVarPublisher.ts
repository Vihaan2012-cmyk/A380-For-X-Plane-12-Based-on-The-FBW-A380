// @ts-strict-ignore
import { VarSpec, VarPublisher } from 'xplane-js/ui';

export class UpdatableSimVarPublisher<T> extends VarPublisher<T> {
  /**
   * Change the simvar read for a given key.
   * @param key The key of the simvar in simVarMap
   * @param value The new value to set the simvar to.
   * @deprecated Removed upstream and won't be needed when DMC switching implemented properly
   */
  public updateSimVarSource(key: keyof T & string, value: VarSpec): void {
    this.watchedTopics.delete(key);
    this.resolvedVars.set(key, value);
    super.onTopicWatched(key);
  }
}
