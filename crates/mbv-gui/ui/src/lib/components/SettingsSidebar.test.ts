// add-gui-settings-screen row 8.5: the sidebar's pointer path (the spec's
// *Every Settings control works by pointer*, design D11). A press on a
// section entry sends `selectSection`, moves the sidebar cursor onto the
// entry, and hands the keyboard to the content — the same effect Enter on
// the cursor row has. The Tauri IPC is mocked; the asserted payload is the
// serde wire shape of bridge/settings.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiSettingsFrame, GuiSettingsSection } from '$lib/bridge/types';
import { initialSettingsUi, settingsUi } from '$lib/settings/state';
import SettingsSidebar from './SettingsSidebar.svelte';

const invokeMock = vi.mocked(invoke);

/** A settings frame over the Services section. */
function frame(): GuiSettingsFrame {
	return {
		section: 'services',
		sections: ['services', 'feeds', 'keys'],
		content: { kind: 'rows', rows: [] },
	};
}

beforeEach(() => {
	invokeMock.mockClear();
	settingsUi.set(initialSettingsUi());
});

test.each<[GuiSettingsSection, number]>([
	['services', 0],
	['feeds', 1],
	['keys', 2],
])(
	'a press on the %s entry sends selectSection and moves the sidebar cursor to it',
	async (section, index) => {
		const { container } = render(SettingsSidebar, { props: { settings: frame() } });

		await fireEvent.click(
			container.querySelector(`[data-settings-section="${section}"]`) as HTMLElement,
		);

		expect(invokeMock).toHaveBeenCalledWith('settings', {
			command: { command: 'selectSection', section },
		});
		const ui = get(settingsUi);
		expect(ui.sidebarCursor).toBe(index);
		expect(ui.pane).toBe('content');
	},
);
