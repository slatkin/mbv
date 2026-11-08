// add-gui-library-panel row 4.4: the pill row paints the core's carried
// selection and sends the set a gesture leaves (design D6). The pointer
// gestures and the keys' Space and Alt+Space intent send the same
// set-collections command; the row marks the pill the keys' focus holds. The
// Tauri IPC is mocked; the asserted payloads are the serde wire shape of
// bridge/library.rs. The gesture table of the rules lives in
// `$lib/library/pills.test.ts`.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiMusicFrame } from '$lib/bridge/types';
import {
	focusLibraryPill,
	initialLibraryUi,
	libraryUi,
	requestLibraryPillToggle,
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

/** The carried Music content: Hip-Hop and Jazz selected, Soul and the
 * single-select Latest pill clear, in a Grouped Music library. */
function music(): GuiMusicFrame {
	return {
		libIdx: 2,
		pills: [
			{ groupId: 'g1', name: 'Hip-Hop', selected: true, collection: true },
			{ groupId: 'g2', name: 'Jazz', selected: true, collection: true },
			{ groupId: 'g3', name: 'Soul', selected: false, collection: true },
			{ groupId: 'latest', name: 'Latest', selected: false, collection: false },
		],
		collectionNames: ['Hip-Hop', 'Jazz'],
		loading: false,
		failure: null,
		artists: [],
		hero: null,
		art: null,
	};
}

/** One painted pill's button. */
function pill(container: HTMLElement, groupId: string): HTMLElement {
	return container.querySelector(`[data-library-pill="${groupId}"]`) as HTMLElement;
}

/** The clear control's button. */
function clearControl(container: HTMLElement): HTMLElement {
	return container.querySelector('[data-library-pill-clear]') as HTMLElement;
}

test('paints aria-pressed and the check on the selected pills', () => {
	const { container } = render(LibraryPills, { music: music() });

	expect(pill(container, 'g1').getAttribute('aria-pressed')).toBe('true');
	expect(pill(container, 'g1').querySelector('svg')).toBeTruthy();
	expect(pill(container, 'g3').getAttribute('aria-pressed')).toBe('false');
	expect(pill(container, 'g3').querySelector('svg')).toBeNull();
});

test('shows the selected count with the clear control for two or more', () => {
	const { container } = render(LibraryPills, { music: music() });

	expect(clearControl(container).textContent?.trim()).toBe('2 selected · clear');
});

test('a press toggles the pressed pill', () => {
	const { container } = render(LibraryPills, { music: music() });

	fireEvent.click(pill(container, 'g3'));

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: {
			command: 'setCollections',
			libIdx: 2,
			groupIds: ['g1', 'g2', 'g3'],
		},
	});
});

test('an Alt-press solos the pressed pill', () => {
	const { container } = render(LibraryPills, { music: music() });

	fireEvent.click(pill(container, 'g3'), { altKey: true });

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'setCollections', libIdx: 2, groupIds: ['g3'] },
	});
});

test('the clear control keeps the first selected pill in pill order', () => {
	const { container } = render(LibraryPills, { music: music() });

	fireEvent.click(clearControl(container));

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'setCollections', libIdx: 2, groupIds: ['g1'] },
	});
});

test.each([
	{
		case: 'Space toggles the focused pill',
		solo: false,
		groupIds: ['g1', 'g2', 'g3'],
	},
	{
		case: 'Alt+Space solos the focused pill',
		solo: true,
		groupIds: ['g3'],
	},
])('$case', async ({ solo, groupIds }) => {
	render(LibraryPills, { music: music() });

	requestLibraryPillToggle('g3', solo);
	await tick();

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'setCollections', libIdx: 2, groupIds },
	});
});

test('marks the pill the keys focus holds and sends nothing for it', async () => {
	const { container } = render(LibraryPills, { music: music() });

	focusLibraryPill('g2');
	await tick();
	expect(pill(container, 'g2').hasAttribute('data-library-pill-focused')).toBe(true);

	focusLibraryPill('g3');
	await tick();
	expect(pill(container, 'g3').hasAttribute('data-library-pill-focused')).toBe(true);
	expect(pill(container, 'g2').hasAttribute('data-library-pill-focused')).toBe(false);
	expect(invokeMock).not.toHaveBeenCalled();
});
