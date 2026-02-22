const APPZ_TOOLBAR_COOKIE = '__appz_toolbar';
const APPZ_DEV_SUFFIX = '.appz.dev';
const APPZ_PREVIEW_SUFFIX = '.preview.appz.dev';

function isAppzDomain(host: string): boolean {
  return (
    host === 'appz.dev' ||
    host.endsWith(APPZ_DEV_SUFFIX) ||
    host.endsWith(APPZ_PREVIEW_SUFFIX)
  );
}

function shouldShowToolbar(host: string): boolean {
  if (!isAppzDomain(host)) return false;
  if (host === 'appz.dev') return false; // Dashboard only needs meta, no toolbar
  const cookie = document.cookie
    .split('; ')
    .find((c) => c.startsWith(`${APPZ_TOOLBAR_COOKIE}=`));
  if (cookie) return cookie.split('=')[1] === '1';
  return true; // Show on *.appz.dev by default
}

export default defineContentScript({
  matches: ['<all_urls>'],
  runAt: 'document_start',
  main() {
    const host = window.location.hostname;
    if (!shouldShowToolbar(host)) return;
    document.documentElement.setAttribute('data-appz-toolbar', '1');
    window.postMessage({ type: 'appz-toolbar-inject' }, '*');
  },
});
