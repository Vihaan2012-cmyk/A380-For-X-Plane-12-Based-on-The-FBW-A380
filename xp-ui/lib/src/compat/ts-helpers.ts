// The helpers that TypeScript's down-levelled package output imports from 'tslib', written on the language's built-ins.

type Bag = Record<PropertyKey, any>;

export const __assign: (target: Bag, ...sources: Bag[]) => Bag = Object.assign;

export function __extends(derived: any, base: any): void {
  Object.setPrototypeOf(derived, base);
  derived.prototype =
    base === null
      ? Object.create(null)
      : Object.create(base.prototype, { constructor: { value: derived, writable: true, enumerable: true, configurable: true } });
}

export function __rest(source: any, excluded: PropertyKey[]): Bag {
  const rest: Bag = {};
  if (source == null) {
    return rest;
  }
  for (const key of Object.keys(source)) {
    if (!excluded.includes(key)) {
      rest[key] = source[key];
    }
  }
  for (const key of Object.getOwnPropertySymbols(source)) {
    if (!excluded.includes(key) && Object.prototype.propertyIsEnumerable.call(source, key)) {
      rest[key] = source[key];
    }
  }
  return rest;
}

export function __values(o: any): Iterator<unknown> {
  const iterate = o[Symbol.iterator];
  if (iterate) {
    return iterate.call(o);
  }
  if (o && typeof o.length === 'number') {
    let index = 0;
    let finished = false;
    return {
      next(): IteratorResult<unknown> {
        finished = finished || index >= o.length;
        return finished ? { value: undefined, done: true } : { value: o[index++], done: false };
      },
    };
  }
  throw new TypeError('Object is not iterable.');
}

export function __read(o: any, n?: number): any {
  const iterate = o[Symbol.iterator];
  if (!iterate) {
    return o;
  }
  const iterator: Iterator<unknown> = iterate.call(o);
  const items: unknown[] = [];
  let step: IteratorResult<unknown> | undefined;
  let failure: { error: unknown } | undefined;
  try {
    while ((n === undefined || items.length < n) && !(step = iterator.next()).done) {
      items.push(step.value);
    }
  } catch (error) {
    failure = { error };
  }
  try {
    if (step && !step.done && typeof iterator.return === 'function') {
      iterator.return();
    }
  } finally {
    if (failure) {
      throw failure.error;
    }
  }
  return items;
}

export function __spread(...lists: any[]): any[] {
  return lists.reduce((all: any[], list) => all.concat(__read(list)), []);
}
