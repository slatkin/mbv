// add-gui-library-panel row 4.4: the pill row paints the core's carried
// selection and sends the select-collection command a press or a key leaves
// (design D6). One group is always selected, so a press selects its pill and
// the Space key does the same on the focused pill. The Tauri IPC is mocked;
// the asserted payloads are the serde wire shape of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiMusicList } from '$lib/bridge/types';
import {
	focusLibraryPill,
	initialLibraryUi,
	libraryUi,
	requestLibraryPillSelect,
} from '$lib/library/state';
import LibraryPills from './LibraryPills.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	libraryUi.set(initialLibraryUi());
});

/** The carried Music content: Hip-Hop selected, Jazz and Soul clear, in a
 * Grouped Music library. */
function music(): GuiMusicList {
	return {
		libIdx: 2,
		pills: [
			{ groupId: 'g1', name: 'Hip-Hop', selected: true, collection: true },
			{ groupId: 'g2', name: 'Jazz', selected: false, collection: true },
			{ groupId: 'g3', name: 'Soul', selected: false, collection: true },
		],
		collectionNames: ['Hip-Hop'],
		loading: false,
		failure: null,
		artists: [],
	};
}

/** One painted pill's button. */
function pill(container: HTMLElement, groupId: string): HTMLElement {
	return container.querySelector(`[data-library-pill="${groupId}"]`) as HTMLElement;
}

test('marks the one selected pill with aria-pressed and no check glyph', () => {
	const { container } = render(LibraryPills, { list: music() });

	expect(pill(container, 'g1').getAttribute('aria-pressed')).toBe('true');
	expect(pill(container, 'g2').getAttribute('aria-pressed')).toBe('false');
	expect(pill(container, 'g1').querySelector('svg')).toBeNull();
});

test('paints no selected count or clear control', () => {
	const { container } = render(LibraryPills, { list: music() });

	expect(container.querySelector('[data-library-pill-clear]')).toBeNull();
});

test('a press selects the pressed pill', () => {
	const { container } = render(LibraryPills, { list: music() });

	fireEvent.click(pill(container, 'g3'));

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'selectCollection', libIdx: 2, groupId: 'g3' },
	});
});

test('Space selects the focused pill', async () => {
	render(LibraryPills, { list: music() });

	requestLibraryPillSelect('g2');
	await tick();

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'selectCollection', libIdx: 2, groupId: 'g2' },
	});
});

test('marks the pill the keys focus holds and sends nothing for it', async () => {
	const { container } = render(LibraryPills, { list: music() });

	focusLibraryPill('g2');
	await tick();
	expect(pill(container, 'g2').hasAttribute('data-library-pill-focused')).toBe(true);

	focusLibraryPill('g3');
	await tick();
	expect(pill(container, 'g3').hasAttribute('data-library-pill-focused')).toBe(true);
	expect(pill(container, 'g2').hasAttribute('data-library-pill-focused')).toBe(false);
	expect(invokeMock).not.toHaveBeenCalled();
});
