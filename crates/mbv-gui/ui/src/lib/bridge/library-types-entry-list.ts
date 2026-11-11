// The pill-scoped entry-list destinations' shared wire types (openspec
// change add-gui-library-panel, design D12, rows 10.1 and 11.1). The Feeds
// and Podcasts pill rows and entry rows are one wire shape each, mirrored by
// the shared `GuiPillFrame` and `GuiEntryRowFrame` serde DTOs in
// crates/mbv-gui/src/bridge/frame/library/entry_list.rs; each destination's
// `library-types-*.ts` file aliases them over its own pill DTO. The names
// are the serde wire names, so a rename on the Rust side is a rename here.

import type { GuiHomeRowState } from './library-types-home';

/** One pill of a pill-scoped destination's pill row (design D12, rows 10.1
 * and 11.1): single-select. */
export interface GuiLibraryPill<P> {
	/** The pill's selection: the set-pill command carries it back. */
	pill: P;
	/** The pill's label: `Latest`, or the scope's own name. */
	name: string;
	/** Whether the core's stored pill is this one. */
	selected: boolean;
	/** Whether the pill is a collection pill (multi-select) rather than a
	 * single-select one. Every pill-scoped destination's pills are
	 * single-select. */
	collection: boolean;
}

/** One row of a pill-scoped destination's entry list (design D12, rows 10.1
 * and 11.1). The identity is the entry's stable Service identity, as one
 * opaque string a select, play, or enqueue command sends, never a row
 * position. */
export interface GuiLibraryEntryRow {
	/** The entry's stable identity, as one opaque string. */
	id: string;
	/** The entry's display title. */
	title: string;
	/** The row's secondary label — the fact that names the row's scope (the
	 * subscription, the show) in the merged Latest view. */
	subtitle: string | null;
	/** The gutter date, `day month`; `null` on an undated entry. */
	date: string | null;
	/** The row's played state, with the resume percent on a partly played
	 * entry. */
	state: GuiHomeRowState;
}
