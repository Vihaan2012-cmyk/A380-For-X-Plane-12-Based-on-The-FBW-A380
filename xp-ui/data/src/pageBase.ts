/** Base styles every glass display page gets before its own stylesheets. */
export const PAGE_BASE_CSS = [
  'html{color:#fff}',
  'html,body{margin:0;padding:0;overflow:hidden}',
  '*,*::before,*::after{box-sizing:border-box;min-height:0}',
  '::-webkit-scrollbar{display:none}',
  '.hidden{display:none!important}',
].join('');

export function installPageBase(): void {
  const doc = (globalThis as { document?: Document }).document;
  if (!doc || doc.getElementById('xplane-js-page-base')) {
    return;
  }
  const style = doc.createElement('style');
  style.id = 'xplane-js-page-base';
  style.textContent = PAGE_BASE_CSS;
  const parent = doc.head ?? doc.documentElement;
  if (parent) {
    parent.insertBefore(style, parent.firstChild);
  }
}
