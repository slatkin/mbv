// add-gui-library-panel row 10.1: the Podcasts pills (design D12) paint the
// core's single-select view with `aria-pressed`, and a press sends the
// set-pill command with that pill's object — as the frame carries it, never a
// stringified pill. The Tauri IPC is mocked; the asserted payload is the
// serde wire shape of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import { initialListScroll, podcastsScroll } from '$lib/library/scroll';
import type { GuiPodcastsList } from '$lib/bridge/types';
import { initialPodcastsUi, podcastsUi } from '$lib/library/podcasts';
import LibraryPodcastsPills from './LibraryPodcastsPills.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	podcastsUi.set(initialPodcastsUi());
	podcastsScroll.set(initialListScroll());
});

/** The carried Podcasts content with the show view selected (design D12). */
function frame(): GuiPodcastsList {
	return {
		libIdx: 0,
		pills: [
			{ pill: { kind: 'latest' }, name: 'Latest', selected: false, collection: false },
			{ pill: { kind: 'show', id: 'sh1' }, name: 'Show One', selected: true, collection: false },
		],
		loading: false,
		failed: false,
		rows: [],
	};
}

test('the pills paint the core’s selection and never none', () => {
	const { container } = render(LibraryPodcastsPills, { list: frame() });
	const pressed = [...container.querySelectorAll('[data-library-pill]')].map((el) => [
		el.getAttribute('data-library-pill'),
		el.getAttribute('aria-pressed'),
	]);

	expect(pressed).toEqual([
		['latest', 'false'],
		['show:sh1', 'true'],
	]);
});

test('a press sends the set-pill command with the frame’s pill object', async () => {
	const { container } = render(LibraryPodcastsPills, { list: frame() });

	await fireEvent.click(container.querySelector('[data-library-pill="latest"]') as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'podcastsSetPill', libIdx: 0, pill: { kind: 'latest' } },
	});
});
