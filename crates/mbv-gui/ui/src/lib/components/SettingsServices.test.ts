// add-gui-settings-screen rows 6.4 and 9.7: the Services section's Emby setup
// form (design D9) and its canvas paint (design D13, the Services grid). A
// submit sends `embySetup` with the three fields and then the password field
// is empty — the typed secret rides the command and stays in no draft or
// painted value (the spec's *Secrets never return*). Each Service is its own
// card, so the open form paints inside the card of the Service it sets up, and
// Remove paints as the destructive button. The Tauri IPC is mocked; the
// asserted payload is the serde wire shape of bridge/settings.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiSettingsCommand, GuiSettingsContent } from '$lib/bridge/types';
import { settingsEntries } from '$lib/settings/entries';
import { cursorEntryId, initialSettingsUi, settingsUi } from '$lib/settings/state';
import SettingsServices from './SettingsServices.svelte';

const invokeMock = vi.mocked(invoke);

/** The Services content with the Emby setup form open. */
function servicesContent(): Extract<GuiSettingsContent, { kind: 'services' }> {
	return {
		kind: 'services',
		services: [
			{
				entry: 'emby',
				name: 'Emby',
				state: 'Not configured',
				context: '',
				actions: [{ intent: 'setupEmby', label: 'Set up Emby' }],
			},
			{
				entry: 'audiobookshelf',
				name: 'Audiobookshelf',
				state: 'Not configured',
				context: '',
				actions: [{ intent: 'setupAudiobookshelf', label: 'Set up Audiobookshelf' }],
			},
			{
				entry: 'feeds',
				name: 'Feeds',
				state: 'Ready',
				context: '1 subscription',
				actions: [{ intent: 'manageFeeds', label: 'Manage feeds' }],
			},
		],
		setup: { service: 'emby', serverUrl: 'http://emby', busy: false, error: '' },
	};
}

beforeEach(() => {
	invokeMock.mockClear();
	settingsUi.set(initialSettingsUi());
});

/** The Audiobookshelf card alone, with its form open and the four actions a
 * configured Service offers. */
function audiobookshelfContent(): Extract<GuiSettingsContent, { kind: 'services' }> {
	return {
		kind: 'services',
		services: [
			{
				entry: 'audiobookshelf',
				name: 'Audiobookshelf',
				state: 'Ready',
				context: '',
				actions: [
					{ intent: 'testAudiobookshelf', label: 'Test connection' },
					{ intent: 'setupAudiobookshelf', label: 'Set up Audiobookshelf' },
					{ intent: 'replaceAudiobookshelf', label: 'Replace' },
					{ intent: 'removeAudiobookshelf', label: 'Remove' },
				],
			},
		],
		setup: { service: 'audiobookshelf', serverUrl: 'http://abs', busy: false, error: '' },
	};
}

test('a submit sends embySetup with the three fields, then the password field is empty', async () => {
	const content = servicesContent();
	// The form paints on the draft the frame's open seeded (state.ts's
	// sync); seed it as the arriving frame would.
	settingsUi.update((ui) => ({
		...ui,
		embySetup: { serverUrl: 'http://emby', username: '', password: '' },
	}));
	const { container, getByRole } = render(SettingsServices, {
		props: { content, entries: settingsEntries(content, {}), cursorId: null },
	});
	await tick();

	// The form hands its first field the keyboard on open (the router's
	// `textField` fact rides the DOM focus events).
	const form = container.querySelector('[data-settings-setup="emby"]') as HTMLElement;
	expect(form).toBeTruthy();
	await fireEvent.input(form.querySelector('[data-settings-field="username"]') as Element, {
		target: { value: 'user@x' },
	});
	await fireEvent.input(form.querySelector('[data-settings-field="password"]') as Element, {
		target: { value: 's3cret' },
	});
	await fireEvent.click(getByRole('button', { name: 'Set up' }));

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: {
			command: 'embySetup',
			serverUrl: 'http://emby',
			username: 'user@x',
			password: 's3cret',
		},
	});
	// The password input is a password field, and the submit emptied it.
	const password = form.querySelector(
		'[data-settings-field="password"]',
	) as HTMLInputElement;
	expect(password.type).toBe('password');
	expect(password.value).toBe('');
});

test.each<[string, GuiSettingsCommand]>([
	['service:emby:setupEmby', { command: 'serviceAction', action: 'setupEmby' }],
	[
		'service:audiobookshelf:setupAudiobookshelf',
		{ command: 'serviceAction', action: 'setupAudiobookshelf' },
	],
	['service:feeds:manageFeeds', { command: 'serviceAction', action: 'manageFeeds' }],
])(
	'a press on %s sends its serviceAction and moves the content cursor to it',
	async (id, command) => {
		const content = servicesContent();
		const entries = settingsEntries(content, {});
		const { container } = render(SettingsServices, {
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

test('the setup form\'s Cancel sends cancelSetup for the form\'s service', async () => {
	const content = servicesContent();
	settingsUi.update((ui) => ({
		...ui,
		embySetup: { serverUrl: 'http://emby', username: '', password: '' },
	}));
	const { getByRole } = render(SettingsServices, {
		props: { content, entries: settingsEntries(content, {}), cursorId: null },
	});
	await tick();

	await fireEvent.click(getByRole('button', { name: 'Cancel' }));

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: { command: 'cancelSetup', service: 'emby' },
	});
});

test('an open Audiobookshelf form renders inside the Audiobookshelf card', async () => {
	const content = audiobookshelfContent();
	settingsUi.update((ui) => ({
		...ui,
		audiobookshelfSetup: { serverUrl: 'http://abs', apiKey: '' },
	}));
	const { container } = render(SettingsServices, {
		props: { content, entries: settingsEntries(content, {}), cursorId: null },
	});
	await tick();

	const card = container.querySelector('[data-settings-card][aria-label="Audiobookshelf"]');
	expect(card).toBeTruthy();
	expect(card?.querySelector('[data-settings-setup="audiobookshelf"]')).toBeTruthy();
});

test('Remove paints as the destructive button', () => {
	const content = audiobookshelfContent();
	const { getByRole } = render(SettingsServices, {
		props: { content, entries: settingsEntries(content, {}), cursorId: null },
	});

	expect(getByRole('button', { name: 'Remove' }).getAttribute('data-settings-button')).toBe(
		'destructive',
	);
});
