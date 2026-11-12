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
import type { GuiSettingRow, GuiSettingsContent, GuiSettingsFrame } from '$lib/bridge/types';
import { settingsEntries } from '$lib/settings/entries';
import { routeSettingsKey } from '$lib/settings/keys';
import { pickMenu } from '$lib/settings/pick-menu';
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
	pickMenu.set(null);
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

/** A rows content: one pick row over three values, the empty one painted as Any. */
function pickContent(): Extract<GuiSettingsContent, { kind: 'rows' }> {
	return {
		kind: 'rows',
		rows: [
			{
				key: 'subtitleLanguage',
				label: 'Subtitle language',
				kind: 'pick',
				value: 'English',
				flag: null,
				choices: null,
				choicesKind: null,
				values: ['', 'English', 'French'],
			},
		],
	};
}

/** The keyboard's position: the content pane and its cursor index. */
function focusContent(index: number): void {
	settingsUi.update((ui) => ({ ...ui, pane: 'content', contentCursor: index }));
}

/** A frame over the rows content, for the key router. */
function rowsFrame(content: GuiSettingsContent): GuiSettingsFrame {
	return { section: 'library', sections: ['library'], content };
}

/** The entry index of one entry id in the rows content. */
function indexOf(entries: ReturnType<typeof settingsEntries>, id: string): number {
	return entries.findIndex((entry) => entry.id === id);
}

// A press on a pick button runs the row's activation: the cursor moves onto
// the row and its value list opens, with no send.
test('a press on a pick button moves the content cursor onto its row and opens its menu', async () => {
	const content = pickContent();
	const entries = settingsEntries(content, {});
	const { container } = render(SettingsRows, {
		props: { rows: content.rows, entries, cursorId: null },
	});

	await fireEvent.click(container.querySelector('[data-settings-pick] button') as HTMLElement);

	const ui = get(settingsUi);
	expect(ui.pane).toBe('content');
	expect(cursorEntryId(entries, ui)).toBe('setting:subtitleLanguage');
	expect(get(pickMenu)?.setting).toBe('subtitleLanguage');
	expect(invokeMock).not.toHaveBeenCalled();
});

// A mark a press makes is the mark a keyboard Apply saves: both paths edit
// the one store.
test('a pointer mark is the set a keyboard Apply saves', async () => {
	const content = rowsContent();
	const entries = settingsEntries(content, {});
	const { container } = render(SettingsRows, {
		props: { rows: content.rows, entries, cursorId: null },
	});

	await fireEvent.click(
		container.querySelector('[data-settings-checkbox="music"]') as HTMLElement,
	);
	focusContent(indexOf(entries, 'apply:hiddenLibraries'));
	routeSettingsKey(new KeyboardEvent('keydown', { key: 'Enter' }), rowsFrame(content));

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: { command: 'applyChoices', kind: 'hiddenLibraries', selected: ['shows', 'music'] },
	});
});

// Space marks the box under the keyboard; a pointer Apply then saves that set.
test('a Space mark is the set a pointer Apply saves', async () => {
	const content = rowsContent();
	const entries = settingsEntries(content, {});
	const { getByRole } = render(SettingsRows, {
		props: { rows: content.rows, entries, cursorId: null },
	});

	focusContent(indexOf(entries, 'choice:hiddenLibraries:music'));
	routeSettingsKey(new KeyboardEvent('keydown', { key: ' ' }), rowsFrame(content));
	await fireEvent.click(getByRole('button', { name: 'Apply' }));

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: { command: 'applyChoices', kind: 'hiddenLibraries', selected: ['shows', 'music'] },
	});
});

// The keyboard's cursor paints on a choice box it rests on, with the rows'
// cursor fact.
test('the cursor paints on a choice box after a key moves onto it', () => {
	const content = rowsContent();
	const entries = settingsEntries(content, {});
	focusContent(indexOf(entries, 'setting:autoplay'));
	routeSettingsKey(new KeyboardEvent('keydown', { key: 'ArrowDown' }), rowsFrame(content));

	const { container } = render(SettingsRows, {
		props: {
			rows: content.rows,
			entries,
			cursorId: cursorEntryId(entries, get(settingsUi)),
		},
	});

	expect(
		container
			.querySelector('[data-settings-entry="choice:hiddenLibraries:music"]')
			?.hasAttribute('data-settings-cursor'),
	).toBe(true);
});

/** A Session section's two version rows: read-only `info` rows. */
function infoContent(): Extract<GuiSettingsContent, { kind: 'rows' }> {
	const info = (key: string, label: string, value: string): GuiSettingRow => ({
		key,
		label,
		kind: 'info',
		value,
		flag: null,
		choices: null,
		choicesKind: null,
		values: null,
	});
	return {
		kind: 'rows',
		rows: [
			info('clientVersion', 'This client', '1.2.3 · abc · def'),
			info('ownerVersion', 'Owner', '1.2.3 · unknown · unknown'),
		],
	};
}

// An info row paints its label and value with no control, and neither Enter
// on it nor a click on it sends a command (owner-gate-on-wire-identity design
// D5).
test('an info row paints its value with no control, and Enter and a click send no command', async () => {
	const content = infoContent();
	const entries = settingsEntries(content, {});
	const { container, getByText, queryByRole } = render(SettingsRows, {
		props: { rows: content.rows, entries, cursorId: null },
	});

	expect(getByText('This client')).toBeTruthy();
	expect(getByText('1.2.3 · abc · def')).toBeTruthy();
	expect(queryByRole('button')).toBeNull();
	expect(queryByRole('switch')).toBeNull();

	focusContent(indexOf(entries, 'setting:clientVersion'));
	routeSettingsKey(new KeyboardEvent('keydown', { key: 'Enter' }), rowsFrame(content));
	await fireEvent.click(
		container.querySelector('[data-settings-info="clientVersion"]') as HTMLElement,
	);

	expect(invokeMock).not.toHaveBeenCalled();
});
