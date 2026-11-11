// add-gui-settings-screen row 6.5: the Feeds section's edit (design D9). An
// edit sends `feedEdit` with the subscription's URL as its identity — the
// config list has no other — so the command addresses the subscription the
// frame painted even if the list moved since. The Tauri IPC is mocked; the
// asserted payload is the serde wire shape of bridge/settings.rs.

import { invoke } from '@tauri-apps/api/core';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiSettingsContent } from '$lib/bridge/types';
import {
	focusSettingsField,
	initialSettingsUi,
	openFeedForm,
	setFeedForm,
	settingsUi,
	submitSettingsForm,
} from './state';

const invokeMock = vi.mocked(invoke);

/** The Feeds section's content around the form. */
function feedsContent(): GuiSettingsContent {
	return {
		kind: 'feeds',
		subscriptions: [{ name: 'Sub One', url: 'https://sub.one/feed', kind: 'audio' }],
		adding: false,
	};
}

beforeEach(() => {
	invokeMock.mockClear();
	settingsUi.set(initialSettingsUi());
});

test('an edit sends feedEdit with the subscription URL as identity', () => {
	openFeedForm({ name: 'Sub One', url: 'https://sub.one/feed', kind: 'audio' });
	setFeedForm({ name: 'Renamed', kind: 'video' });
	focusSettingsField('feed');

	submitSettingsForm(feedsContent());

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: {
			command: 'feedEdit',
			url: 'https://sub.one/feed',
			name: 'Renamed',
			kind: 'video',
		},
	});
});
