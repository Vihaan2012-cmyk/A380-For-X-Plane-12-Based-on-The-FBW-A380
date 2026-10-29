import type * as Api from 'xplane-js/ui';
import * as Mine from './index';

export const checkEventBus: typeof Api.Bus = Mine.Bus;
export const checkBasicConsumer: typeof Api.StreamSource = Mine.StreamSource;
export const checkClock: typeof Api.SimClock = Mine.SimClock;
export const checkState: typeof Api.SessionState = Mine.SessionState;
export const checkPacer: typeof Api.Pacer = Mine.Pacer;
export const checkFeed: typeof Api.makeBusState = Mine.makeBusState;
export const checkEnum: typeof Api.VarUnit = Mine.VarUnit;

export const apiConsumer = <T>(c: Mine.Stream<T>): Api.Stream<T> => c;
export const mineConsumer = <T>(c: Api.Stream<T>): Mine.Stream<T> => c;
export const apiSubscriber = <E>(c: Mine.Inbox<E>): Api.Inbox<E> => c;
export const apiPublisher = <E>(c: Mine.Outbox<E>): Api.Outbox<E> => c;
export const apiBackplane = (c: Mine.BoardFeeder): Api.BoardFeeder => c;
export const apiEntry = <T>(c: Mine.VarTopic<T>): Api.VarTopic<T> => c;
export const apiClock = (c: Mine.ClockTopics): Api.ClockTopics => c;
export const apiKeys = (c: Mine.KeyTopics): Api.KeyTopics => c;
export const apiHub: typeof Api.Bus = Mine.Exchange;
export const apiBasicTap: typeof Api.StreamSource = Mine.PipeSource;

export const checkSubjectCreate: typeof Api.BusSignal.from = Mine.BusSignal.from;
export const checkValueCreate: typeof Api.BusValue.from = Mine.BusValue.from;
export const checkKeysGet: typeof Api.KeyControl.connect = Mine.KeyControl.connect;
export const apiSubjectInst = <T>(c: Mine.BusSignal<T>): Api.BusSignal<T> => c as unknown as Api.BusSignal<T>;
type Shape<A> = { begin(): void; tick(): void } & A;
export const apiHEvent = (c: Mine.PanelEventPublisher): Shape<{ dispatchPanelEvent(h: string, sync?: boolean): void }> => c;
export const apiKeyInst = (c: Mine.KeyControl): Pick<Api.KeyControl, 'intercept' | 'fire'> => c;
export const apiSimPub = <E>(c: Mine.VarPublisher<E>): Api.BoardFeeder => c;
