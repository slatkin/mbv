// add-gui-settings-screen row 6.3: the F2-row sections and the Library routes
// list (design D9, D11). Space on a toggle sends `activate` with the row's
// key name; applying a choice list sends `applyChoices` with the marked ids;
// selecting a library in the routes list sends `routeDevices`, and assigning
// a device sends `assignRoute`. The Tauri IPC is mocked; the asserted
// payloads are the serde wire shapes of bridge/settings.rs.

import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiSettingRow, GuiSettingsFrame } from '$lib/bridge/types';
import { routeSettingsKey } from './keys';
import { initialSettingsUi, settingsUi } from './state';

const invokeMock = vi.mocked(invoke);

/** A settings frame over the Queue section's two toggle rows. */
function rowsFrame(rows: GuiSettingRow[]): GuiSettingsFrame {
	return {
		section: 'queue',
		sections: ['services', 'queue', 'keys'],
		content: { kind: 'rows', rows },
	};
}

/** A settings frame over the Library section's routes list. */
function libraryFrame(
	routes: { library: string; name: string; endpoint: string | null }[],
	selected: string | null,
	devices: { name: string; endpoint: string | null }[] | null,
): GuiSettingsFrame {
	return {
		section: 'library',
		sections: ['services', 'library', 'keys'],
		content: { kind: 'library', rows: [], routes, selected, devices },
	};
}

function press(key: string, frame: GuiSettingsFrame): string {
	return routeSettingsKey(new KeyboardEvent('keydown', { key }), frame);
}

/** Put the keyboard on the content pane's `index` entry. */
function cursorAt(index: number): void {
	settingsUi.update((ui) => ({ ...ui, pane: 'content', contentCursor: index, section: 'queue' }));
}

beforeEach(() => {
	invokeMock.mockClear();
	settingsUi.set(initialSettingsUi());
});

test('Space on a toggle sends activate with the row key', () => {
	const frame = rowsFrame([
		{
			key: 'consumeAudio',
			label: 'Consume audio',
			kind: 'toggle',
			value: 'on',
			flag: true,
			choices: null,
			choicesKind: null,
		},
	]);
	cursorAt(0);

	expect(press(' ', frame)).toBe('handled');

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: { command: 'activate', setting: 'consumeAudio' },
	});
});

// The choice list edits in place: Space marks a choice, and the Apply
// entry's Enter commits the marked ids with the apply kind the row carries
// (design D11 *Choice kind*). The row's key is a different choice list's name,
// so a kind derived from the key would send `myLanguages` instead.
test('applying a choice list sends applyChoices with the row-carried kind', () => {
	const frame = rowsFrame([
		{
			key: 'myLanguages',
			label: 'Languages',
			kind: 'choiceList',
			value: 'eng',
			flag: null,
			choices: [
				{ id: 'eng', label: 'English', selected: true },
				{ id: 'deu', label: 'German', selected: false },
			],
			choicesKind: 'hiddenLibraries',
		},
	]);
	cursorAt(1);

	expect(press(' ', frame)).toBe('handled');
	expect(invokeMock).not.toHaveBeenCalled();

	cursorAt(2);
	expect(press('Enter', frame)).toBe('handled');

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: {
			command: 'applyChoices',
			kind: 'hiddenLibraries',
			selected: ['eng', 'deu'],
		},
	});
	expect(get(settingsUi).choiceMarks).toEqual({ myLanguages: ['eng', 'deu'] });
});

test('selecting a library in the routes list sends routeDevices', () => {
	const frame = libraryFrame(
		[
			{ library: 'movies', name: 'Movies', endpoint: null },
			{ library: 'shows', name: 'Shows', endpoint: 'tcp://tv' },
		],
		null,
		null,
	);
	cursorAt(0);

	expect(press('Enter', frame)).toBe('handled');

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: { command: 'routeDevices', library: 'movies' },
	});
});

// The selected library's device list: the automatic entry clears the route,
// a routable device assigns its endpoint, and a device with no endpoint
// activates nothing.
test('assigning a device sends assignRoute with the library and the endpoint', () => {
	const frame = libraryFrame(
		[{ library: 'movies', name: 'Movies', endpoint: null }],
		'movies',
		[
			{ name: 'Living Room', endpoint: 'tcp://tv:8096' },
			{ name: 'Phone', endpoint: null },
		],
	);
	// Entries: route, the automatic entry, the two devices.
	cursorAt(2);

	expect(press('Enter', frame)).toBe('handled');

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: { command: 'assignRoute', library: 'movies', endpoint: 'tcp://tv:8096' },
	});
});

test('the automatic entry clears the route', () => {
	const frame = libraryFrame(
		[{ library: 'movies', name: 'Movies', endpoint: 'tcp://tv:8096' }],
		'movies',
		[{ name: 'Living Room', endpoint: 'tcp://tv:8096' }],
	);
	cursorAt(1);

	expect(press('Enter', frame)).toBe('handled');

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: { command: 'assignRoute', library: 'movies', endpoint: null },
	});
});

test('a device with no endpoint assigns nothing', () => {
	const frame = libraryFrame(
		[{ library: 'movies', name: 'Movies', endpoint: null }],
		'movies',
		[{ name: 'Phone', endpoint: null }],
	);
	cursorAt(2);

	expect(press('Enter', frame)).toBe('handled');

	expect(invokeMock).not.toHaveBeenCalled();
});
