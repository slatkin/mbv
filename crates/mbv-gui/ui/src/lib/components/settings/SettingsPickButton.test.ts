// add-gui-settings-screen row 9.4: the Settings pick button and its value menu
// (design D13, the canvas pick button). A press opens the row's values in the
// row menu's chrome, the Settings key router takes the menu's own keys, and
// Enter on an entry sends `setValue` for the row's setting and that value. Esc
// and a press outside close the menu with no send. The Tauri IPC is mocked; the
// asserted payload is the serde wire shape of bridge/settings.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiSettingsFrame } from '$lib/bridge/types';
import { routeSettingsKey } from '$lib/settings/keys';
import { pickMenu } from '$lib/settings/pick-menu';
import { initialSettingsUi, settingsUi } from '$lib/settings/state';
import SettingsPickButton from './SettingsPickButton.svelte';

const invokeMock = vi.mocked(invoke);

/** A Settings frame over the Playback section: the menu step runs before any
 * content fact, so the rows stay empty. */
function settings(): GuiSettingsFrame {
	return {
		section: 'playback',
		sections: ['playback'],
		content: { kind: 'rows', rows: [] },
	};
}

/** Route one keydown through the Settings key router. */
function press(key: string): string {
	return routeSettingsKey(new KeyboardEvent('keydown', { key }), settings());
}

/** The pick button over three values, the empty one painted as Any. */
function pick() {
	return render(SettingsPickButton, {
		props: {
			setting: 'subtitleLanguage',
			label: 'Subtitle language',
			value: 'English',
			values: ['', 'English', 'French'],
		},
	});
}

/** Open the pick's menu by pressing its button. */
async function open(container: HTMLElement): Promise<void> {
	await fireEvent.click(container.querySelector('[data-settings-pick] button') as HTMLElement);
}

beforeEach(() => {
	invokeMock.mockClear();
	settingsUi.set(initialSettingsUi());
	pickMenu.set(null);
});

test('Enter on a menu entry sends setValue with that value', async () => {
	const { container } = pick();
	await open(container);

	expect(press('ArrowDown')).toBe('handled');
	expect(press('Enter')).toBe('handled');
	await tick();

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: { command: 'setValue', setting: 'subtitleLanguage', value: 'French' },
	});
	expect(container.querySelector('[role="menu"]')).toBeNull();
});

test('Esc closes the menu and sends nothing', async () => {
	const { container } = pick();
	await open(container);

	expect(press('Escape')).toBe('handled');
	await tick();

	expect(invokeMock).not.toHaveBeenCalled();
	expect(container.querySelector('[role="menu"]')).toBeNull();
});

test('a press outside the menu closes it and sends nothing', async () => {
	const { container } = pick();
	await open(container);

	await fireEvent.pointerDown(document.body);

	expect(invokeMock).not.toHaveBeenCalled();
	expect(container.querySelector('[role="menu"]')).toBeNull();
});
