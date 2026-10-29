import {
  ViewProps,
  View,
  UI,
  Derived,
  Signal,
  Bond,
  ViewNode,
} from 'xplane-js/ui';

import { IconButton } from '../../../../MsfsAvionicsCommon/UiWidgets/IconButton';
import { Button } from '../../../../MsfsAvionicsCommon/UiWidgets/Button';
import { ActivePageTitleBar } from '../../common/ActivePageTitleBar';
import { Coordinates, distanceTo } from 'msfs-geo';
import { FmcServiceInterface } from '../../../FMC/FmcServiceInterface';
import { DatabaseItem, isFix, isIlsNavaid, isNdbNavaid, isVhfNavaid } from '@flybywiresim/fbw-sdk';

import './MfdFmsFpln.scss';

interface MfdFmsFplnDuplicateNamesProps extends ViewProps {
  visible: Signal<boolean>;
  fmcService: FmcServiceInterface;
}

type DuplicateWaypointData = {
  ident: string;
  /** in nautical miles */
  distance: number | null;
  location: Coordinates;
  /** In MHz */
  freqChan?: number;
  fixData: DatabaseItem<any>;
};

export class MfdFmsFplnDuplicateNames extends View<MfdFmsFplnDuplicateNamesProps> {
  // Make sure to collect all subscriptions here, otherwise page navigation doesn't work.
  private subs = [] as Bond[];

  private readonly eoActive = Signal.of<boolean>(false);

  private topRef = UI.ref<HTMLDivElement>();

  private linesDivRef = UI.ref<HTMLDivElement>();

  private returnButtonRef = UI.ref<Button>();

  private duplicateOptions = Signal.of<DuplicateWaypointData[]>([]);

  private displayFromWaypointIndex = Signal.of<number>(0);

  /** A function to resolve the currently pending dedpulication request, or undefined if no request is pending. */
  private pendingResolve?: (value: unknown | PromiseLike<unknown>) => void;
  /** A function to reject the currently pending dedpulication request, or undefined if no request is pending. */
  private pendingReject?: (reason?: any) => void;

  private disabledScrollDown = Derived.from(
    ([options, fromIndex]) => !(options.length > fromIndex + 10),
    this.duplicateOptions,
    this.displayFromWaypointIndex,
  );

  private disabledScrollUp = Derived.from(
    ([, fromIndex]) => !(fromIndex > 0),
    this.duplicateOptions,
    this.displayFromWaypointIndex,
  );

  public afterRender(node: ViewNode): void {
    super.afterRender(node);

    this.subs.push(this.displayFromWaypointIndex.watch((idx) => this.update(idx)));
    this.subs.push(
      this.props.visible.watch((vis) => {
        if (this.topRef.peek()) {
          this.topRef.target.style.display = vis ? 'block' : 'none';
        }
      }, true),
    );
    this.subs.push(
      this.props.fmcService.masterFmcChanged.watch(() =>
        this.props.fmcService.master.fmgc.data.engineOut.forward(this.eoActive),
      ),
    );

    this.update(0);
  }

  private async loadIdent<T extends DatabaseItem<any>>(items: T[]) {
    if (!items) {
      return;
    }
    const result: DuplicateWaypointData[] = [];
    items.forEach((fx) => {
      const ppos = this.props.fmcService?.master?.navigation.getPpos();
      if (isVhfNavaid(fx) || isNdbNavaid(fx) || isIlsNavaid(fx)) {
        const location = isIlsNavaid(fx) ? fx.locLocation : fx.location;
        const dwd: DuplicateWaypointData = {
          ident: fx.ident,
          distance: ppos ? distanceTo(location, ppos) : null,
          location,
          freqChan: fx.frequency,
          fixData: fx,
        };
        result.push(dwd);
      } else if (isFix(fx)) {
        const dwd: DuplicateWaypointData = {
          ident: fx.ident,
          distance: ppos ? distanceTo(fx.location, ppos) : null,
          location: fx.location,
          freqChan: undefined,
          fixData: fx,
        };
        result.push(dwd);
      }
    });

    // Sort by ascending distance and set data
    this.duplicateOptions.set(result.sort((a, b) => (a.distance ?? Infinity) - (b.distance ?? Infinity)));

    this.update(0);
  }

  private update(startAtIndex: number): void {
    while (this.linesDivRef.target.firstChild) {
      this.linesDivRef.target.removeChild(this.linesDivRef.target.firstChild);
    }

    for (let i = startAtIndex; i < startAtIndex + 10; i++) {
      if (this.duplicateOptions.get()[i] !== undefined) {
        const fix = this.duplicateOptions.get()[i];
        const latLonString = `${Math.round(Math.abs(fix.location.lat)).toFixed(0).padStart(2, '0')}${fix.location.lat > 0 ? 'N' : 'S'}/${Math.round(Math.abs(fix.location.long)).toFixed(0).padStart(3, '0')}${fix.location.long > 0 ? 'E' : 'W'}`;
        const node: ViewNode = (
          <div class="mfd-fms-fpln-duplicate-table-row" id={`mfd-fms-dupl-${i}`}>
            <div style="width: 20%">
              <span class="mfd-value bigger">{fix.ident ?? '\u00A0'}</span>
            </div>
            <div style="width: 20%">
              <span class="mfd-value bigger">{fix.distance ? Math.round(fix.distance).toFixed(0) : '\u00A0'}</span>
            </div>
            <div style="width: 30%">
              <span class="mfd-value bigger">{latLonString ?? '\u00A0'}</span>
            </div>
            <div style="width: 30%">
              <span class="mfd-value bigger">
                {fix.freqChan && fix.freqChan > 120 ? fix.freqChan.toFixed(0) : fix.freqChan?.toFixed(2) ?? '\u00A0'}
              </span>
            </div>
          </div>
        );
        UI.mount(node, this.linesDivRef.target);

        document.getElementById(`mfd-fms-dupl-${i}`)?.addEventListener('click', this.itemClickedHandler.bind(this, i));
      }
    }
  }

  private itemClickedHandler(i: number) {
    this.pendingResolve?.(this.duplicateOptions.get()[i].fixData);
    this.pendingResolve = undefined;
    this.pendingReject = undefined;
    this.props.visible.set(false);
  }

  // Entry point after opening this dialog
  public deduplicateFacilities<T extends DatabaseItem<any>>(items: T[]): Promise<T> {
    if (this.pendingReject !== undefined) {
      this.pendingReject('A new request overrode this one!');
    }

    return new Promise((resolve, reject) => {
      this.pendingResolve = resolve as any;
      this.pendingReject = reject;

      this.loadIdent(items);
      this.props.visible.set(true);
    });
  }

  public destroy(): void {
    // Destroy all subscriptions to remove all references to this instance.
    this.subs.forEach((x) => x.destroy());

    for (let i = 0; i < this.duplicateOptions.get().length; i++) {
      if (this.duplicateOptions.get()[i] !== undefined) {
        document
          .getElementById(`mfd-fms-dupl-${i}`)
          ?.removeEventListener('click', this.itemClickedHandler.bind(this, i));
      }
    }

    super.destroy();
  }

  render(): ViewNode {
    return (
      <div ref={this.topRef} class="mfd-fms-fpln-dialog-outer">
        <div class="mfd-fms-fpln-dialog-inner">
          <ActivePageTitleBar
            activePage={Signal.of('DUPLICATE NAMES')}
            offset={Signal.of('')}
            eoIsActive={this.eoActive}
            isFmsSubsystemPage={true}
          />
          {/* begin page content */}
          <div class="mfd-fms-fpln-duplicate-table" style="margin-top: 100px;">
            <div class="mfd-fms-fpln-duplicate-table-header">
              <div style="width: 20%">
                <span class="mfd-label">IDENT</span>
              </div>
              <div style="width: 20%">
                <span class="mfd-label">DIST(NM)</span>
              </div>
              <div style="width: 30%">
                <span class="mfd-label">LAT/LONG</span>
              </div>
              <div style="width: 30%">
                <span class="mfd-label">FREQ/CHAN</span>
              </div>
            </div>
          </div>
          <div class="mfd-fms-fpln-duplicate-outline mfd-fms-fpln-label-bottom-space">
            <div ref={this.linesDivRef} class="mfd-fms-fpln-duplicate-table" />
            <div style="flex-grow: 1;" />
          </div>
          <div style="display: flex; flex-direction: row; justify-content: center">
            <IconButton
              icon="double-down"
              onClick={() => this.displayFromWaypointIndex.set(this.displayFromWaypointIndex.get() + 1)}
              disabled={this.disabledScrollDown}
              containerStyle="width: 60px; height: 60px; margin-right: 20px;"
            />
            <IconButton
              icon="double-up"
              onClick={() => this.displayFromWaypointIndex.set(this.displayFromWaypointIndex.get() - 1)}
              disabled={this.disabledScrollUp}
              containerStyle="width: 60px; height: 60px;"
            />
          </div>
          <div style="flex-grow: 1;" />
          <div style="display: flex; flex-direction: row; justify-content: space-between;">
            <div style="display: flex; justify-content: flex-end; padding: 2px;">
              <Button
                ref={this.returnButtonRef}
                label="RETURN"
                onClick={() => {
                  this.pendingResolve?.(undefined);
                  this.pendingResolve = undefined;
                  this.pendingReject = undefined;
                  this.props.visible.set(false);
                }}
              />
            </div>
          </div>
          {/* end page content */}
        </div>
      </div>
    );
  }
}
