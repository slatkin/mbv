// add-gui-library-panel row 11.1: the Feeds pills (design D12) paint the
// core's single-select view with `aria-pressed`, and a press sends the
// set-pill command with that pill's object — as the frame carries it, never a
// stringified pill. The Tauri IPC is mocked; the asserted payload is the
// serde wire shape of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiFeedsFrame } from '$lib/bridge/types';
import { initialFeedsUi, feedsUi } from '$lib/library/feeds';
import LibraryFeedsPills from './LibraryFeedsPills.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	feedsUi.set(initialFeedsUi());
});

/** The carried Feeds content with the subscription view selected (design
 * D12). */
function frame(): GuiFeedsFrame {
	return {
		pills: [
			{ pill: { kind: 'latest' }, name: 'Latest', selected: false, collection: false },
			{
				pill: { kind: 'subscription', id: 'https://sub.one' },
				name: 'Sub One',
				selected: true,
				collection: false,
			},
		],
		loading: false,
		failed: false,
		rows: [],
		selected: null,
		hero: null,
	};
}

test('the pills paint the core’s selection and never none', () => {
	const { container } = render(LibraryFeedsPills, { feeds: frame() });
	const pressed = [...container.querySelectorAll('[data-library-pill]')].map((el) => [
		el.getAttribute('data-library-pill'),
		el.getAttribute('aria-pressed'),
	]);

	expect(pressed).toEqual([
		['latest', 'false'],
		['subscription:https://sub.one', 'true'],
	]);
});

test('a press sends the set-pill command with the frame’s pill object', async () => {
	const { container } = render(LibraryFeedsPills, { feeds: frame() });

	await fireEvent.click(container.querySelector('[data-library-pill="latest"]') as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'feedsSetPill', pill: { kind: 'latest' } },
	});
});
