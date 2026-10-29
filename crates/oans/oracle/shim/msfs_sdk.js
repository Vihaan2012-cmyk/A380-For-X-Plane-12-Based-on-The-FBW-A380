export class Subject {
  constructor(value) {
    this.value = value;
    this.subs = [];
  }
  static create(value) {
    return new Subject(value);
  }
  get() {
    return this.value;
  }
  set(value) {
    if (value !== this.value) {
      this.value = value;
      for (const fn of this.subs.slice()) fn(value);
    }
  }
  sub(fn, notify) {
    this.subs.push(fn);
    if (notify) fn(this.value);
    return { destroy() {} };
  }
}

export class EventBus {
  constructor() {
    this.handlers = {};
    this.cache = {};
    this.log = [];
  }
  getSubscriber() {
    const bus = this;
    return {
      on(topic) {
        return {
          changedOnly: false,
          whenChanged() {
            this.changedOnly = true;
            return this;
          },
          handle(fn) {
            let last;
            let first = true;
            const wrapped = (v) => {
              if (this.changedOnly && !first && last === v) return;
              first = false;
              last = v;
              fn(v);
            };
            (bus.handlers[topic] ||= []).push(wrapped);
            if (topic in bus.cache) wrapped(bus.cache[topic]);
            return { destroy() {} };
          },
        };
      },
    };
  }
  getPublisher() {
    const bus = this;
    return {
      pub(topic, data) {
        bus.cache[topic] = data;
        bus.log.push([topic, data]);
        for (const fn of (bus.handlers[topic] || []).slice()) fn(data);
      },
    };
  }
}

export class ConsumerSubject extends Subject {
  static create(consumer, initial) {
    const s = new ConsumerSubject(initial);
    if (consumer) s.setConsumer(consumer);
    return s;
  }
  setConsumer(consumer) {
    if (consumer) consumer.handle((v) => this.set(v));
  }
}

export class MappedSubject extends Subject {
  constructor(fn, inputs) {
    super(undefined);
    this.fn = fn;
    this.inputs = inputs;
    this.value = fn(inputs.map((i) => i.get()), undefined);
    for (const input of inputs) input.sub(() => this.update());
  }
  static create(fn, ...inputs) {
    return new MappedSubject(fn, inputs);
  }
  update() {
    this.set(this.fn(this.inputs.map((i) => i.get()), this.value));
  }
}

export class ArraySubject {
  constructor(arr) {
    this.arr = arr ?? [];
    this.subs = [];
  }
  static create(arr) {
    return new ArraySubject(arr);
  }
  notify() {
    for (const fn of this.subs.slice()) fn(this.arr);
  }
  get length() {
    return this.arr.length;
  }
  get(i) {
    return this.arr[i];
  }
  getArray() {
    return this.arr;
  }
  set(arr) {
    this.arr = arr.slice();
    this.notify();
  }
  insert(v) {
    this.arr.push(v);
    this.notify();
  }
  removeAt(i) {
    this.arr.splice(i, 1);
    this.notify();
  }
  clear() {
    this.arr = [];
    this.notify();
  }
  sub(fn, notify) {
    this.subs.push(fn);
    if (notify) fn(this.arr);
    return { destroy() {} };
  }
}
