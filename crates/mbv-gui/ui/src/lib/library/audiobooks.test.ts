// add-gui-library-panel row 9.1: the book list's interaction state (design
// D12) — a press selects the book the Hero shows, the play path and a queue
// send their commands with the book's stable `library_item_id`, and a rail
// jump puts the cursor on the letter's first book. The Tauri IPC is mocked;
// the asserted payloads are the serde wire shapes of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { get } from 'svelte/store';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiAudiobooksFrame, GuiAudiobooksRow, GuiLibraryContent } from '$lib/bridge/types';
import { audiobooksUi, initialAudiobooksUi, jumpAudiobooksToLetter, playAudiobooksRow, pressAudiobooksRow, queueAudiobooksRow } from './audiobooks';
import { buildAudiobooksRows } from './audiobooks-list';
import { carryLibraryContent, libraryUi } from './state';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	audiobooksUi.set(initialAudiobooksUi());
	libraryUi.update((ui) => ({ ...ui, heroDismissed: true, itemMenuOpen: true }));
	carryLibraryContent(null);
});

/** The catalog's book rows: two books under `A` and one under `B`. */
const CATALOG: GuiAudiobooksRow[] = [
	{ id: 'b1', title: 'Aftermath', subtitle: 'Bill Wyman', duration: '4:32', sortLetter: 'A' },
	{ id: 'b2', title: 'Audition', subtitle: 'Ryu Murakami', duration: '2:14', sortLetter: 'A' },
	{ id: 'b3', title: 'Blindsight', subtitle: 'Peter Watts', duration: '10:02', sortLetter: 'B' },
];

/** The carried Audiobooks content with `selected` picked (row 9.1). */
function frame(selected: string | null = 'b1'): GuiAudiobooksFrame {
	return {
		libIdx: 4,
		loading: false,
		failed: false,
		workspaceLoading: false,
		rows: CATALOG,
		selected,
		hero: null,
	};
}

/** Carry the content and paint the list's cursor at `cursor`. */
function carry(selected: string | null = 'b1', cursor: string | null = null): void {
	const content: GuiLibraryContent = {
		tabs: [{ position: 4, label: 'Audiobooks' }],
		currentTab: 4,
		music: null,
		home: null,
		shows: null,
		emby: null,
		audiobooks: frame(selected),
		podcasts: null,
	};
	carryLibraryContent({ revision: 1, content });
	audiobooksUi.set({ ...initialAudiobooksUi(), cursor });
}

test('a press selects the book and repaints the Hero overlay', () => {
	carry('b1');
	const rows = buildAudiobooksRows(CATALOG);
	const row = rows.find((candidate) => candidate.kind === 'item' && candidate.row.id === 'b3');

	pressAudiobooksRow(row as NonNullable<typeof row>);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'absSelect', libIdx: 4, row: 'b3' },
	});
});

test('a press on a heading selects nothing', () => {
	carry();
	const heading = buildAudiobooksRows(CATALOG)[0];

	pressAudiobooksRow(heading);

	expect(invokeMock).not.toHaveBeenCalled();
});

test('a press repaints the Hero overlay and closes an open item menu', () => {
	carry();
	const row = buildAudiobooksRows(CATALOG)[1];

	pressAudiobooksRow(row);

	expect(get(libraryUi).heroDismissed).toBe(false);
	expect(get(libraryUi).itemMenuOpen).toBe(false);
});

test('playing a book sends absPlay with its id', () => {
	carry();

	playAudiobooksRow('b2');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'absPlay', libIdx: 4, itemId: 'b2' },
	});
});

test('queueing a book sends absQueue with its id', () => {
	carry();

	queueAudiobooksRow('b3');

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'absQueue', libIdx: 4, itemId: 'b3' },
	});
});

test('a rail jump puts the cursor on the letter\'s first book', () => {
	carry();

	jumpAudiobooksToLetter('B');

	expect(get(audiobooksUi).cursor).toBe('b3');
});
