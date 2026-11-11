// add-gui-settings-screen: the Settings status line shows the armed prefix and
// the live toast together (the spec's *The Settings screen shows the toasts*).

import { render } from '@testing-library/svelte';
import { expect, test } from 'vitest';
import SettingsStatusBar from './SettingsStatusBar.svelte';

test('the status line shows the armed prefix and the live toast together', () => {
	const { container } = render(SettingsStatusBar, {
		props: {
			status: { prefixArmed: true, toast: { text: 'Setting refused', severity: 'warning' } },
			section: 'Playback',
		},
	});

	expect(container.querySelector('[data-settings-status-prefix]')?.textContent).toBe('PREFIX');
	expect(container.querySelector('[data-settings-status-toast]')?.textContent).toBe(
		'Setting refused',
	);
});
