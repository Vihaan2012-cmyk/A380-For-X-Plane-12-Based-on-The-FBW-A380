// @ts-strict-ignore
import React from 'react';
import { getRootElement } from '@instruments/common/defaults';
import ReactDOM from 'react-dom';
import { renderTarget } from '../util.js';
import { render } from '../Common';
import { OitEfbWrapper } from './OitLegacy.js';
import { Bus } from 'xplane-js/ui';

if (renderTarget) {
  render(<OitEfbWrapper eventBus={new Bus()} />);
}

getRootElement().addEventListener('unload', () => {
  ReactDOM.unmountComponentAtNode(renderTarget ?? document.body);
});
