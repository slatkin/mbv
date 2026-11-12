// The Emby-library destination's wire types (add-gui-library-panel design D12,
// row 8.1), split out of library-types.ts; re-exported there.

import type { GuiArtRef, GuiMetaRow } from './library-types';

/** The feed-group pill a press selects (design D12, row 8.1): `New` (the
 * destination Latest view), `All`, or one root folder by its stable Service
 * id. */
export type GuiEmbyFeedGroup =
	| { kind: 'latest' }
	| { kind: 'all' }
	| { kind: 'folder'; id: string };

/** The plain Emby-library destination's content (design D12, row 8.1),
 * present while a plain Emby library is the active destination: the
 * feed-group pills, the active list's rows, the core's selected row, and its
 * Hero. */
export interface GuiEmbyFrame {
	/** The plain Emby-library index the Emby commands name (design D2). */
	libIdx: number;
	/** The library's display name: the list header's title, so the column
	 * names the library it holds. */
	libraryName: string;
	/** The feed-group mode's pills in pill order; empty in plain mode,
	 * whose letter rail replaces the TUI's pill row. */
	pills: GuiEmbyPill[];
	/** The selected collection's name the Library Status bar's location
	 * reads (design D10); empty in plain mode. */
	collectionNames: string[];
	/** Whether the library presents the feed-group mode
	 * (`feed_view_libraries`). */
	feedGroup: boolean;
	/** Whether the active list has not landed yet. */
	loading: boolean;
	/** Whether the plain mode's root-list fetch failed. */
	failed: boolean;
	/** The active list's rows in the list's order. */
	rows: GuiEmbyRow[];
	/** The core's selected row (design D2, D12), as the frame painted it:
	 * a select command sends it back. The first row is the Hero default. */
	selected: string | null;
	/** The selected row's Hero facts; `null` while no row is selected. */
	hero: GuiEmbyHero | null;
}

/** One pill of a feed-view library's pill row (design D12, row 8.1):
 * single-select. */
export interface GuiEmbyPill {
	/** The group the pill selects: the set-feed-group command carries it. */
	group: GuiEmbyFeedGroup;
	/** The pill's label: `All`, or the folder's name. */
	name: string;
	/** Whether the feed state's selection is this pill's group. */
	selected: boolean;
	/** Whether the pill is a collection pill (multi-select) rather than a
	 * single-select one. Every Emby-library pill is single-select. */
	collection: boolean;
}

/** One row's kind (design D12, row 8.1): `collection` when the row is a
 * folder or a boxset the folder-play rule expands, `media` on a playable
 * leaf. */
export type GuiEmbyRowKind = 'collection' | 'media';

/** One row of the plain Emby-library list (design D12, row 8.1). The
 * identity is the stable Service id a select or play command sends, never a
 * row position. */
export interface GuiEmbyRow {
	/** The row's stable identity. */
	id: string;
	/** The row's primary label, with a folder's item-count decoration. */
	title: string;
	/** The row's secondary label (plain mode): the year, else the runtime. */
	subtitle: string | null;
	/** The gutter date (feed-group mode): the production year. */
	date: string | null;
	/** The row's rail letter (design D7), or `#`; set only in plain mode. */
	sortLetter: string | null;
	/** The row's kind. */
	kind: GuiEmbyRowKind;
}

/** The Hero's facts for the selected row (design D12, row 8.1), with no
 * Workspace: a plain Emby-library Hero carries none. */
export interface GuiEmbyHero {
	/** The item's kind word for the Hero's kicker (design D17): `movie`,
	 * `video`, as the projection carries it. */
	kind: string;
	title: string;
	metaRows: GuiMetaRow[];
	durationRow: number | null;
	progressRow: number | null;
	overview: string | null;
	/** The artwork box the Hero reserves (the artwork policy's declared
	 * arm). */
	artShape: 'landscape' | 'square' | 'portrait';
	art: GuiArtRef | null;
	/** The movie's credits table (design D17, Q7): the director first,
	 * then the cast, as the TUI's credits table paints them. */
	credits: GuiEmbyHeroCredit[];
	/** The IMDb link's target (design D17, Q7), when the item carries
	 * one; the `IMDb` meta row links to it. */
	imdbUrl: string | null;
}

/** One credit of the Hero's credits table (design D17, Q7), mirroring
 * `GuiEmbyHeroCreditFrame` field for field. */
export interface GuiEmbyHeroCredit {
	/** The person's name. */
	name: string;
	/** The role painted beside the name: the person's role, else their
	 * kind (a director's `Director`). */
	role: string;
}
