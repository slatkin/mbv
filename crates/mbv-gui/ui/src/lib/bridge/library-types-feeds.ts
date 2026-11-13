// The Feeds DTOs of the GUI bridge contract (openspec change
// add-gui-library-panel, design D12, row 11.1). Each type mirrors one serde
// DTO in crates/mbv-gui/src/bridge/frame/library/feeds.rs field for field:
// the names are the serde wire names, so a rename on the Rust side is a
// rename here. The pill row and the entry rows are the shared entry-list
// types (`library-types-entry-list.ts`, mirrored by the shared serde DTOs in
// `bridge/frame/library/entry_list.rs`) over this destination's pill DTO.
// `library-types.ts` re-exports this family with the rest of the Library's
// wire types, so every call site keeps importing from the one bridge module.

import type { GuiLibraryEntryRow, GuiLibraryPill } from './library-types-entry-list';
import type { GuiMetaRow } from './library-types';

/** The Feeds destination's list part (design D12, row 11.1, D8): the
 * single-select pill row of Latest and one pill per subscription, and the
 * active pill's entry rows in the date gutter. A Client holds one feed tab, so
 * no library index names it. No letter rail rides here — the list is sorted by
 * date — no state filter, and no Workspace. */
export interface GuiFeedsList {
	/** The pill row in paint order: Latest, then each configured subscription
	 * by its name. */
	pills: GuiFeedsPill[];
	/** Whether the tab's fetch results are still landing. */
	loading: boolean;
	/** Whether a fetch failed. The feed tab keeps no failure state — a failed
	 * subscription flashes the TUI's toast and keeps its previous entries —
	 * so this is always `false`. */
	failed: boolean;
	/** The active pill's entry rows: Latest merges every subscription's
	 * entries, a subscription pill holds that one subscription's, both newest
	 * first with undated entries last. */
	rows: GuiFeedsRow[];
}

/** The Feeds destination's detail part (design D8): the core's selected entry
 * and its Hero, with the placeholder artwork's shape. */
export interface GuiFeedsDetail {
	/** The core's selected entry (design D2, D12), as the frame painted it: a
	 * select, play, or enqueue command sends it back. The first row is the
	 * Hero default. */
	selected: string | null;
	/** The selected entry's Hero facts; `null` while no entry is selected. */
	hero: GuiFeedsHero | null;
}

/** One pill of the Feeds pill row (design D12, row 11.1): the shared
 * single-select pill frame over the Feeds pill. */
export type GuiFeedsPill = GuiLibraryPill<GuiFeedPill>;

/** One Feeds pill (design D12, row 11.1): the merged Latest view, or one
 * subscription by its URL. */
export type GuiFeedPill = { kind: 'latest' } | { kind: 'subscription'; id: string };

/** One row of the Feeds list (design D12, row 11.1) — the shared entry row.
 * The identity is the entry's `(subscription url, guid)` pair as one opaque
 * string a select, play, or enqueue command sends, never a row position: two
 * subscriptions can publish the same `guid`. */
export type GuiFeedsRow = GuiLibraryEntryRow;

/** The selected entry's Hero facts (design D12, row 11.1): the TUI
 * `hero_content_feed` producer's title and duration behind the subscription
 * name, and the placeholder artwork's shape. A feed entry has no artwork
 * source and no overview, so neither rides here. */
export interface GuiFeedsHero {
	/** The hero's title: the entry title. */
	title: string;
	/** The subscription's name: the largest unit, painted as the header's title. */
	parent?: string | null;
	/** The meta rows in display order: the duration when the entry declares
	 * one (the subscription name is the parent). */
	metaRows: GuiMetaRow[];
	/** Which meta row paints the duration role, when one does. */
	durationRow: number | null;
	/** Which meta row paints the progress role; `null` on every entry. */
	progressRow: number | null;
	/** The artwork box the Hero reserves: `square` for a podcast feed,
	 * `landscape` otherwise. The Hero paints its placeholder, because a feed
	 * entry has no artwork source. */
	artShape: 'landscape' | 'square' | 'portrait';
}

/** The Feeds destination's commands (design D12, row 11.1), carried in the
 * Library command union with the rest of the panel's requests. A Client holds
 * one feed tab, so no library index rides them. */
export type GuiFeedsLibraryCommand =
	| {
			/** Set the Feeds destination's pill (design D12, row 11.1): Latest,
			 * or one subscription by its URL. */
			command: 'feedsSetPill';
			pill: GuiFeedPill;
	  }
	| {
			/** Select the Feeds entry the Hero shows (design D12, row 11.1):
			 * the row id the frame carried. */
			command: 'feedsSelect';
			row: string;
	  }
	| {
			/** Play a Feeds entry (design D12, row 11.1): Enter on the row, or
			 * the double-click a front end sends with this same command. */
			command: 'feedsPlay';
			itemId: string;
	  }
	| {
			/** Add a Feeds entry to the queue (design D12, row 11.1). */
			command: 'feedsQueue';
			itemId: string;
	  };
