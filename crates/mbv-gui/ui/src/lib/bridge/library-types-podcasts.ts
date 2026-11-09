// The Podcasts DTOs of the GUI bridge contract (openspec change
// add-gui-library-panel, design D12, row 10.1). Each type mirrors one serde
// DTO in crates/mbv-gui/src/bridge/frame/library/podcasts.rs field for field:
// the names are the serde wire names, so a rename on the Rust side is a
// rename here. `library-types.ts` re-exports this family with the rest of the
// Library's wire types, so every call site keeps importing from the one
// bridge module.

import type { GuiHomeRowState, GuiLibraryArt } from './library-types';
import type { GuiPodcastPill } from './library-types';

/** The Podcasts destination's content (design D12, row 10.1), present while
 * an Audiobookshelf podcast library is the active destination: the
 * single-select pill row of Latest and one pill per show, the active pill's
 * episode rows in the date gutter, the core's selected episode, and its Hero
 * with the parent show's Square cover. No letter rail rides here — the list
 * is sorted by date — and no Workspace. */
export interface GuiPodcastsFrame {
	/** The Podcasts library index the Podcasts commands name (design D2,
	 * D12). */
	libIdx: number;
	/** The pill row in paint order: Latest, then each show by title. */
	pills: GuiPodcastsPill[];
	/** Whether the active view's episodes are still landing: the show
	 * catalog's pages or that view's own episode fetches. */
	loading: boolean;
	/** Whether the catalog fetch failed. */
	failed: boolean;
	/** The active pill's episode rows: Latest merges every listed show's
	 * episodes, a show pill holds that one show's, both newest first with
	 * undated episodes last. */
	rows: GuiPodcastsRow[];
	/** The core's selected episode (design D2, D12), as the frame painted it:
	 * a select, play, or enqueue command sends it back. The first row is the
	 * Hero default. */
	selected: string | null;
	/** The selected episode's Hero facts; `null` while no episode is
	 * selected. */
	hero: GuiPodcastsHero | null;
}

/** One pill of the Podcasts pill row (design D12, row 10.1): single-select. */
export interface GuiPodcastsPill {
	/** The pill's selection: the set-pill command carries it back. */
	pill: GuiPodcastPill;
	/** The pill's label: `Latest`, or the show's title. */
	name: string;
	/** Whether the core's stored pill is this one. */
	selected: boolean;
	/** Whether the pill is a collection pill (multi-select) rather than a
	 * single-select one. Every Podcasts pill is single-select. */
	collection: boolean;
}

/** One row of the Podcasts list (design D12, row 10.1). The identity is the
 * episode's stable `(library_item_id, episode_id)` pair as one opaque string
 * a select, play, or enqueue command sends, never a row position. */
export interface GuiPodcastsRow {
	/** The episode's stable identity, as one opaque string. */
	id: string;
	/** The episode's display title. */
	title: string;
	/** The parent show's title. */
	subtitle: string | null;
	/** The gutter date, `day month`; `null` on an undated episode. */
	date: string | null;
	/** The row's played state, with the resume percent on a partly played
	 * episode. */
	state: GuiHomeRowState;
}

/** The selected episode's Hero facts (design D12, row 10.1): the TUI
 * producer's facts — the episode title, the show, the duration, and the
 * publish date — the cleaned description, and the parent show's Square
 * cover. */
export interface GuiPodcastsHero {
	/** The hero's title: the episode title. */
	title: string;
	/** The meta rows in display order: the show title, the duration, and the
	 * publish date. */
	metaRows: string[];
	/** Which meta row paints the duration role, when one does. */
	durationRow: number | null;
	/** Which meta row paints the progress role; `null` on every episode. */
	progressRow: number | null;
	/** The cleaned description text, when the episode carries one. */
	overview: string | null;
	/** The artwork box the Hero reserves: the episode artwork policy's
	 * Square. */
	artShape: 'landscape' | 'square' | 'portrait';
	art: GuiLibraryArt | null;
}
