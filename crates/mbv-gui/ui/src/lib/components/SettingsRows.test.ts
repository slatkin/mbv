// add-gui-settings-screen row 8.5: the F2-row sections' pointer path (the
// spec's *Every Settings control works by pointer*, design D11). A press on
// a toggle row sends the row's `activate`, a press on a choice mark flips
// the held mark in place with no command, and a press on Apply sends
// `applyChoices` with the marked ids — each the same command the keyboard's
// activation sends, and each moves the content cursor onto the pressed row.
// The Tauri IPC is mocked; the asserted payload is the serde wire shape of
// bridge/settings.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiSettingsCommand, GuiSettingsContent } from '$lib/bridge/types';
import { settingsEntries } from '$lib/settings/entries';
import { cursorEntryId, initialSettingsUi, settingsUi } from '$lib/settings/state';
import SettingsRows from './SettingsRows.svelte';

const invokeMock = vi.mocked(invoke);

/** A rows content: one toggle row and one choice-list row. */
function rowsContent(): Extract<GuiSettingsContent, { kind: 'rows' }> {
	return {
		kind: 'rows',
		rows: [
			{
				key: 'autoplay',
				label: 'Autoplay',
				kind: 'toggle',
				value: 'Off',
				flag: false,
				choices: null,
				choicesKind: null,
				values: null,
			},
			{
				key: 'hiddenLibraries',
				label: 'Hidden libraries',
				kind: 'choiceList',
				value: '',
				flag: null,
				choices: [
					{ id: 'music', label: 'Music', selected: false },
					{ id: 'shows', label: 'Shows', selected: true },
				],
				choicesKind: 'hiddenLibraries',
				values: null,
			},
		],
	};
}

beforeEach(() => {
	invokeMock.mockClear();
	settingsUi.set(initialSettingsUi());
});

test.each<[string, GuiSettingsCommand]>([
	['setting:autoplay', { command: 'activate', setting: 'autoplay' }],
	[
		'apply:hiddenLibraries',
		{ command: 'applyChoices', kind: 'hiddenLibraries', selected: ['shows'] },
	],
])(
	'a press on %s sends its command and moves the content cursor to it',
	async (id, command) => {
		const content = rowsContent();
		const entries = settingsEntries(content, {});
		const { container } = render(SettingsRows, {
			props: { rows: content.rows, entries, cursorId: null },
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

test('a press on a choice mark flips the held mark in place and sends no command', async () => {
	const content = rowsContent();
	const entries = settingsEntries(content, {});
	const { container } = render(SettingsRows, {
		props: { rows: content.rows, entries, cursorId: null },
	});

	const id = 'choice:hiddenLibraries:music';
	await fireEvent.click(
		container.querySelector(`[data-settings-entry="${id}"]`) as HTMLElement,
	);

	expect(invokeMock).not.toHaveBeenCalled();
	const ui = get(settingsUi);
	expect(ui.choiceMarks['hiddenLibraries']).toEqual(['shows', 'music']);
	expect(ui.pane).toBe('content');
	expect(cursorEntryId(entries, ui)).toBe(id);
});
