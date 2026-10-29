import { ViewProps, View, UI, Signal, Bond, ViewNode } from 'xplane-js/ui';
import '../../common/style.scss';
import { Button } from '../../../../MsfsAvionicsCommon/UiWidgets/Button';
import { InputField } from '../../../../MsfsAvionicsCommon/UiWidgets/InputField';
import { AirportFormat } from '../../common/DataEntryFormats';
import { FmsDisplayInterface } from '@fmgc/flightplanning/interface/FmsDisplayInterface';
import { MfdDisplayInterface } from '../../../MFD';
import { FmcServiceInterface } from '../../../FMC/FmcServiceInterface';
import { FlightPlanInterface } from '@fmgc/flightplanning/FlightPlanInterface';
import { Bridge } from 'xplane-js/data';

interface DestinationWindowProps extends ViewProps {
  fmcService: FmcServiceInterface;
  flightPlanInterface: FlightPlanInterface;
  mfd: FmsDisplayInterface & MfdDisplayInterface;
  visible: Signal<boolean>;
}
export class DestinationWindow extends View<DestinationWindowProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  private readonly subs = [] as Bond[];

  private readonly topRef = UI.ref<HTMLDivElement>();

  private readonly identRef = UI.ref<HTMLSpanElement>();

  private readonly newDest = Signal.of<string | null>(null);

  private onModified(newDest: string | null): void {
    if (newDest) {
      const revWpt = this.props.fmcService.master.revisedLegIndex.get();
      if (newDest.length === 4 && revWpt) {
        this.props.flightPlanInterface.newDest(
          revWpt,
          newDest,
          this.props.fmcService.master.revisedLegPlanIndex.get() ?? undefined,
          this.props.fmcService.master.revisedLegIsAltn.get() ?? undefined,
        );
        this.props.fmcService.master.acInterface.updateFmsData();
      }
      this.props.visible.set(false);
      this.newDest.set('');
      this.props.fmcService.master.resetRevisedWaypoint();
    }
  }

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subs.push(
      this.props.visible.watch((val) => {
        if (this.topRef.peek()) {
          this.topRef.target.style.display = val ? 'block' : 'none';
          this.newDest.set('');
        }
      }, true),
    );

    if (this.props.fmcService.master) {
      this.subs.push(
        this.props.fmcService.master.revisedLegIndex.watch(() => {
          if (this.props.fmcService.master.revisedWaypoint()) {
            this.identRef.target.innerText = this.props.fmcService.master.revisedWaypoint()?.ident ?? '';
          }
        }),
      );
    }
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    this.subs.forEach((x) => x.destroy());

    super.destroy();
  }

  render(): ViewNode {
    return (
      <div ref={this.topRef} style="position: relative;">
        <div class="mfd-dialog mfd-fms-new-dest-box">
          <div class="mfd-fms-new-dest-box-inner">
            <span class="mfd-label">
              NEW DEST FROM{' '}
              <span ref={this.identRef} class="mfd-value bigger">
                {this.props.fmcService.master.revisedWaypoint()?.ident ?? ''}
              </span>
            </span>
            <div style="align-self: center; margin-top: 50px;">
              <InputField<string>
                dataEntryFormat={new AirportFormat()}
                dataHandlerDuringValidation={async (icao) => this.onModified(icao)}
                mandatory={Signal.of(false)}
                canBeCleared={Signal.of(true)}
                inactive={Signal.of(false)}
                value={this.newDest}
                alignText="center"
                errorHandler={(e) => this.props.mfd.showFmsErrorMessage(e.type)}
                hEventConsumer={this.props.mfd.hEventConsumer}
                interactionMode={this.props.mfd.interactionMode}
              />
            </div>
          </div>
          <div class="fr" style="justify-content: space-between">
            <Button
              label="CANCEL"
              onClick={() => {
                Bridge.signal('input.blur');
                this.props.fmcService.master.resetRevisedWaypoint();
                this.props.visible.set(false);
              }}
            />
          </div>
        </div>
      </div>
    );
  }
}
