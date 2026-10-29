//  Copyright (c) 2024 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { SignalList, View, Bus, UI, SignalMap, Signal, ViewNode } from 'xplane-js/ui';
import { Button } from '../MsfsAvionicsCommon/UiWidgets/Button';
import { IconButton } from '../MsfsAvionicsCommon/UiWidgets/IconButton';
import { MouseCursor } from '../MsfsAvionicsCommon/UiWidgets/MouseCursor';
import {
  AtsuMailboxMessages,
  CpdlcMessage,
  Conversion,
  DclMessage,
  MailboxStatusMessage,
  AtsuMessageSerializationFormat,
  CpdlcMessageMonitoringState,
  AtsuMessageDirection,
} from '@datalink/common';

import './style.scss';

export class MailboxMessageBlock {
  public messages: CpdlcMessage[] = [];
  public timestamp: number = 0;
  public response: number = -1;
  public statusMessage: MailboxStatusMessage = MailboxStatusMessage.NoMessage;
  public messageVisible: Signal<boolean> = Signal.of<boolean>(false);
  public automaticCloseTimeout: Signal<number> = Signal.of<number>(-1);
  public semanticResponseIncomplete: boolean = false;
  public reachEndOfMessage: boolean = false;

  constructor() {
    this.timestamp = new Date().getTime();
  }
}

export interface AtcMailboxProps {
  readonly bus: Bus;
}

export class AtcMailbox extends View<AtcMailboxProps> {
  private readonly topRef = UI.ref<HTMLDivElement>();

  private readonly mouseCursorRef = UI.ref<MouseCursor>();

  private onMouseMove(ev: MouseEvent) {
    this.mouseCursorRef.peek()?.updatePosition(ev.clientX, ev.clientY - 768);
  }

  private onMouseMoveHandler = this.onMouseMove.bind(this);

  private readonly publisher = this.props.bus.outbox<AtsuMailboxMessages>();
  private readonly subs = this.props.bus.inbox<AtsuMailboxMessages>();

  private messages: SignalMap<number, MailboxMessageBlock> = SignalMap.of<number, MailboxMessageBlock>();

  private messageIndex: Signal<number> = Signal.of<number>(-1);
  private messageReadComplete: Signal<boolean> = Signal.of(false);
  private visibleMessageSemanticResponseIncomplete: Signal<boolean> = Signal.of<boolean>(false);
  private visibleMessages: SignalList<CpdlcMessage> = SignalList.of();
  private visibleMessageStatus: Signal<MailboxStatusMessage> = Signal.of<MailboxStatusMessage>(
    MailboxStatusMessage.NoMessage,
  );
  private response: Signal<number> = Signal.of<number>(-1);

  private mailboxText = Signal.of<string>('');

  private mailboxTime = Signal.of<string>('');

  private mailboxStation = Signal.of<string>('');

  private mailboxOpen = Signal.of<string>('');

  private handleIncomingMessages(cpdlcMessages: CpdlcMessage[]): void {
    console.log('message received');
    console.log(cpdlcMessages);

    // convert messages to enhanced format
    const enhancedMessages: CpdlcMessage[] = [];
    cpdlcMessages.forEach((message) => {
      enhancedMessages.push(Conversion.messageDataToMessage(message) as CpdlcMessage);
      console.log(enhancedMessages[0].serialize(AtsuMessageSerializationFormat.Mailbox));
      this.mailboxText.set(enhancedMessages[0].serialize(AtsuMessageSerializationFormat.Mailbox));
      this.mailboxTime.set(enhancedMessages[0].Timestamp.mailboxTimestamp());
      this.mailboxStation.set(
        `${enhancedMessages[0].Direction === AtsuMessageDirection.Downlink ? 'TO' : 'FROM'} ${enhancedMessages[0].Station}`,
      );
      this.mailboxOpen.set('OPEN');
    });

    if (enhancedMessages.length !== 0) {
      // check if incoming message already exist in messages
      const messageBlock = this.messages.lookup(enhancedMessages[0].UniqueMessageID);

      if (messageBlock !== undefined) {
        // update the communication states and reponses of the existing message
        // messageBlock.messages.set(enhancedMessages);
      } else {
        // store new message in messages
        const message = new MailboxMessageBlock();
        message.messages = enhancedMessages;
        if (enhancedMessages[0].MessageMonitoring === CpdlcMessageMonitoringState.Monitoring) {
          message.statusMessage = MailboxStatusMessage.Monitoring;
        } else if (enhancedMessages[0].MessageMonitoring === CpdlcMessageMonitoringState.Cancelled) {
          message.statusMessage = MailboxStatusMessage.MonitoringCancelled;
        }
        this.messages.assign(enhancedMessages[0].UniqueMessageID, message);
      }

      // TODO implement semantic response section

      // display message
      if (this.messages.size === 1) {
        const message = this.messages.lookup(enhancedMessages[0].UniqueMessageID);
        if (message) {
          message.messageVisible.set(true);
          this.messages.assign(enhancedMessages[0].UniqueMessageID, message);
          this.publisher.post('visibleMessage', message.messages[0].UniqueMessageID, true, false);
        }
      }
    }
  }

  private sendMessage(uid: number) {
    this.publisher.post('downlinkTransmit', uid, true, false);
  }

  private sortedMessageArray(messages: SignalMap<number, MailboxMessageBlock>) {
    const arrMessages = Array.from(messages.get().values());
    arrMessages.sort((a, b) => a.timestamp - b.timestamp);
    return arrMessages;
  }

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.topRef.target.addEventListener('mousemove', this.onMouseMoveHandler);

    this.subs.topic('cpdlcMessages').listen((messages: CpdlcMessage[]) => this.handleIncomingMessages(messages));
    this.subs.topic('dclMessages').listen((messages: DclMessage[]) => this.handleIncomingMessages(messages));

    this.messages.watch((messages) => {
      if (messages.size === 0) return;

      // find the first visible message
      const arrMessages = this.sortedMessageArray(this.messages);
      this.messageIndex.set(arrMessages.findIndex((element) => element.messageVisible));

      if (this.messageIndex.get() !== -1) {
        this.response.set(arrMessages[this.messageIndex.get()].response);
        this.visibleMessages.set(arrMessages[this.messageIndex.get()].messages);
        this.messageReadComplete.set(arrMessages[this.messageIndex.get()].reachEndOfMessage);
        this.visibleMessageStatus.set(arrMessages[this.messageIndex.get()].statusMessage);
        this.visibleMessageSemanticResponseIncomplete.set(
          arrMessages[this.messageIndex.get()].semanticResponseIncomplete,
        );
      }

      // TODO: check for priority messages
    });
  }

  destroy(): void {
    this.topRef.peek()?.removeEventListener('mousemove', this.onMouseMoveHandler);
    this.mouseCursorRef.peek()?.destroy();

    super.destroy();
  }

  render(): ViewNode | null {
    return (
      <div ref={this.topRef} class="atc-mailbox-top-layout">
        <div class="atc-mailbox-left-layout">
          <Button label="PRINT" onClick={() => {}} buttonStyle="height: 50px;"></Button>
          <Button label="RECALL" onClick={() => {}} buttonStyle="height: 50px;"></Button>
        </div>
        <div class="atc-mailbox-center-layout">
          <div class="atc-mailbox-center-top">
            <div class="atc-mailbox-msg-status atc-mailbox-text">
              <span>{this.mailboxTime}</span>
              <span>{this.mailboxStation}</span>
              <span class="status-msg status-open">{this.mailboxOpen}</span>
            </div>
            <div class="atc-mailbox-msg-area">
              <div class="atc-mailbox-msg-body atc-mailbox-text">{this.mailboxText}</div>
              <div class="atc-mailbox-msg-pg-number-indication">
                <IconButton
                  icon="double-up"
                  onClick={() => {}}
                  // disabled={this.isCurrentMsgFirstPage}
                  containerStyle="width: 70px; height: 40px; padding:4px"
                />
                <IconButton
                  icon="double-down"
                  onClick={() => {}}
                  // disabled={this.isCurrentMsgLastPage}
                  containerStyle="width: 70px; height: 40px; padding:4px"
                />
              </div>
            </div>
          </div>
          <div class="atc-mailbox-center-bottom">
            <div class="atc-mailbox-cb-1" />
            <div class="atc-mailbox-cb-2" />
          </div>
        </div>
        <div class="atc-mailbox-right-layout">
          <Button label="SEND" onClick={() => {}} buttonStyle="height: 50px; justify-content: flex-end;"></Button>
          <Button label="STANBY" onClick={() => {}} buttonStyle="height: 50px; justify-content: flex-end;"></Button>
          <Button label="UNABLE" onClick={() => {}} buttonStyle="height: 50px; justify-content: flex-end;"></Button>
          <Button label="LOAD SEC3" onClick={() => {}} buttonStyle="height: 50px; justify-content: flex-end;"></Button>
          <Button label="PRINT" onClick={() => {}} buttonStyle="height: 50px; justify-content: flex-end;"></Button>
        </div>
        <MouseCursor side={Signal.of('CAPT')} ref={this.mouseCursorRef} />
      </div>
    );
  }
}
