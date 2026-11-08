// add-gui-library-panel row 4.2: a press inside the Library panel sends the
// take-focus command (design D4), so the core sets the focus stack and Panel
// focus together before the press acts on the row. The Tauri IPC is mocked;
// the asserted payload is the serde wire shape of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiMusicFrame } from '$lib/bridge/types';
import LibraryPanel from './LibraryPanel.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
});

/** The carried Music content with no album picked. */
function music(): GuiMusicFrame {
	return {
		libIdx: 0,
		pills: [],
		loading: false,
		failure: null,
		artists: [],
		hero: null,
		art: null,
	};
}

test('a press inside the panel sends the take-focus command', () => {
	const { container } = render(LibraryPanel, {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
		music: music(),
		arrangement: 'wide',
	});
	const panel = container.querySelector('[data-library-panel]');

	expect(panel).toBeTruthy();
	fireEvent.pointerDown(panel as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'takeFocus' },
	});
});

test('the panel paints the top bar and the pill row above the list region', () => {
	const { container } = render(LibraryPanel, {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
		music: music(),
		arrangement: 'wide',
	});
	const panel = container.querySelector('[data-library-panel]');
	const regions = [...(panel?.querySelectorAll('[data-library-region]') ?? [])].map((el) =>
		el.getAttribute('data-library-region'),
	);

	expect(regions.slice(0, 3)).toEqual(['topBar', 'pills', 'hero']);
	expect(regions).toContain('list');
});
