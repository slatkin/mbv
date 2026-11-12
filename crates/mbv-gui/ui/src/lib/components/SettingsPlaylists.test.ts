// add-gui-settings-screen row 8.5: the Playlists section's pointer path (the
// spec's *Every Settings control works by pointer*, design D11). A press on
// a playlist row or action sends the same command the keyboard's activation
// sends and moves the content cursor onto the pressed control; a press on
// Rename opens the draft in place, whose Enter then sends `playlistRename`.
// The Tauri IPC is mocked; the asserted payload is the serde wire shape of
// bridge/settings.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import type {
	GuiSettingsCommand,
	GuiSettingsContent,
	GuiSettingsFrame,
} from '$lib/bridge/types';
import { settingsEntries } from '$lib/settings/entries';
import { routeSettingsKey } from '$lib/settings/keys';
import { cursorEntryId, initialSettingsUi, settingsUi } from '$lib/settings/state';
import SettingsPlaylists from './SettingsPlaylists.svelte';

const invokeMock = vi.mocked(invoke);

/** The playlist list's content. */
function listContent(): Extract<GuiSettingsContent, { kind: 'playlists' }> {
	return {
		kind: 'playlists',
		playlists: [{ id: 'p1', name: 'Mix', itemCount: 2 }],
		open: null,
		loading: false,
		openLoading: false,
	};
}

/** The open playlist's content. */
function openContent(): Extract<GuiSettingsContent, { kind: 'playlists' }> {
	return {
		kind: 'playlists',
		playlists: [],
		open: { id: 'p1', name: 'Mix', items: [{ id: 'i1', name: 'Song' }] },
		loading: false,
		openLoading: false,
	};
}

/** A settings frame over the Playlists section. */
function playlistsFrame(content: Extract<GuiSettingsContent, { kind: 'playlists' }>): GuiSettingsFrame {
	return { section: 'playlists', sections: ['playlists'], content };
}

beforeEach(() => {
	invokeMock.mockClear();
	settingsUi.set(initialSettingsUi());
});

test.each<[string, GuiSettingsCommand]>([
	['playlistRefresh:list', { command: 'playlistRefresh', id: null }],
	['playlist:p1', { command: 'playlistOpen', id: 'p1' }],
	['playlistAction:p1:play', { command: 'playlistAction', id: 'p1', action: 'play' }],
	['playlistAction:p1:shuffle', { command: 'playlistAction', id: 'p1', action: 'shuffle' }],
	['playlistAction:p1:enqueue', { command: 'playlistAction', id: 'p1', action: 'enqueue' }],
	['playlistDelete:p1', { command: 'playlistDelete', id: 'p1' }],
])(
	'a press on the list\'s %s sends its command and moves the content cursor to it',
	async (id, command) => {
		const content = listContent();
		const entries = settingsEntries(content, {});
		const { container } = render(SettingsPlaylists, {
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

test.each<[string, GuiSettingsCommand]>([
	['playlistBack', { command: 'playlistBack' }],
	['playlistRefresh:p1', { command: 'playlistRefresh', id: 'p1' }],
	[
		'item:p1:i1:enqueue',
		{ command: 'playlistItemAction', id: 'p1', itemId: 'i1', action: 'enqueue' },
	],
])(
	'a press on the open playlist\'s %s sends its command and moves the content cursor to it',
	async (id, command) => {
		const content = openContent();
		const entries = settingsEntries(content, {});
		const { container } = render(SettingsPlaylists, {
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

test('a press on Rename opens the draft, and the field\'s Enter sends playlistRename', async () => {
	const content = listContent();
	const entries = settingsEntries(content, {});
	const { container } = render(SettingsPlaylists, {
		props: { content, entries, cursorId: null },
	});

	const id = 'playlistRename:p1';
	await fireEvent.click(
		container.querySelector(`[data-settings-entry="${id}"]`) as HTMLElement,
	);

	expect(invokeMock).not.toHaveBeenCalled();
	expect(get(settingsUi).rename).toEqual({ id: 'p1', name: 'Mix' });
	expect(cursorEntryId(entries, get(settingsUi))).toBe(id);
	await tick();

	const field = container.querySelector('[data-settings-field="rename"]') as HTMLElement;
	await fireEvent.focusIn(field);
	await fireEvent.input(field, { target: { value: 'New mix' } });
	routeSettingsKey(new KeyboardEvent('keydown', { key: 'Enter' }), playlistsFrame(content));

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: { command: 'playlistRename', id: 'p1', name: 'New mix' },
	});
});
