// The Home DTOs of the GUI bridge contract (openspec change
// add-gui-library-panel, design D12, row 6.1). Each type mirrors one serde DTO
// in crates/mbv-gui/src/bridge/frame/library field for field: the names are
// the serde wire names, so a rename on the Rust side is a rename here.
// `library-types.ts` re-exports this family with the rest of the Library's
// wire types, so every call site keeps importing from the one bridge module.

import type { GuiArtRef, GuiMetaRow } from './library-types';

/** One Home row's stable identity (add-gui-library-panel design D12): the
 * identity the Home paths resolve, addressed by Service ids and never by a
 * row position. A select, play, or enqueue command carries it back as the
 * frame painted it. */
export type GuiHomeRowId =
	| { kind: 'continue'; id: string }
	| { kind: 'recent'; id: GuiHomeContentId };

/** The `Recently added` row's queue-item content id, mirroring
 * `mbv_queue.QueueItemContentId`'s serde form (`provider` and `value`,
 * snake_case struct fields as declared). */
export type GuiHomeContentId =
	| { provider: 'Emby'; value: string }
	| { provider: 'Feed'; value: string }
	| {
			provider: 'Audiobookshelf';
			value: { library_item_id: string; episode_id: string };
	  }
	| { provider: 'AudiobookshelfBook'; value: { library_item_id: string } }
	| { provider: 'Twitch'; value: string };

/** The Home destination's list part (design D12, D8): the flat list's rows
 * and its load fact. No pill, no letter rail, and no Workspace ride here. */
export interface GuiHomeList {
	/** Whether the Home content's first fetch has not landed yet: the list
	 * paints its loading line. */
	loading: boolean;
	/** The flat list's rows in paint order (design D12): the two sections'
	 * heading rows and item rows, with the spacer between two non-empty
	 * sections. */
	rows: GuiHomeRow[];
}

/** The Home destination's detail part (design D8): the core's selected row and
 * its Hero. The top bar marks the Home icon current while Home is the active
 * destination. */
export interface GuiHomeDetail {
	/** The core's selected row (design D2, D12), as the frame painted it:
	 * a select, play, or enqueue command sends it back. */
	selected: GuiHomeRowId | null;
	/** The selected row's Hero facts; `null` while no row is selected. */
	hero: GuiHomeHero | null;
}

/** One row of the flat Home list (design D12). */
export type GuiHomeRow =
	| {
			/** A section heading row (`Continue watching`, `Recently added`). */
			row: 'heading';
			/** The section's name, painted as-is. */
			text: string;
	  }
	| { row: 'spacer' }
	| { row: 'item'; id: GuiHomeRowId; title: string; subtitle: string | null; date: string | null; kind: GuiHomeRowKind; state: GuiHomeRowState; art: GuiArtRef | null };

/** The kind one item row paints (design D12): `collection` when the row
 * targets a container the play path refuses, `media` on a playable leaf. */
export type GuiHomeRowKind = 'collection' | 'media';

/** The state one item row paints (design D12): a resume position is
 * `active` with the bounded percentage, `played` without one is `played`,
 * otherwise `ordinary`. */
export type GuiHomeRowState =
	| 'ordinary'
	| 'played'
	| { active: { progress: number | null } };

/** The Hero's facts for the selected row (design D12). */
export interface GuiHomeHero {
	/** The hero's title. */
	title: string;
	/** The meta rows in display order. */
	metaRows: GuiMetaRow[];
	/** Which meta row paints the duration role, when one does. */
	durationRow: number | null;
	/** Which meta row paints the progress role, when one does. */
	progressRow: number | null;
	/** The cleaned overview text, when the item's payload carries one. */
	overview: string | null;
	/** The artwork box the Hero reserves (the artwork policy's arm). */
	artShape: 'landscape' | 'square' | 'portrait';
	/** The Hero's artwork, present exactly while the row's item obtains art
	 * from the image cache; the window paints its own placeholder while
	 * `null`. */
	art: GuiArtRef | null;
}
