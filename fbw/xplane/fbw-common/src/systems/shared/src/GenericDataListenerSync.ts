// @ts-strict-ignore
import { Channels, openRelay } from 'xplane-js/data';
interface GenericDataListener extends Channels.Channel {
  receive(key: string, callback: (data: any) => void): void;
  send(key: string, data: any): void;
}

const openListenerRelay = (callback?: () => void): GenericDataListener => openRelay(callback) as unknown as GenericDataListener;

interface SyncDataPackage {
  packagedId: number;
  data: TopicDataPackage[];
}

interface TopicDataPackage {
  topic: string;
  data: any;
}

export class GenericDataListenerSync {
  static EB_LISTENER_KEY = 'EB_EVENTS';

  private dataPackageQueue: any[];

  private topic: string;

  private isRunning: boolean;

  private listener: GenericDataListener;

  private lastEventSynced = -1;

  private recvEventCb: (topic: string, data: any) => void;

  constructor(recvEventCb?: (topic: string, data: any) => void, topic?: string) {
    this.topic = topic;
    this.dataPackageQueue = [];
    this.isRunning = true;
    this.recvEventCb = recvEventCb;

    this.listener = openListenerRelay(() => {
      this.listener.receive(GenericDataListenerSync.EB_LISTENER_KEY, (data) => {
        this.processEventsReceived(data);
      });
    });

    const sendFn = () => {
      if (this.dataPackageQueue.length > 0) {
        const syncDataPackage = { packagedId: Math.floor(Math.random() * 1000000000), data: this.dataPackageQueue };
        this.listener.send(GenericDataListenerSync.EB_LISTENER_KEY, syncDataPackage);
        this.dataPackageQueue.length = 0;
      }
      if (this.isRunning) {
        requestAnimationFrame(sendFn);
      }
    };
    requestAnimationFrame(sendFn);
  }

  public stop() {
    this.isRunning = false;
  }

  /**
   * Processes events received from onInteractionEvent and executes the configured callback.
   * @param target always empty
   * @param args SyncDataPackage
   */
  processEventsReceived(syncPackage: SyncDataPackage) {
    const syncDataPackage = syncPackage;
    if (syncDataPackage.packagedId !== this.lastEventSynced) {
      this.lastEventSynced = syncDataPackage.packagedId;
      syncDataPackage.data.forEach((data) => {
        if (data.topic === this.topic) {
          try {
            this.recvEventCb(data.topic, data.data !== undefined ? data.data : undefined);
          } catch (e) {
            console.error(e);
            if (e instanceof Error) {
              console.error(e.stack);
            }
          }
        }
      });
    }
  }

  /**
   * Sends an event via flow events.
   * @param topic The topic to send data on.
   * @param data The data to send.
   */
  sendEvent(topic: string, data: any) {
    const dataObj = data;

    const dataPackage: TopicDataPackage = {
      topic,
      data: dataObj,
    };

    this.dataPackageQueue.push(dataPackage);
  }

  receiveEvent() {
    // noop
  }
}
