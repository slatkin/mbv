// add-gui-library-panel row 7.1: the Shows mode pills (design D12) paint the
// core's single-select view mode with `aria-pressed`, and a press sends the
// set-mode command with that pill's mode. The Tauri IPC is mocked; the asserted
// payload is the serde wire shape of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiShowsFrame } from '$lib/bridge/types';
import { initialShowsUi, showsUi } from '$lib/library/shows';
import LibraryShowsPills from './LibraryShowsPills.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	showsUi.set(initialShowsUi());
});

/** The carried Shows content with `all` selected (design D12). */
function showsFrame(): GuiShowsFrame {
	return {
		libIdx: 0,
		pills: [
			{ mode: 'latest', name: 'Latest', selected: false, collection: false },
			{ mode: 'upcoming', name: 'Upcoming', selected: false, collection: false },
			{ mode: 'all', name: 'All series', selected: true, collection: false },
		],
		collectionNames: ['All series'],
		loading: false,
		workspaceLoading: false,
		series: [],
		upcomingFailed: false,
		rows: [],
		selected: null,
		hero: null,
	};
}

test('the pills paint the core\u2019s selection and never none', () => {
	const { container } = render(LibraryShowsPills, { shows: showsFrame() });
	const pressed = [...container.querySelectorAll('[data-library-pill]')].map((el) => [
		el.getAttribute('data-library-pill'),
		el.getAttribute('aria-pressed'),
	]);

	expect(pressed).toEqual([
		['latest', 'false'],
		['upcoming', 'false'],
		['all', 'true'],
	]);
});

test('a press sends the set-mode command with the pill\u2019s mode', async () => {
	const { container } = render(LibraryShowsPills, { shows: showsFrame() });

	await fireEvent.click(container.querySelector('[data-library-pill="upcoming"]') as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'showsSetMode', libIdx: 0, mode: 'upcoming' },
	});
});
