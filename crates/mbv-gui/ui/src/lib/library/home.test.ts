// add-gui-library-panel row 6.2: the Home list's interaction state (design
// D1, D12) — the cursor's settle selects the row it rests on through the
// select command with the row's opaque identity, and the scroll and measure
// reports keep the offset inside the list's range. The Tauri IPC is mocked;
// the asserted payloads are the serde wire shapes of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiHomeFrame, GuiHomeRow, GuiLibraryContent } from '$lib/bridge/types';
import { homeRowKey, HOME_ITEM_ROW_HEIGHT } from './home-list';
import { carryLibraryContent, initialLibraryUi, libraryUi } from './state';
import {
	homeUi,
	initialHomeUi,
	measureHomeList,
	moveHomeListCursor,
	pressHomeRow,
} from './home';
import { buildHomeRows } from './home-list';

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	libraryUi.set(initialLibraryUi());
	homeUi.set(initialHomeUi());
});

/** The item row's narrowed type: the union's arm a populated row carries. */
type ItemRow = Extract<GuiHomeRow, { row: 'item' }>;

/** The `Continue watching` rows the fixture carries. */
const CONTINUED: ItemRow = {
	row: 'item',
	id: { kind: 'continue', id: 'e1' },
	title: 'Better Call Saul',
	subtitle: null,
	date: null,
	kind: 'media',
	state: 'ordinary',
	art: null,
};

/** The `Recently added` row the cursor steps to. */
const ADDED: ItemRow = {
	row: 'item',
	id: { kind: 'recent', id: { provider: 'Feed', value: 'f1' } },
	title: 'A Feed Entry',
	subtitle: null,
	date: '12 May',
	kind: 'media',
	state: 'ordinary',
	art: null,
};

/** The carried content while Home is active (design D12). */
function content(): GuiLibraryContent {
	return {
		tabs: [{ position: 1, label: 'Music' }],
		currentTab: null,
		music: null,
		shows: null,
		emby: null,
		audiobooks: null,
		podcasts: null,
		feeds: null,
		rowMenu: null,
		home: {
			rows: [
				{ row: 'heading', text: 'Continue watching' },
				CONTINUED,
				{ row: 'spacer' },
				{ row: 'heading', text: 'Recently added' },
				ADDED,
			],
			selected: CONTINUED.id,
			hero: null,
		} satisfies GuiHomeFrame,
	};
}

/** Carry the Home content, so the panel's presence fact holds and the row
 * model has its rows (design D2, D12). */
function carry(): void {
	carryLibraryContent({ revision: 1, search: null, content: content() });
}

test('a cursor move that settles on a row selects it by its opaque id', () => {
	carry();
	homeUi.set({ ...initialHomeUi(), cursor: homeRowKey(CONTINUED.id) });

	moveHomeListCursor('down');

	expect(get(homeUi).cursor).toBe(homeRowKey(ADDED.id));
	expect(get(homeUi).scrollTop).toBe(0);
	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'homeSelect', row: ADDED.id },
	});
});

test('a cursor move past a section scrolls the row into view', () => {
	carry();
	measureHomeList(HOME_ITEM_ROW_HEIGHT * 3);
	homeUi.set({ ...get(homeUi), cursor: homeRowKey(CONTINUED.id) });

	const rows = buildHomeRows(content().home?.rows ?? []);
	const adRow = rows.find((row) => row.id === homeRowKey(ADDED.id)) as {
		top: number;
		height: number;
	};

	moveHomeListCursor('end');

	expect(get(homeUi).cursor).toBe(homeRowKey(ADDED.id));
	expect(get(homeUi).scrollTop).toBe(adRow.top + adRow.height - HOME_ITEM_ROW_HEIGHT * 3);
});

// The press is the selection's half only (design D12): it selects, and Enter
// or the double-click is the play's. A heading or spacer names no item, so
// its press changes nothing.
test('a press on a heading selects nothing', () => {
	carry();
	const heading = buildHomeRows(content().home?.rows ?? [])[0];

	pressHomeRow(heading);

	expect(invokeMock).not.toHaveBeenCalledWith('library', expect.anything());
});
