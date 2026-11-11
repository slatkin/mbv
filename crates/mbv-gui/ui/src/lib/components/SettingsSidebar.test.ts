// add-gui-settings-screen rows 8.5, 9.5, and the design's *Sources label*:
// the sidebar's pointer path and its canvas paint (the spec's *Every Settings
// control works by pointer* and *A Back control in the sidebar closes the
// screen*, design D11, D13). A press on a section entry sends `selectSection`,
// moves the sidebar cursor onto the entry, and hands the keyboard to the
// content — the same effect Enter on the cursor row has. Each entry paints its
// section's icon and name, the open section's entry carries
// `aria-current="page"`, and a press on Back closes the screen. The Services
// section paints as Sources, a label only. The Tauri IPC is mocked; the
// asserted payload is the serde wire shape of bridge/settings.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiSettingsFrame, GuiSettingsSection } from '$lib/bridge/types';
import { hideSettingsCursor, initialSettingsUi, settingsUi, showSettingsCursor } from '$lib/settings/state';
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

/** One section row of a rendered sidebar. */
function row(container: HTMLElement, section: GuiSettingsSection): HTMLElement {
	return container.querySelector(`[data-settings-section="${section}"]`) as HTMLElement;
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

		await fireEvent.click(row(container, section));

		expect(invokeMock).toHaveBeenCalledWith('settings', {
			command: { command: 'selectSection', section },
		});
		const ui = get(settingsUi);
		expect(ui.sidebarCursor).toBe(index);
		expect(ui.pane).toBe('content');
	},
);

test.each<[GuiSettingsSection, string]>([
	['services', 'Sources'],
	['feeds', 'Feeds'],
	['keys', 'Keys'],
])('the %s entry paints its icon and its name', (section, name) => {
	const { container } = render(SettingsSidebar, { props: { settings: frame() } });

	const entry = row(container, section);

	expect(entry.querySelector('svg')).not.toBeNull();
	expect(entry.textContent?.trim()).toBe(name);
});

test.each<[GuiSettingsSection, string | null]>([
	['services', 'page'],
	['feeds', null],
	['keys', null],
])('the %s entry carries aria-current %s', (section, current) => {
	const { container } = render(SettingsSidebar, { props: { settings: frame() } });

	expect(row(container, section).getAttribute('aria-current')).toBe(current);
});

test('a press on Back closes the screen', async () => {
	const { container } = render(SettingsSidebar, { props: { settings: frame() } });

	await fireEvent.click(container.querySelector('[data-settings-back]') as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: { command: 'close' },
	});
});

// A pointer user sees no row that looks picked: the cursor paints only after a
// screen key, like `:focus-visible` (the first row looked selected at open).
test('the sidebar cursor row paints only while the keyboard shows it', async () => {
	const { container } = render(SettingsSidebar, { props: { settings: frame() } });
	const painted = () => container.querySelector('[data-settings-cursor]') !== null;

	const atOpen = painted();
	showSettingsCursor();
	await tick();
	const afterKey = painted();
	hideSettingsCursor();
	await tick();
	const afterPointer = painted();

	expect([atOpen, afterKey, afterPointer]).toEqual([false, true, false]);
});
