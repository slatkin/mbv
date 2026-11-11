// add-gui-settings-screen row 9.3: the shared Settings switch (design D13, the
// canvas switch). The control carries `role="switch"` and `aria-checked` for
// assistive technology, and a press calls the handler the keyboard's Space
// calls, so both paths share one effect (design D11's pointer contract).

import { fireEvent, render } from '@testing-library/svelte';
import { expect, test, vi } from 'vitest';
import SettingsSwitch from './SettingsSwitch.svelte';

test.each<[boolean, string]>([
	[true, 'true'],
	[false, 'false'],
])('a switch with checked=%s carries role="switch" and aria-checked=%s', (checked, expected) => {
	const { container } = render(SettingsSwitch, {
		props: { label: 'Use mpv config', checked, onpress: () => {} },
	});

	const control = container.querySelector('[role="switch"]') as HTMLElement;
	expect(control.getAttribute('aria-label')).toBe('Use mpv config');
	expect(control.getAttribute('aria-checked')).toBe(expected);
});

test('a press on the switch calls its press handler', async () => {
	const onpress = vi.fn();
	const { container } = render(SettingsSwitch, {
		props: { label: 'Use mpv config', checked: false, onpress },
	});

	await fireEvent.click(container.querySelector('[role="switch"]') as HTMLElement);

	expect(onpress).toHaveBeenCalledTimes(1);
});
