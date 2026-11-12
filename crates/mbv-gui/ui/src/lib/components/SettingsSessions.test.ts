// add-gui-settings-screen row 8.5: the Sessions section's pointer path (the
// spec's *Every Settings control works by pointer*, design D11). A press on
// a playback target sends `selectTarget` with the target's stable key — the
// chooser's own path — and moves the content cursor onto the pressed row.
// The Tauri IPC is mocked; the asserted payload is the serde wire shape of
// bridge/settings.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiSettingsCommand, GuiSettingsContent } from '$lib/bridge/types';
import { settingsEntries } from '$lib/settings/entries';
import { cursorEntryId, initialSettingsUi, settingsUi } from '$lib/settings/state';
import SettingsSessions from './SettingsSessions.svelte';

const invokeMock = vi.mocked(invoke);

/** A Sessions content: the local output and one Emby session. */
function sessionsContent(): Extract<GuiSettingsContent, { kind: 'sessions' }> {
	return {
		kind: 'sessions',
		targets: [
			{ key: { kind: 'local' }, label: 'This machine', isActive: true },
			{ key: { kind: 'emby', id: 'sess-1' }, label: 'Bedroom', isActive: false },
		],
	};
}

beforeEach(() => {
	invokeMock.mockClear();
	settingsUi.set(initialSettingsUi());
});

test.each<[string, GuiSettingsCommand]>([
	['target:0', { command: 'selectTarget', target: { kind: 'local' } }],
	['target:1', { command: 'selectTarget', target: { kind: 'emby', id: 'sess-1' } }],
])(
	'a press on %s sends its command and moves the content cursor to it',
	async (id, command) => {
		const content = sessionsContent();
		const entries = settingsEntries(content, {});
		const { container } = render(SettingsSessions, {
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
