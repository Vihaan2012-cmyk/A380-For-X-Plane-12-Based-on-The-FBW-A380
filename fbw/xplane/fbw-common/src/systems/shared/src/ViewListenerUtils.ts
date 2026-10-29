import { Channels, openChannel } from 'xplane-js/data';
export class ViewListenerUtils {
  public static getListener(listenerName: string, singleton = true): Promise<Channels.Channel> {
    return new Promise((resolve, reject) => {
      let listener: Channels.Channel | undefined;
      // eslint-disable-next-line prefer-const
      listener = openChannel(listenerName, () => (listener ? resolve(listener) : reject()), singleton);
    });
  }
}
