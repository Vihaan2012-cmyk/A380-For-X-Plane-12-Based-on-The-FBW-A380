// Copyright (c) 2025 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { Settable, VarUnit } from 'xplane-js/ui';
import { Bridge, Vars } from 'xplane-js/data';

export class RegisteredSimVar<T> implements Settable<T> {
  protected constructor(protected readonly id: number) {}

  /**
   * Creates a new registered "simvar" that registers a handle for faster get/set of a local or sim var.
   * Note: use {@link RegisteredSimVar.createBoolean} for boolean variables.
   * @param name The name of the local or sim var.
   * @param unit The MSFS unit to read/write the var with. MSFS will perform the unit conversions required.
   * @param source An optional source, which is used somewhere deep inside MSFS for... something.
   */
  public static create(name: string, unit: VarUnit.String, source?: string): RegisteredSimVar<string>;
  public static create<T extends number, U extends VarUnit | string = VarUnit | string>(
    name: string,
    unit: U,
    source?: string,
  ): U extends VarUnit.Bool | 'bool' | 'boolean' ? never : RegisteredSimVar<T>;
  public static create(name: string, unit: VarUnit | string, source = ''): RegisteredSimVar<unknown> {
    switch (unit) {
      case VarUnit.String:
        return new RegisteredStringSimVar(Vars.register(name, unit, source));
    }
    return new RegisteredSimVar(Vars.register(name, unit, source));
  }

  /**
   * Creates a new registered boolean "simvar" that registers a handle for faster get/set of a local or sim var.
   * It is converted to true/false, rather than non-zero/zero.
   * @param name The name of the local or sim var.
   * @param source An optional source, which is used somewhere deep inside MSFS for... something.
   */
  public static createBoolean(name: string, source = ''): RegisteredSimVar<boolean> {
    return new RegisteredBooleanSimVar(Vars.register(name, VarUnit.Bool, source));
  }

  /** @inheritdoc */
  public get(): T {
    return Vars.readById(this.id) as T;
  }

  /** @inheritdoc */
  public set(value: T): Promise<void> {
    return Bridge.call('var.set', this.id, value);
  }
}

class RegisteredBooleanSimVar extends RegisteredSimVar<boolean> {
  public override get(): boolean {
    return Vars.readById(this.id) > 0;
  }

  public override set(value: boolean): Promise<void> {
    return Bridge.call('var.set', this.id, value ? 1 : 0);
  }
}

class RegisteredStringSimVar extends RegisteredSimVar<string> {
  public override get(): string {
    return Vars.readTextById(this.id);
  }

  public override set(value: string): Promise<void> {
    return Bridge.call('var.set.text', this.id, value);
  }
}
