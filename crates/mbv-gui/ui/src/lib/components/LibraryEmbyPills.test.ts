// add-gui-library-panel row 8.1: the plain Emby-library destination's feed-group
// pills (design D12) paint the core's single-select group with `aria-pressed`,
// and a press sends the set-feed-group command with that pill's group. The Tauri
// IPC is mocked; the asserted payload is the serde wire shape of
// bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiEmbyFrame } from '$lib/bridge/types';
import { embyUi, initialEmbyUi } from '$lib/library/emby';
import LibraryEmbyPills from './LibraryEmbyPills.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	embyUi.set(initialEmbyUi());
});

/** The carried Emby content in the feed-group mode with `All` selected (row
 * 8.1). */
function embyFrame(): GuiEmbyFrame {
	return {
		libIdx: 3,
		libraryName: 'Movies',
		pills: [
			{ group: { kind: 'all' }, name: 'All', selected: true, collection: false },
			{
				group: { kind: 'folder', id: 'folder-1' },
				name: 'Family',
				selected: false,
				collection: false,
			},
		],
		collectionNames: ['All'],
		feedGroup: true,
		loading: false,
		failed: false,
		rows: [],
		selected: null,
		hero: null,
	};
}

test('the pills paint the core\u2019s selection and never none', () => {
	const { container } = render(LibraryEmbyPills, { emby: embyFrame() });
	const pressed = [...container.querySelectorAll('[data-library-emby-pill]')].map((el) => [
		el.getAttribute('data-library-emby-pill'),
		el.getAttribute('aria-pressed'),
	]);

	expect(pressed).toEqual([
		['all', 'true'],
		['folder:folder-1', 'false'],
	]);
});

test('a press sends the set-feed-group command with the pill\u2019s group', async () => {
	const { container } = render(LibraryEmbyPills, { emby: embyFrame() });

	await fireEvent.click(
		container.querySelector('[data-library-emby-pill="folder:folder-1"]') as HTMLElement,
	);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: {
			command: 'embySetFeedGroup',
			libIdx: 3,
			group: { kind: 'folder', id: 'folder-1' },
		},
	});
});

// The keys' Space and Enter intent (design D3, row 8.1) becomes the command a
// press sends: the state consumes the request, so one key sends one command.
test('the keys\u2019 pill request sends the set-feed-group command', () => {
	embyUi.update((ui) => ({ ...ui, pillSelect: { kind: 'folder', id: 'folder-1' } }));

	render(LibraryEmbyPills, { emby: embyFrame() });

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: {
			command: 'embySetFeedGroup',
			libIdx: 3,
			group: { kind: 'folder', id: 'folder-1' },
		},
	});
});
