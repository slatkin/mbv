// add-gui-settings-screen rows 8.4 and 8.5: the Library section's pointer
// path (the spec's *Every Settings control works by pointer* and *A device
// with no endpoint*, design D11). A press on a routable library sends
// `routeDevices`, a press on the Automatic entry or a routable device sends
// `assignRoute`, and a press on a device with no endpoint — a disabled row —
// sends nothing. The Tauri IPC is mocked; the asserted payload is the serde
// wire shape of bridge/settings.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiSettingsCommand, GuiSettingsContent } from '$lib/bridge/types';
import { settingsEntries } from '$lib/settings/entries';
import { cursorEntryId, initialSettingsUi, settingsUi } from '$lib/settings/state';
import SettingsLibrary from './SettingsLibrary.svelte';

const invokeMock = vi.mocked(invoke);

/** A Library content: two routable libraries, `music` selected with one
 * routable device and one device with no endpoint. */
function libraryContent(): Extract<GuiSettingsContent, { kind: 'library' }> {
	return {
		kind: 'library',
		rows: [],
		routes: [
			{ library: 'music', name: 'Music', endpoint: null },
			{ library: 'shows', name: 'Shows', endpoint: 'dlna:tv' },
		],
		selected: 'music',
		devices: [
			{ name: 'Phone', endpoint: 'dlna:phone' },
			{ name: 'Ghost', endpoint: null },
		],
	};
}

beforeEach(() => {
	invokeMock.mockClear();
	settingsUi.set(initialSettingsUi());
});

test.each<[string, GuiSettingsCommand]>([
	['route:shows', { command: 'routeDevices', library: 'shows' }],
	['routeClear:music', { command: 'assignRoute', library: 'music', endpoint: null }],
	[
		'routeDevice:music:Phone',
		{ command: 'assignRoute', library: 'music', endpoint: 'dlna:phone' },
	],
])(
	'a press on %s sends its command and moves the content cursor to it',
	async (id, command) => {
		const content = libraryContent();
		const entries = settingsEntries(content, {});
		const { container } = render(SettingsLibrary, {
			props: { content, entries, cursorId: null },
		});

		await fireEvent.click(
			container.querySelector(`[data-settings-entry="${id}"]`) as HTMLElement,
		);

		expect(invokeMock).toHaveBeenCalledWith('settings', { command });
		const ui = get(settingsUi);
		expect(ui.pane).toBe('content');
		expect(cursorEntryId(entries, ui)).toBe(id);
	},
);

test('a press on a device with no endpoint sends no command', async () => {
	const content = libraryContent();
	const entries = settingsEntries(content, {});
	const { container } = render(SettingsLibrary, {
		props: { content, entries, cursorId: null },
	});

	const row = container.querySelector(
		'[data-settings-entry="routeDevice:music:Ghost"]',
	) as HTMLButtonElement;
	expect(row.disabled).toBe(true);
	await fireEvent.click(row);

	expect(invokeMock).not.toHaveBeenCalled();
});
