// add-gui-settings-screen U6 review fix: the feed form's Cancel releases
// the keyboard. The button used to drop only `feedForm` and left
// `textField: 'feed'` set, and removing the focused field fires no usable
// `focusout` — the router kept answering 'text' for every key until Esc,
// so the keyboard looked dead. After Cancel a movement key must reach the
// screen's router again.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiSettingsContent, GuiSettingsFrame } from '$lib/bridge/types';
import { settingsEntries } from '$lib/settings/entries';
import { routeSettingsKey } from '$lib/settings/keys';
import { cursorEntryId, initialSettingsUi, settingsUi } from '$lib/settings/state';
import SettingsFeeds from './SettingsFeeds.svelte';

const invokeMock = vi.mocked(invoke);

/** The Feeds content with two subscriptions. */
function feedsContent(): Extract<GuiSettingsContent, { kind: 'feeds' }> {
	return {
		kind: 'feeds',
		subscriptions: [
			{ name: 'A feed', url: 'https://a', kind: 'audio' },
			{ name: 'B feed', url: 'https://b', kind: 'video' },
		],
		adding: false,
	};
}

/** A settings frame over the Feeds section. */
function feedsFrame(): GuiSettingsFrame {
	return {
		section: 'feeds',
		sections: ['services', 'feeds', 'keys'],
		content: feedsContent(),
	};
}

function press(key: string): string {
	return routeSettingsKey(new KeyboardEvent('keydown', { key }), feedsFrame());
}

beforeEach(() => {
	invokeMock.mockClear();
	settingsUi.set(initialSettingsUi());
});

test.each<[string, string | null]>([
	['feedAdd', null],
	['feed:https://a', 'https://a'],
])(
	'a press on %s opens the feed form in place and moves the content cursor to it',
	async (id, identity) => {
		const content = feedsContent();
		const entries = settingsEntries(content, {});
		const { container } = render(SettingsFeeds, {
			props: { content, entries, cursorId: null },
		});

		await fireEvent.click(
			container.querySelector(`[data-settings-entry="${id}"]`) as HTMLElement,
		);

		expect(invokeMock).not.toHaveBeenCalled();
		const ui = get(settingsUi);
		expect(ui.feedForm?.identity ?? null).toBe(identity);
		expect(ui.pane).toBe('content');
		expect(cursorEntryId(entries, ui)).toBe(id);
	},
);

test('a press on a remove row sends feedRemove with the subscription URL', async () => {
	const content = feedsContent();
	const entries = settingsEntries(content, {});
	const { container } = render(SettingsFeeds, {
		props: { content, entries, cursorId: null },
	});

	const id = 'feedRemove:https://a';
	await fireEvent.click(
		container.querySelector(`[data-settings-entry="${id}"]`) as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: { command: 'feedRemove', url: 'https://a' },
	});
	const ui = get(settingsUi);
	expect(ui.pane).toBe('content');
	expect(cursorEntryId(entries, ui)).toBe(id);
});

test('the add form\'s submit sends feedAdd with the typed fields', async () => {
	const content = feedsContent();
	const { container } = render(SettingsFeeds, {
		props: { content, entries: settingsEntries(content, {}), cursorId: null },
	});
	await fireEvent.click(
		container.querySelector('[data-settings-entry="feedAdd"]') as HTMLElement,
	);
	await tick();

	await fireEvent.focusIn(
		container.querySelector('[data-settings-field="name"]') as Element,
	);
	await fireEvent.input(container.querySelector('[data-settings-field="name"]') as Element, {
		target: { value: 'C feed' },
	});
	await fireEvent.input(container.querySelector('[data-settings-field="url"]') as Element, {
		target: { value: 'https://c' },
	});
	await fireEvent.click(
		container.querySelector('[data-settings-feed-submit]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: { command: 'feedAdd', name: 'C feed', url: 'https://c', kind: 'audio' },
	});
	expect(get(settingsUi).feedForm).toBeNull();
});

test('the edit form\'s submit sends feedEdit with the subscription URL as the identity', async () => {
	const content = feedsContent();
	const { container } = render(SettingsFeeds, {
		props: { content, entries: settingsEntries(content, {}), cursorId: null },
	});
	await fireEvent.click(
		container.querySelector('[data-settings-entry="feed:https://a"]') as HTMLElement,
	);
	await tick();

	await fireEvent.focusIn(
		container.querySelector('[data-settings-field="name"]') as Element,
	);
	await fireEvent.input(container.querySelector('[data-settings-field="name"]') as Element, {
		target: { value: 'Renamed' },
	});
	await fireEvent.click(
		container.querySelector('[data-settings-feed-submit]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: { command: 'feedEdit', url: 'https://a', name: 'Renamed', kind: 'audio' },
	});
	expect(get(settingsUi).feedForm).toBeNull();
});

test('Cancel drops the text-field hold, so a movement key reaches the screen router', async () => {
	const content = feedsContent();
	// The form paints on the draft the add control opened; seed it as
	// `openFeedForm` would.
	settingsUi.update((ui) => ({
		...ui,
		pane: 'content',
		section: 'feeds',
		feedForm: { identity: null, name: '', url: '', kind: 'audio' },
	}));
	const { container } = render(SettingsFeeds, {
		props: { content, entries: settingsEntries(content, {}), cursorId: null },
	});
	await tick();

	// A field's focus holds the keyboard (the router's `textField` fact
	// rides the DOM focus events), so a movement key types into the field.
	const form = container.querySelector('[data-settings-feed-form]') as HTMLElement;
	await fireEvent.focusIn(form.querySelector('[data-settings-field="name"]') as Element);
	expect(get(settingsUi).textField).toBe('feed');
	expect(press('ArrowDown')).toBe('text');

	await fireEvent.click(container.querySelector('[data-settings-feed-cancel]') as HTMLElement);

	expect(invokeMock).not.toHaveBeenCalled();
	expect(get(settingsUi).feedForm).toBeNull();
	expect(get(settingsUi).textField).toBeNull();
	expect(press('ArrowDown')).toBe('handled');
	expect(get(settingsUi).contentCursor).toBe(1);
});
