//  Copyright (c) 2026 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import { StateBase as ReadableBase, Pipe as Stream, Lifetime as Lifespan, Handle as Bond } from 'xplane-js/ui';
import { Arinc429Register, Arinc429WordData } from './arinc429';

type Arinc429Input = Arinc429WordData;

export class Arinc429ConsumerSubject extends ReadableBase<Arinc429WordData> {
  public readonly canNotifyNow = true;

  private readonly word = Arinc429Register.empty();

  private consumerSub: Bond | undefined;

  private paused = false;

  private alive = true;

  private constructor(consumer: Stream<Arinc429Input> | null | undefined) {
    super();
    this.follow(consumer ?? null);
  }

  public static create(consumer: Stream<Arinc429Input> | null | undefined): Arinc429ConsumerSubject {
    return new Arinc429ConsumerSubject(consumer);
  }

  public get isLive(): boolean {
    return this.alive;
  }

  public get isHeld(): boolean {
    return this.paused;
  }

  public get(): Arinc429WordData {
    return this.word;
  }

  public follow(consumer: Stream<Arinc429Input> | null): this {
    return this.track(consumer);
  }

  public setConsumer(consumer: Stream<Arinc429Input> | null): this {
    return this.track(consumer);
  }

  private track(consumer: Stream<Arinc429Input> | null): this {
    if (!this.alive) {
      return this;
    }
    this.consumerSub?.destroy();
    this.consumerSub = consumer ? consumer.listen((value) => this.onValue(value), this.paused) : undefined;
    return this;
  }

  public reset(initialVal: Arinc429Input, consumer: Stream<Arinc429Input> | null = null): this {
    this.consumerSub?.destroy();
    this.consumerSub = undefined;
    this.onValue(initialVal);
    return this.follow(consumer);
  }

  public hold(): this {
    if (!this.paused) {
      this.paused = true;
      this.consumerSub?.hold();
    }
    return this;
  }

  public release(): this {
    if (this.paused) {
      this.paused = false;
      this.consumerSub?.release(true);
    }
    return this;
  }

  public scoped(lifecycle: Lifespan): this {
    lifecycle.adopt(this);
    return this;
  }

  public destroy(): void {
    this.alive = false;
    this.consumerSub?.destroy();
    this.consumerSub = undefined;
  }

  private onValue(input: Arinc429Input): void {
    if (input.value !== this.word.value || input.ssm !== this.word.ssm) {
      this.word.set(input.rawWord);
      this.emit();
    }
  }
}

export class Arinc429LocalVarConsumerSubject extends ReadableBase<Arinc429WordData> {
  public readonly canNotifyNow = true;

  private readonly word = Arinc429Register.empty();

  private consumerSub: Bond | undefined;

  private paused = false;

  private alive = true;

  private constructor(consumer: Stream<number> | null | undefined, initialVal: number | null) {
    super();
    if (initialVal !== null) {
      this.word.set(initialVal);
    }
    this.follow(consumer ?? null);
  }

  public static create(
    consumer: Stream<number> | null | undefined,
    initialVal: number | null = 0,
  ): Arinc429LocalVarConsumerSubject {
    return new Arinc429LocalVarConsumerSubject(consumer, initialVal);
  }

  public get isLive(): boolean {
    return this.alive;
  }

  public get isHeld(): boolean {
    return this.paused;
  }

  public get(): Arinc429WordData {
    return this.word;
  }

  public follow(consumer: Stream<number> | null): this {
    return this.track(consumer);
  }

  public setConsumer(consumer: Stream<number> | null): this {
    return this.track(consumer);
  }

  private track(consumer: Stream<number> | null): this {
    if (!this.alive) {
      return this;
    }
    this.consumerSub?.destroy();
    this.consumerSub = consumer ? consumer.listen((value) => this.onValue(value), this.paused) : undefined;
    return this;
  }

  public hold(): this {
    if (!this.paused) {
      this.paused = true;
      this.consumerSub?.hold();
    }
    return this;
  }

  public release(): this {
    if (this.paused) {
      this.paused = false;
      this.consumerSub?.release(true);
    }
    return this;
  }

  public scoped(lifecycle: Lifespan): this {
    lifecycle.adopt(this);
    return this;
  }

  public destroy(): void {
    this.alive = false;
    this.consumerSub?.destroy();
    this.consumerSub = undefined;
  }

  private onValue(raw: number): void {
    if (raw !== this.word.rawWord) {
      this.word.set(raw);
      this.emit();
    }
  }
}
