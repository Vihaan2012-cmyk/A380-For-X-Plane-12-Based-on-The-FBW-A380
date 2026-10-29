import { View, UI, VarUnit, ViewNode } from 'xplane-js/ui';
import { IconButton } from '../../../../MsfsAvionicsCommon/UiWidgets/IconButton';
import { NXDataStore } from '@flybywiresim/fbw-sdk';
import { Vars } from 'xplane-js/data';

interface SdSoftKeyProps {}

export class SdSoftKey extends View<SdSoftKeyProps> {
  private readonly sdSoftKeyRef = UI.ref<HTMLDivElement>();

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    NXDataStore.getAndSubscribeLegacy(
      'CONFIG_A380X_SHOW_ECL_SOFTKEYS',
      (_, v) => {
        if (this.sdSoftKeyRef.peek()) {
          this.sdSoftKeyRef.target.style.display = v === '1' ? 'flex' : 'none';
        }
      },
      '0',
    );
  }

  render() {
    return (
      <div class="sd-sts-soft-key" ref={this.sdSoftKeyRef}>
        <IconButton
          icon={'double-right'}
          containerStyle="width: 50px; height: 50px;"
          onClick={() => {
            Vars.write('H:A32NX_SD_STS_NEXT_PAGE', VarUnit.Number, 1);
          }}
        />
      </div>
    );
  }
}
