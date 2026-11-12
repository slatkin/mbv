// add-gui-settings-screen row 6.1: the Settings screen's key router (design
// D11). An active capture swallows its key and sends `keysCapture`, Esc
// cancels a capture without a send, Esc walks the pane ladder (content to
// sidebar, then `close`), and Space on the sidebar stays the core's
// play/pause. The Tauri IPC is mocked; the asserted payloads are the serde
// wire shapes of bridge/settings.rs.

import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiSettingsFrame } from '$lib/bridge/types';
import { routeSettingsKey } from './keys';
import { pickMenu } from './pick-menu';
import { initialSettingsUi, settingsUi, startKeysCapture } from './state';

const invokeMock = vi.mocked(invoke);

/** A settings frame over the Queue section: the sidebar's full section list
 * and a two-row content. */
function settings(): GuiSettingsFrame {
	return {
		section: 'queue',
		sections: ['services', 'playback', 'queue', 'actions', 'keys', 'sessions', 'playlists'],
		content: {
			kind: 'rows',
			rows: [
				{
					key: 'consumeAudio',
					label: 'Consume audio',
					kind: 'toggle',
					value: 'on',
					flag: true,
					choices: null,
					choicesKind: null,
					values: null,
				},
				{
					key: 'alwaysPlayNext',
					label: 'Always play next',
					kind: 'toggle',
					value: 'off',
					flag: false,
					choices: null,
					choicesKind: null,
					values: null,
				},
			],
		},
	};
}

function press(key: string, options: KeyboardEventInit = {}): string {
	return routeSettingsKey(new KeyboardEvent('keydown', { ...options, key }), settings());
}

beforeEach(() => {
	invokeMock.mockClear();
	settingsUi.set(initialSettingsUi());
	pickMenu.set(null);
});

// A capture takes every key: F2 is its candidate, not the core's settings
// toggle — and the sent command carries the capture target and the raw DOM
// key.
test('a capture swallows F2 and sends keysCapture', () => {
	startKeysCapture('next_track');

	expect(press('F2')).toBe('handled');

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: {
			command: 'keysCapture',
			target: 'next_track',
			key: {
				key: 'F2',
				code: '',
				metaKey: false,
				repeat: false,
				shiftKey: false,
				ctrlKey: false,
				altKey: false,
			},
		},
	});
});

test('Esc cancels a capture and sends nothing', () => {
	startKeysCapture('next_track');

	expect(press('Escape')).toBe('handled');

	expect(get(settingsUi).capture).toBeNull();
	expect(invokeMock).not.toHaveBeenCalled();
});

// The Esc ladder: content hands the keyboard back to the sidebar, and the
// sidebar's Esc closes the screen.
test('Esc moves content to the sidebar, then sends close', () => {
	settingsUi.update((ui) => ({ ...ui, pane: 'content', section: 'queue' }));

	expect(press('Escape')).toBe('handled');
	expect(get(settingsUi).pane).toBe('sidebar');
	expect(invokeMock).not.toHaveBeenCalled();

	expect(press('Escape')).toBe('handled');
	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: { command: 'close' },
	});
});

test('Space on the sidebar goes to the core', () => {
	settingsUi.update((ui) => ({ ...ui, pane: 'sidebar', section: 'queue' }));

	expect(press(' ')).toBe('core');

	expect(invokeMock).not.toHaveBeenCalled();
});

/** A settings frame over the Playback section with one pick row. */
function pickSettings(): GuiSettingsFrame {
	return {
		section: 'playback',
		sections: ['playback'],
		content: {
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
		},
	};
}

// Enter on a pick row opens its value list (the spec's *Value settings are
// picked from their list*) and sends nothing; the menu's own choice sends
// `setValue`.
test('Enter on a pick row opens its menu and sends nothing', () => {
	settingsUi.update((ui) => ({ ...ui, pane: 'content' }));

	expect(routeSettingsKey(new KeyboardEvent('keydown', { key: 'Enter' }), pickSettings())).toBe(
		'handled',
	);

	expect(get(pickMenu)?.setting).toBe('subtitleLanguage');
	expect(invokeMock).not.toHaveBeenCalled();
});

// Esc from the sidebar closes the screen even with a setup form open: the
// form's cancel steps belong to the content pane (the spec's *Esc closes the
// screen from the sidebar*).
test('Esc from the sidebar with a setup form open sends close, not cancelSetup', () => {
	const setupSettings: GuiSettingsFrame = {
		section: 'services',
		sections: ['services'],
		content: {
			kind: 'services',
			services: [],
			setup: { service: 'emby', serverUrl: '', busy: false, error: '' },
		},
	};

	expect(routeSettingsKey(new KeyboardEvent('keydown', { key: 'Escape' }), setupSettings)).toBe(
		'handled',
	);

	expect(invokeMock).toHaveBeenCalledTimes(1);
	expect(invokeMock).toHaveBeenCalledWith('settings', { command: { command: 'close' } });
});
