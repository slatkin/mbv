// add-gui-settings-screen row 6.7: the Playlists section's rename and item
// actions (design D9). The rename field sends `playlistRename` with the
// playlist's id and the typed name, and an item's add sends
// `playlistItemAction` with both ids. The Tauri IPC is mocked; the asserted
// payloads are the serde wire shapes of bridge/settings.rs.

import { invoke } from '@tauri-apps/api/core';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiSettingsContent } from '$lib/bridge/types';
import { entryId, settingsEntries } from './entries';
import {
	focusSettingsField,
	initialSettingsUi,
	openRenameDraft,
	pressSettingsEntry,
	setRenameDraft,
	settingsUi,
	submitSettingsForm,
} from './state';

const invokeMock = vi.mocked(invoke);

/** The Playlists section's content with one open playlist. */
function playlistsContent(): GuiSettingsContent {
	return {
		kind: 'playlists',
		playlists: [{ id: 'pl1', name: 'Mix', itemCount: 2 }],
		open: {
			id: 'pl1',
			name: 'Mix',
			items: [
				{ id: 'i1', name: 'Song A' },
				{ id: 'i2', name: 'Song B' },
			],
		},
		loading: false,
		openLoading: false,
	};
}

beforeEach(() => {
	invokeMock.mockClear();
	settingsUi.set(initialSettingsUi());
});

test('a rename sends playlistRename with the id and the typed name', () => {
	openRenameDraft('pl1', 'Mix');
	setRenameDraft('Road mix');
	focusSettingsField('rename');

	submitSettingsForm(playlistsContent());

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: { command: 'playlistRename', id: 'pl1', name: 'Road mix' },
	});
});

test("an item's add sends playlistItemAction with both ids", () => {
	const content = playlistsContent();
	const entries = settingsEntries(content, {});

	pressSettingsEntry(entries, entryId.playlistItemAction('pl1', 'i2', 'enqueue'));

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: {
			command: 'playlistItemAction',
			id: 'pl1',
			itemId: 'i2',
			action: 'enqueue',
		},
	});
});
