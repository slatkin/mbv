// add-gui-library-panel row 4.3: the top bar paints one tab per presented
// destination, marks the current one, and a press sends the open-destination
// command with the tab's position (design D2). Phase 1 presents Music only.
// Row 12.1 adds the Search field: the dropdown paints the carried Search
// facts in the list-row look, the typing sends the debounced `searchQuery`
// command, a row press sends `searchActivate`, and the highlight survives a
// new results frame by row id, never object identity. The Tauri IPC is
// mocked; the asserted payloads are the serde wire shapes of
// bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { flushSync, tick } from 'svelte';
import { get } from 'svelte/store';
import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiSearchFrame, GuiSearchRow } from '$lib/bridge/types';
import {
	initialLibrarySearch,
	librarySearch,
	setSearchHighlight,
	setSearchScheduler,
} from '$lib/library/search';
import LibraryTopBar from './LibraryTopBar.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	librarySearch.set(initialLibrarySearch());
});

test('paints one tab per presented destination and marks the current one', () => {
	const { container } = render(LibraryTopBar, {
		tabs: [
			{ position: 1, label: 'Music' },
			{ position: 2, label: 'Shows' },
		],
		currentTab: 2,
	});
	const tabs = [...container.querySelectorAll('[data-library-tab]')];

	expect(tabs.map((tab) => tab.textContent?.trim())).toEqual(['Music', 'Shows']);
	expect(tabs.map((tab) => tab.getAttribute('aria-current'))).toEqual([null, 'page']);
});

test('the bar paints the dim canvas fill, the TUI BgDim slot', () => {
	const { container } = render(LibraryTopBar, {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
	});

	const bar = container.querySelector<HTMLElement>('[data-library-region="topBar"]');
	expect(bar?.style.background).toBe('var(--canvas-dim)');
});

test('a press opens the destination tab at its position', () => {
	const { container } = render(LibraryTopBar, {
		tabs: [
			{ position: 1, label: 'Music' },
			{ position: 3, label: 'Audiobooks' },
		],
		currentTab: 1,
	});

	fireEvent.click(container.querySelectorAll('[data-library-tab]')[1] as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'openDestination', position: 3 },
	});
});

// add-gui-library-panel row 6.2: the canvas's Home icon sits before the tabs
// (design D12), opens Home's own position, and marks itself current while the
// Home content is present — the tabs keep marking their destination.
test('a press on the Home icon opens Home', () => {
	const { container } = render(LibraryTopBar, {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
	});

	fireEvent.click(container.querySelector('[data-library-home]') as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'openDestination', position: 0 },
	});
});

test('the Home icon is current only while the Home content is present', () => {
	// A realistic frame (design D12): the destinations are exclusive, so the
	// frame's `currentTab` stays null while the Home content is present.
	const { container } = render(LibraryTopBar, {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: null,
		homeActive: true,
	});

	expect(
		container.querySelector('[data-library-home]')?.getAttribute('aria-current'),
	).toBe('page');
	expect(
		container.querySelector('[data-library-tab]')?.getAttribute('aria-current'),
	).toBeNull();
});

// ---- Search (row 12.1) -----------------------------------------------------

/** Two found rows: the dropdown's rows in the list-row look. */
function results(fresh: boolean): GuiSearchFrame {
	const rows: GuiSearchRow[] = [
		{ id: 'r1', itemType: 'Artist', title: 'Beastie Boys', subtitle: 'Artist' },
		{ id: 'r2', itemType: 'Album', title: 'Licensed To Ill', subtitle: 'Beastie Boys' },
	];
	// Each call builds fresh row objects that share only ids: the highlight
	// must never depend on object identity.
	const rowsOut = fresh ? rows.map((row) => ({ ...row })) : rows;
	return { query: 'ba', loading: false, failed: false, results: rowsOut };
}

/** Thedebounce seam becomes a recorded timer the tests flush. */
function flush(): () => void {
	let pending: (() => void) | null = null;
	setSearchScheduler((send) => {
		pending = send;
		return {
			cancel: () => {
				pending = null;
			},
		};
	});
	return () => {
		const send = pending;
		pending = null;
		send?.();
	};
}

test('a closed Search section paints the field and no dropdown', () => {
	const { container } = render(LibraryTopBar, {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
		search: null,
	});

	expect(container.querySelector('[data-library-search-input]')).toBeTruthy();
	expect(container.querySelector('[data-library-search-results]')).toBeNull();
});

test('the dropdown paints the rows in the list-row look, with the first row highlighted', async () => {
	const { container } = render(LibraryTopBar, {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
		search: results(false),
	});
	await tick();

	const rows = [...container.querySelectorAll('[data-library-search-row]')];
	expect(rows.map((row) => row.textContent)).toEqual([
		'Beastie Boys Artist',
		'Licensed To Ill Beastie Boys',
	]);
	// No held highlight: the first row marks itself (row 12.1).
	expect(get(librarySearch).highlight).toBe('r1');
	expect(
		container.querySelector('[data-library-search-highlight]')?.getAttribute('data-library-search-row'),
	).toBe('r1');
});

test('the highlight survives a rerendered results frame by row id, not object identity', async () => {
	setSearchHighlight('r2');
	const { container, rerender } = render(LibraryTopBar, {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
		search: results(false),
	});

	// A new frame: fresh row objects, same ids, same highlight.
	await rerender({
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
		search: results(true),
	});

	expect(
		container.querySelector('[data-library-search-highlight]')?.getAttribute('data-library-search-row'),
	).toBe('r2');
});

test('the highlight falls to the first row when a new frame drops its id', async () => {
	setSearchHighlight('r2');
	const dropped: GuiSearchFrame = {
		query: 'ba',
		loading: false,
		failed: false,
		results: [{ id: 'r1', itemType: 'Artist', title: 'Beastie Boys', subtitle: 'Artist' }],
	};
	const { container, rerender } = render(LibraryTopBar, {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
		search: dropped,
	});

	await rerender({
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
		search: { ...dropped, results: [] },
	});

	expect(get(librarySearch).highlight).toBeNull();
	expect(container.querySelector('[data-library-search-highlight]')).toBeNull();
});

test('the dropdown paints the loading and failed states while the query lands', () => {
	const loading = { query: 'ba', loading: true, failed: false, results: [] };
	const failed = { query: 'ba', loading: false, failed: true, results: [] };

	const ok = render(LibraryTopBar, {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
		search: loading,
	});
	expect(ok.container.querySelector('[data-library-search-loading]')).toBeTruthy();

	const bad = render(LibraryTopBar, {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
		search: failed,
	});
	expect(bad.container.querySelector('[data-library-search-failed]')).toBeTruthy();
});

test('the typing schedules the query send, and the send carries the settled text', () => {
	const run = flush();
	const { container } = render(LibraryTopBar, {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
		search: null,
	});
	const input = container.querySelector('[data-library-search-input]') as HTMLInputElement;

	fireEvent.input(input, { target: { value: 'ba' } });
	expect(invokeMock).not.toHaveBeenCalled();

	run();
	flushSync();

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'searchQuery', query: 'ba' },
	});
});

test('a press on a result row sends searchActivate, and never searchClose', () => {
	const { container } = render(LibraryTopBar, {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
		search: results(false),
	});

	fireEvent.click(container.querySelectorAll('[data-library-search-row]')[1] as HTMLElement);

	expect(invokeMock).toHaveBeenCalledTimes(1);
	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'searchActivate', id: 'r2', itemType: 'Album' },
	});
});

test('a press on the Settings control opens the screen', async () => {
	// add-gui-settings-screen row 9.11: the top bar's Settings control is the
	// pointer's way into the screen (design D11 *Mouse-centric*), so a press
	// sends `open` — never the keyboard's `selectSection`.
	const { container } = render(LibraryTopBar, {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
	});

	const button = container.querySelector('[data-library-settings]') as HTMLElement;
	expect(button.getAttribute('aria-label')).toBe('Settings');

	await fireEvent.click(button);

	expect(invokeMock).toHaveBeenCalledWith('settings', {
		command: { command: 'open' },
	});
});

test('unmounting the top bar retracts the mount and focus facts', async () => {
	// Review of row 12.1, finding P1: the bar's teardown retracts the focus
	// fact with the mount fact, so an arrangement change never leaves the
	// keyboard armed on a field whose element nothing paints.
	const { unmount } = render(LibraryTopBar, {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
	});
	await tick();
	expect(get(librarySearch).mounted).toBe(true);

	librarySearch.update((s) => ({ ...s, focused: true }));
	await tick();
	expect(get(librarySearch).focused).toBe(true);

	unmount();

	expect(get(librarySearch).mounted).toBe(false);
	expect(get(librarySearch).focused).toBe(false);
});

test('the field follows its focus fact into DOM focus, and its own focus event checks in', async () => {
	const { container } = render(LibraryTopBar, {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: 1,
	});

	// The key router's Ctrl+K fact (design D3, step 3) moves the DOM focus.
	librarySearch.update((s) => ({ ...s, focused: true }));
	await tick();
	const input = container.querySelector('[data-library-search-input]') as HTMLInputElement;
	expect(document.activeElement).toBe(input);

	// The focused field answers where the Esc close will leave it.
	fireEvent.blur(input);
	await tick();
	expect(get(librarySearch).focused).toBe(false);
});
