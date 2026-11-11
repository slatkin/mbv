// add-gui-settings-screen row 6.6: the Keys section's refusal line and
// reset control (design D9). The frame's refused edit paints its text on the
// edited row, and a reset sends `keysReset` with the action's id. The Tauri
// IPC is mocked; the asserted payload is the serde wire shape of
// bridge/settings.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiSettingsCommand, GuiSettingsContent } from '$lib/bridge/types';
import { settingsEntries } from '$lib/settings/entries';
import { cursorEntryId, initialSettingsUi, settingsUi } from '$lib/settings/state';
import SettingsKeys from './SettingsKeys.svelte';

const invokeMock = vi.mocked(invoke);

/** The Keys content: one changed action carrying the frame's refusal, and
 * one unchanged action. */
function keysContent(): Extract<GuiSettingsContent, { kind: 'keys' }> {
	return {
		kind: 'keys',
		prefix: 'ctrl+p',
		rows: [
			{
				id: 'stop',
				label: 'Stop',
				section: 'Playback',
				chords: ['x'],
				prefixChord: 'p',
				changed: true,
				prefixAddressable: true,
			},
			{
				id: 'toggle_play_pause',
				label: 'Play or pause',
				section: 'Playback',
				chords: [' '],
				prefixChord: null,
				changed: false,
				prefixAddressable: false,
			},
		],
		refusal: { action: 'stop', text: 'the chord collides with `stop`' },
	};
}

beforeEach(() => {
	invokeMock.mockClear();
	settingsUi.set(initialSettingsUi());
});

test('the refusal text from the frame shows on the edited row', () => {
	const content = keysContent();
	const { container } = render(SettingsKeys, {
		props: { content, entries: settingsEntries(content, {}), cursorId: null },
	});

	const row = container.querySelector('[data-settings-key-row="stop"]') as HTMLElement;
	expect(row.nextElementSibling?.getAttribute('data-settings-key-refusal')).not.toBeNull();
	expect(row.nextElementSibling?.textContent).toContain(
		'the chord collides with `stop`',
	);
	// The unchanged row paints no refusal, and the section paints one.
	expect(
		container.querySelectorAll('[data-settings-key-refusal]'),
	).toHaveLength(1);
});

test('a reset sends keysReset with the action id', async () => {
	const content = keysContent();
	const { container } = render(SettingsKeys, {
		props: { content, entries: settingsEntries(content, {}), cursorId: null },
	});

	const row = container.querySelector('[data-settings-key-row="stop"]') as HTMLElement;
	await fireEvent.click(row.querySelector('[aria-label^="Reset"]') as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: { command: 'keysReset', action: 'stop' },
	});
});

test.each<[string, GuiSettingsCommand]>([
	['keyClear:prefix', { command: 'keysClear', target: 'prefix' }],
	['keyClear:prefix:stop', { command: 'keysClear', target: 'prefix:stop' }],
])('a press on %s sends its keysClear and moves the content cursor to it', async (id, command) => {
	const content = keysContent();
	const entries = settingsEntries(content, {});
	const { container } = render(SettingsKeys, {
		props: { content, entries, cursorId: null },
	});

	await fireEvent.click(
		container.querySelector(`[data-settings-entry="${id}"]`) as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('settings', { command });
	const ui = get(settingsUi);
	expect(ui.pane).toBe('content');
	expect(cursorEntryId(entries, ui)).toBe(id);
});

test('a press on a chord arms its capture and sends no command', async () => {
	const content = keysContent();
	const entries = settingsEntries(content, {});
	const { container } = render(SettingsKeys, {
		props: { content, entries, cursorId: null },
	});

	const id = 'keyTarget:stop';
	await fireEvent.click(
		container.querySelector(`[data-settings-entry="${id}"]`) as HTMLElement,
	);

	expect(invokeMock).not.toHaveBeenCalled();
	const ui = get(settingsUi);
	expect(ui.capture).toEqual({ target: 'stop', sent: false });
	expect(ui.pane).toBe('content');
	expect(cursorEntryId(entries, ui)).toBe(id);
});
