import { render } from '@testing-library/svelte';
import { expect, test } from 'vitest';
import App from './App.svelte';

// add-tauri-gui-client D11: the webview's own context menu is turned off.
test('prevents the default context menu', () => {
  render(App);
  const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });

  document.body.dispatchEvent(event);

  expect(event.defaultPrevented).toBe(true);
});
