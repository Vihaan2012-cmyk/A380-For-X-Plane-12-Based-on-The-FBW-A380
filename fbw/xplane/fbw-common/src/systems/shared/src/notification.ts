// @ts-strict-ignore
import { Bus, KeyTopics, KeyControl } from 'xplane-js/ui';
import { Channels, openChannel } from 'xplane-js/data';

let nxNotificationsListener: Channels.Channel;

export enum NotificationType {
  Message = 'MESSAGE',
  Subtitles = 'SUBTITLES',
}

export enum NotificationTheme {
  Tips = 'TIPS',
  Gameplay = 'GAMEPLAY',
  System = 'SYSTEM',
}

export enum NotificationImage {
  Notification = 'IMAGE_NOTIFICATION',
  Score = 'IMAGE_SCORE',
}

/** Parameters that may be provided for construction of a notification */
export interface NotificationParameters {
  /** Type of the notification */
  type: NotificationType;
  /** Theme of the notification */
  theme: NotificationTheme;
  /** Image icon to display */
  image: NotificationImage;
  /** Title of the notification */
  title: string;
  /** Message to display (can be multiline) */
  message: string;
  /** Time in ms before notification message will disappear */
  timeout: number;
}

/**
 * NotificationData container for notifications to package notification metadata
 */
export interface NotificationData extends Omit<NotificationParameters, 'message'> {
  id: string;
  description: string;
  timeout: number;
  time: number;
}

/**
 * Notification utility class to create a notification event and element
 *
 * Usage:
 * import { NotificationManager } from '@flybywiresim/fbw-sdk';
 * ...
 * const notification = new NotificationManager();
 * notification.showNotification({ message: 'Your notification here!' });
 */

export class NotificationManager {
  manager: KeyControl;

  notifications: Notification[];

  constructor(private readonly bus: Bus) {
    this.notifications = [];
    KeyControl.connect(this.bus).then((man) => {
      this.manager = man;
      this.registerIntercepts();
    });
  }

  registerIntercepts() {
    this.manager.intercept('PAUSE_TOGGLE', true);
    this.manager.intercept('PAUSE_ON', true);
    this.manager.intercept('PAUSE_OFF', true);
    this.manager.intercept('PAUSE_SET', true);

    const subscriber = this.bus.inbox<KeyTopics>();
    subscriber.topic('key_intercept').listen((keyData) => {
      switch (keyData.key) {
        case 'PAUSE_TOGGLE':
        case 'PAUSE_ON':
        case 'PAUSE_OFF':
        case 'PAUSE_SET':
          this.notifications.forEach((notif: Notification) => {
            notif.hideNotification();
          });
          this.notifications.length = 0;
          break;
        default:
          break;
      }
    });
  }

  showNotification(params: Partial<NotificationParameters> = {}): void {
    const notif: Notification = new Notification();
    notif.showNotification(params);
    this.notifications.push(notif);
  }
}

class Notification {
  time: number;

  params: NotificationData;

  /**
   * Creates a Notification
   */
  constructor() {
    const title = 'A32NX ALERT';
    this.time = new Date().getTime();
    this.params = {
      id: `${title}_${this.time}`,
      title,
      type: NotificationType.Message,
      theme: NotificationTheme.Gameplay,
      image: NotificationImage.Notification,
      description: 'Default Message',
      timeout: 10000,
      time: this.time,
    };
  }

  /**
   * Modify the display data for this Notification
   * @param params Parameters for the notification
   */
  private setData(params: Partial<NotificationParameters> = {}): void {
    if (params.title) {
      this.params.title = params.title;
      this.params.id = `${params.title}_${new Date().getTime()}`;
    }
    if (params.type) {
      this.params.type = params.type;
    }
    if (params.theme) {
      this.params.theme = params.theme;
    }
    if (params.image) {
      this.params.image = params.image;
    }
    if (params.message) {
      this.params.description = params.message;
    }
    if (params.timeout) {
      this.params.timeout = params.timeout;
    }
  }

  /**
   * Show notification with given or already initiated parametrs.
   * @param params Parameters for the notification
   */
  showNotification(params: Partial<NotificationParameters> = {}): void {
    this.setData(params);

    if (!nxNotificationsListener) {
      nxNotificationsListener = openChannel('notifications');
    }
    nxNotificationsListener.broadcast('notification.show', this.params);
    setTimeout(() => {
      // TODO FIXME: May break in the future, check every update
      this.hideNotification();
    }, this.params.timeout);
  }

  hideNotification() {
    nxNotificationsListener.broadcast('notification.hide', this.params.type, null, this.params.id);
  }
}
