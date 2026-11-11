// add-gui-settings-screen row 9.6: the content frame (design D11, D13). The
// canvas `main` opens on the shown section's own name, so a user who arrives
// by a sidebar press or by F2 reads which section they are on. The Tauri IPC
// is mocked, and the frame is the window's own prop, so no command runs.

import { render } from '@testing-library/svelte';
import { beforeEach, expect, test } from 'vitest';
import type { GuiSettingsFrame, GuiSettingsSection } from '$lib/bridge/types';
import { initialSettingsUi, settingsUi } from '$lib/settings/state';
import SettingsScreen from './SettingsScreen.svelte';

/** A settings frame over `section`, with that section's name painted. */
function frame(section: GuiSettingsSection): GuiSettingsFrame {
	return {
		section,
		sections: ['services', 'playback', 'mpv'],
		content: { kind: 'rows', rows: [] },
	};
}

beforeEach(() => {
	settingsUi.set(initialSettingsUi());
});

test.each<[GuiSettingsSection, string]>([
	['services', 'Services'],
	['playback', 'Playback'],
	['mpv', 'mpv'],
])('the content frame titles the %s section with its name', (section, title) => {
	const { container } = render(SettingsScreen, {
		props: {
			settings: frame(section),
			status: { prefixArmed: false, toast: null },
			arrangement: 'narrow',
		},
	});

	expect(
		container.querySelector('[data-settings-content-title]')?.textContent?.trim(),
	).toBe(title);
});
