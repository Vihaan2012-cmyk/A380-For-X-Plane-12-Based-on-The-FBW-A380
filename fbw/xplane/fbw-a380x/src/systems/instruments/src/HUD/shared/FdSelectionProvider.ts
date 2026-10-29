import { Arinc429LocalVarConsumerSubject } from '@flybywiresim/fbw-sdk';
import { BusSignal, Bus, BoardDevice, Derived, Signal } from 'xplane-js/ui';
import { PrimFgBusBaseEvents } from '@shared/publishers/PrimFgPublisher';
import { getDisplayIndex } from '../HUD';

export interface SelectedFdEvents {
  fd_engaged: boolean;
  // The roll FD command for the selected FD, in deg.
  prim_roll_fd_command: number;
  // The pitch FD command for selected FD, in deg.
  prim_pitch_fd_command: number;
  // The yaw FD command for selected FD, in deg.
  prim_yaw_fd_command: number;
}

export class FdSelectionProvider implements BoardDevice {
  private readonly sub = this.bus.inbox<PrimFgBusBaseEvents>();

  private readonly primFgDiscreteWord1 = Arinc429LocalVarConsumerSubject.create(this.sub.topic('prim_fg_discrete_word_1'));

  // We have to initialize this in init, as otherwise the content will not be ready and the getDisplayIndex method will fail
  private isSide2 = Signal.of(false);

  private readonly fd1Selected = Derived.from(
    ([isSide2, primFgDiscreteWord1]) => {
      const fd1Engaged = primFgDiscreteWord1.bitValueOr(13, false);
      const fd2Engaged = primFgDiscreteWord1.bitValueOr(14, false);

      return (!isSide2 && !fd1Engaged && !fd2Engaged) || (!isSide2 && fd1Engaged) || (isSide2 && !fd2Engaged);
    },
    this.isSide2,
    this.primFgDiscreteWord1,
  );

  private readonly fdEngaged = Derived.from(
    ([fd1Selected, primFgDiscreteWord1]) => {
      const fd1Engaged = primFgDiscreteWord1.bitValueOr(13, false);
      const fd2Engaged = primFgDiscreteWord1.bitValueOr(14, false);

      return fd1Selected ? fd1Engaged : fd2Engaged;
    },
    this.fd1Selected,
    this.primFgDiscreteWord1,
  );

  private readonly rollFdCommand = BusSignal.from(null, 0);

  private readonly pitchFdCommand = BusSignal.from(null, 0);

  private readonly yawFdCommand = BusSignal.from(null, 0);

  constructor(private readonly bus: Bus) {}

  /** @inheritdoc */
  public setup(): void {
    const publisher = this.bus.outbox<SelectedFdEvents>();

    this.isSide2.set(getDisplayIndex() === 2);

    this.fd1Selected.watch((fd1Selected) => {
      this.rollFdCommand.rebind(this.sub.topic(`prim_roll_fd_command_${fd1Selected ? 1 : 2}`));
      this.pitchFdCommand.rebind(this.sub.topic(`prim_pitch_fd_command_${fd1Selected ? 1 : 2}`));
      this.yawFdCommand.rebind(this.sub.topic(`prim_yaw_fd_command_${fd1Selected ? 1 : 2}`));
    }, true);

    this.rollFdCommand.watch((rollFdCommand) => {
      publisher.post('prim_roll_fd_command', rollFdCommand);
    }, true);

    this.pitchFdCommand.watch((pitchFdCommand) => {
      publisher.post('prim_pitch_fd_command', pitchFdCommand);
    }, true);

    this.yawFdCommand.watch((yawFdCommand) => {
      publisher.post('prim_yaw_fd_command', yawFdCommand);
    }, true);

    this.fdEngaged.watch((fdEngaged) => {
      publisher.post('fd_engaged', fdEngaged);
    }, true);
  }

  /** @inheritdoc */
  public tick(): void {
    // noop
  }
}
