import { View, UI, VarUnit, ViewNode } from 'xplane-js/ui';
import { IconButton } from '../../MsfsAvionicsCommon/UiWidgets/IconButton';
import { NXDataStore } from '@flybywiresim/fbw-sdk';
import { Vars } from 'xplane-js/data';

interface EclSoftKeysProps {}

export class EclSoftKeys extends View<EclSoftKeysProps> {
  private readonly eclSoftKeysRef = UI.ref<HTMLDivElement>();

  afterRender(node: ViewNode): void {
    super.afterRender(node);

    NXDataStore.getAndSubscribeLegacy(
      'CONFIG_A380X_SHOW_ECL_SOFTKEYS',
      (_, v) => {
        if (this.eclSoftKeysRef.peek()) {
          this.eclSoftKeysRef.target.style.display = v === '1' ? 'flex' : 'none';
        }
      },
      '0',
    );
  }

  render() {
    return (
      <div class="EclSoftKeys" ref={this.eclSoftKeysRef}>
        <IconButton
          icon={'ecl-check'}
          containerStyle="width: 50px; height: 50px; margin-bottom: 5px;"
          onClick={() => {
            Vars.write('L:A32NX_BTN_CHECK_LH', VarUnit.Number, 1);
            setTimeout(() => Vars.write('L:A32NX_BTN_CHECK_LH', VarUnit.Number, 0), 50);
          }}
        />
        <IconButton
          icon={'ecl-single-up'}
          containerStyle="width: 50px; height: 50px; margin-bottom: 5px;"
          onClick={() => {
            Vars.write('L:A32NX_BTN_UP', VarUnit.Number, 1);
            setTimeout(() => Vars.write('L:A32NX_BTN_UP', VarUnit.Number, 0), 50);
          }}
        />
        <IconButton
          icon={'ecl-single-down'}
          containerStyle="width: 50px; height: 50px;"
          onClick={() => {
            Vars.write('L:A32NX_BTN_DOWN', VarUnit.Number, 1);
            setTimeout(() => Vars.write('L:A32NX_BTN_DOWN', VarUnit.Number, 0), 50);
          }}
        />
      </div>
    );
  }
}
