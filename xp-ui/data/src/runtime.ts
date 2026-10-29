import { createData } from './create.ts';
import type { XplaneJsData } from './create.ts';
import { installErrorForwarding } from './errors.ts';
import { globalHost } from './host.ts';
import { installPageBase } from './pageBase.ts';
import { whenReady } from './panelHost.ts';

const g = globalThis as { xplaneJsData?: XplaneJsData; addEventListener?: (type: string, listener: (event: any) => void) => void };

if (!g.xplaneJsData) {
  g.xplaneJsData = createData();
  installPageBase();
  whenReady(() => {
    if (typeof g.addEventListener === 'function') {
      installErrorForwarding(g as { addEventListener(type: string, listener: (event: any) => void): void }, globalHost);
    }
  });
}

export {};
