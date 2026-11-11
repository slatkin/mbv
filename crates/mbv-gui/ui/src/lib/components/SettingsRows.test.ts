// add-gui-settings-screen rows 8.5 and 9.6: the F2-row sections' pointer path
// and their canvas paint (the spec's *Every Settings control works by
// pointer*, design D11, D13). A toggle row paints a switch whose press sends
// the row's `activate`; a choice list paints a checkbox-grid card whose boxes
// hold the marks and whose Apply sends `applyChoices` with them — each the
// same command the keyboard's activation sends, and each moves the content
// cursor onto the pressed row. The Tauri IPC is mocked; the asserted payload
// is the serde wire shape of bridge/settings.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiSettingsContent } from '$lib/bridge/types';
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

test('a press on a toggle row switch sends activate and moves the content cursor to it', async () => {
	const content = rowsContent();
	const entries = settingsEntries(content, {});
	const { getByRole } = render(SettingsRows, {
		props: { rows: content.rows, entries, cursorId: null },
	});

	await fireEvent.click(getByRole('switch', { name: 'Autoplay' }));

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: { command: 'activate', setting: 'autoplay' },
	});
	const ui = get(settingsUi);
	expect(ui.pane).toBe('content');
	expect(cursorEntryId(entries, ui)).toBe('setting:autoplay');
});

test('a press on the choice list Apply sends applyChoices and moves the content cursor to it', async () => {
	const content = rowsContent();
	const entries = settingsEntries(content, {});
	const { getByRole } = render(SettingsRows, {
		props: { rows: content.rows, entries, cursorId: null },
	});

	await fireEvent.click(getByRole('button', { name: 'Apply' }));

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: { command: 'applyChoices', kind: 'hiddenLibraries', selected: ['shows'] },
	});
	const ui = get(settingsUi);
	expect(ui.pane).toBe('content');
	expect(cursorEntryId(entries, ui)).toBe('apply:hiddenLibraries');
});

// The grid card holds the marks until Apply sends them: a press on a box
// sends nothing, and the Apply press carries the frame's marks plus the
// pressed one.
test('a press on a choice box holds the mark, and Apply sends the marked ids', async () => {
	const content = rowsContent();
	const entries = settingsEntries(content, {});
	const { container, getByRole } = render(SettingsRows, {
		props: { rows: content.rows, entries, cursorId: null },
	});

	await fireEvent.click(
		container.querySelector('[data-settings-checkbox="music"]') as HTMLElement,
	);
	expect(invokeMock).not.toHaveBeenCalled();

	await fireEvent.click(getByRole('button', { name: 'Apply' }));

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: { command: 'applyChoices', kind: 'hiddenLibraries', selected: ['shows', 'music'] },
	});
});
