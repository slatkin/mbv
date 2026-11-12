// The Audiobooks destination's wire types (add-gui-library-panel design D12,
// row 9.1), split out of library-types.ts; re-exported there.

import type { GuiArtRef, GuiMetaRow } from './library-types';

/** The Audiobooks destination's list part (design D12, row 9.1, D8): the flat
 * book list with its rail letters and the catalog's load facts. No pill rides
 * here — the letter rail replaces the TUI's surname buckets. */
export interface GuiAudiobooksList {
	/** The Audiobooks library index the Audiobooks commands name (design
	 * D2, D12). */
	libIdx: number;
	/** Whether the book catalog has not landed yet. */
	loading: boolean;
	/** Whether the catalog fetch failed. */
	failed: boolean;
	/** The catalog's books in the TUI's sort order: author sort key, then
	 * title. */
	rows: GuiAudiobooksRow[];
}

/** The Audiobooks destination's detail part (design D8): the core's selected
 * book, the playing book, and its Hero with the chapters Workspace. */
export interface GuiAudiobooksDetail {
	/** Whether the selected book's detail fetch has not landed yet: the
	 * Workspace's loading state. */
	workspaceLoading: boolean;
	/** The core's selected book (design D2, D12), as the frame painted it:
	 * a select command sends it back. The first book is the Hero default. */
	selected: string | null;
	/** The Audiobooks book playing at the playhead (`gui-change-driven-sections`
	 * design D5): its stable `library_item_id`, the fact the window compares
	 * with the Hero's book before it marks a chapter row from the position.
	 * `null` while no book plays or the active slot is pending. */
	playingBookId: string | null;
	/** The selected book's Hero facts; `null` while no book is selected. */
	hero: GuiAudiobooksHero | null;
}

/** One row of the book list (design D12, row 9.1). The identity is the
 * book's stable `library_item_id` a select or play command sends, never a
 * row position. */
export interface GuiAudiobooksRow {
	/** The book's stable `library_item_id`. */
	id: string;
	/** The book's display title. */
	title: string;
	/** The author display credit. */
	subtitle: string | null;
	/** The book's total duration, the TUI row's duration gutter; empty when
	 * unknown. */
	duration: string;
	/** The row's rail letter, or `#` for a digit- or symbol-led author sort
	 * key. */
	sortLetter: string;
}

/** The book Hero's facts (design D12, row 9.1): the TUI's facts — title,
 * author, duration, `Finished` or `N%` — the portrait cover artwork, and
 * the chapters as the Workspace. */
export interface GuiAudiobooksHero {
	title: string;
	/** The meta rows in display order: the author, the duration, and the
	 * `Finished` or `N%` progress row. */
	metaRows: GuiMetaRow[];
	durationRow: number | null;
	progressRow: number | null;
	/** The cleaned description, when the catalog record carries one: the
	 * overview the TUI's Hero paints. */
	overview: string | null;
	/** The artwork box the Hero reserves: the book artwork policy's
	 * portrait. */
	artShape: 'landscape' | 'square' | 'portrait';
	art: GuiArtRef | null;
	/** The book's chapter rows; `null` while the book's detail has not
	 * landed, which the frame's `workspaceLoading` names. */
	workspace: GuiAudiobooksWorkspace | null;
}

/** The book Hero's Workspace (design D12, row 9.1): the book's chapter rows
 * — or its `Part N` rows when the book holds no chapters — in the TUI's
 * order. */
export interface GuiAudiobooksWorkspace {
	rows: GuiAudiobooksChapterRow[];
}

/** One chapter row of the Hero's Workspace (design D12, row 9.1). The
 * chapter value is the stable Service discriminator the TUI's chapter
 * activation addresses — the chapter number on a chapter row, the audio-part
 * index on a `Part N` row — never a display position. */
export interface GuiAudiobooksChapterRow {
	/** The row's stable Service discriminator. */
	chapter: number;
	/** The row's label: the chapter's title, or `Part N`. */
	title: string;
	/** The row's duration, the TUI's duration gutter; empty when unknown. */
	duration: string;
	/** The chapter's start on the book's merged timeline, in ticks (design
	 * D5). The window marks the row whose span holds the predicted position;
	 * an audio-part row carries no span — its `startTicks` and `endTicks`
	 * are both `0`, and an empty span never holds one. */
	startTicks: number;
	/** The chapter's end on the book's merged timeline, in ticks (design
	 * D5). The span's end is exclusive. */
	endTicks: number;
}
