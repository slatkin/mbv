// The Library wire types of the GUI bridge contract (openspec change
// add-gui-library-panel, design D2). Each type mirrors one serde DTO in
// crates/mbv-gui/src/bridge/ field for field: the names are the serde wire
// names, so a rename on the Rust side is a rename here. `types.ts` re-exports
// this family, so every call site keeps importing from the one bridge module.

/** The Library panel's section (add-gui-library-panel design D2). A frame
 * re-carries `content` only when `revision` changes; a `reportLayout` reload
 * re-carries it under the revision the window already holds, so the window
 * keeps its last carried content while `content` is `null`. */
export interface GuiLibraryFrame {
	revision: number;
	content: GuiLibraryContent | null;
}

/** The Library panel's content: the top bar's destination tabs and the
 * active destination's section. */
export interface GuiLibraryContent {
	/** The presented destination tabs in catalog order (design D2): one per
	 * Grouped Music and Shows library. The top bar paints only these — no
	 * stub for a destination the GUI does not present yet. */
	tabs: GuiLibraryTab[];
	/** The active tab's position while the active destination is one of the
	 * tabs (design D2): the tab the top bar marks current. `null` for Home,
	 * Feeds, and every destination the GUI does not present yet. */
	currentTab: number | null;
	/** The Grouped Music library's content while Music is active; `null` for a
	 * destination that presents no content yet. */
	music: GuiMusicFrame | null;
	/** The Home destination's content while Home is active (design D12);
	 * `null` while any other destination is active. The top bar marks the
	 * Home icon current exactly while this field is present. */
	home: GuiHomeFrame | null;
	/** The Shows destination's content while a Shows library is active
	 * (design D12, row 7.1); `null` while any other destination is active. */
	shows: GuiShowsFrame | null;
}

/** One destination tab of the top bar (design D2). */
export interface GuiLibraryTab {
	/** The tab position the open-destination command takes, where 0 is
	 * Home. */
	position: number;
	/** The tab's label: the library's display name. */
	label: string;
}

/** One request from the Library panel's window content (add-gui-library-panel
 * design D2): the wire shape of `GuiLibraryCommand` in bridge/library.rs, whose
 * variants and field names this mirrors. Item identities are stable Service
 * ids, never row positions. */
export type GuiLibraryCommand =
	| {
			/** Open the destination tab at `position`: the tab position the
			 * frame's destination tabs carry, where 0 is Home. */
			command: 'openDestination';
			position: number;
	  }
	| {
			/** Select the one collection group of the Grouped Music library
			 * `libIdx`: one group is always selected (design D6). */
			command: 'selectCollection';
			libIdx: number;
			groupId: string;
	  }
	| { command: 'showDetail'; itemId: string }
	| { command: 'openAlbum'; albumId: string }
	| { command: 'takeFocus' }
	| {
			/** Take Panel focus for the Queue panel: Tab across the panel
			 * boundary's far end (design D3, D4). The mirror of
			 * `takeFocus`. */
			command: 'takeQueueFocus';
	  }
	| { command: 'play'; itemId: string }
	| { command: 'playNext'; itemId: string }
	| { command: 'addToQueue'; itemId: string }
	| { command: 'shuffleArtist'; albumIds: string[] }
	| { command: 'menuEntry'; itemId: string; index: number }
	| { command: 'rowMenu'; itemId: string }
	| {
			/** Select a Home row (design D12): the row the Hero shows. The
			 * core keeps the selection, so the frame paints it. */
			command: 'homeSelect';
			row: GuiHomeRowId;
	  }
	| {
			/** Play a Home row (design D12). */
			command: 'homePlay';
			row: GuiHomeRowId;
	  }
	| {
			/** Enqueue a Home row after the active slot (design D12): the Owner
			 * owns the queue, and the row resolves through the same Home
			 * enqueue path the add carries. */
			command: 'homePlayNext';
			row: GuiHomeRowId;
	  }
	| {
			/** Enqueue a Home row (design D12). */
			command: 'homeQueue';
			row: GuiHomeRowId;
	  }
	| {
			/** Set the Shows destination's view mode (design D12, row 7.1):
			 * the root level loads through the TUI's own mode load. */
			command: 'showsSetMode';
			libIdx: number;
			mode: GuiShowsMode;
	  }
	| {
			/** Select a Shows row (design D12): the row the Hero shows —
			 * the All series mode's series, or a flat mode's episode or
			 * premiere row. */
			command: 'showsSelect';
			libIdx: number;
			row: string;
	  }
	| {
			/** Select the season whose episodes the Hero's Workspace shows
			 * (design D12). */
			command: 'showsSelectSeason';
			libIdx: number;
			seasonId: string;
	  }
	| {
			/** Play a Shows episode (design D12): Enter on the row, or the
			 * double-click a front end sends with this same command. */
			command: 'showsPlay';
			libIdx: number;
			itemId: string;
	  }
	| {
			/** Play a Shows episode next (design D12). */
			command: 'showsPlayNext';
			libIdx: number;
			itemId: string;
	  }
	| {
			/** Add a Shows episode to the queue (design D12). */
			command: 'showsQueue';
			libIdx: number;
			itemId: string;
	  };

/** The Music destination's content (design D5). */
export interface GuiMusicFrame {
	/** The Grouped Music library index the set-collections command names
	 * (design D2, D6): the library whose group pills these are. */
	libIdx: number;
	/** The group pills in the group level's order (design D6). */
	pills: GuiMusicPill[];
	/** The selected collection names in pill order (design D6): the names the
	 * list header and the Library Status bar location read, carried once for
	 * the whole panel. The Hero header's own copy rides in
	 * `hero.collections`, present exactly when the Hero is. */
	collectionNames: string[];
	/** Whether one of the selected groups' album lists has not landed yet. */
	loading: boolean;
	/** The failure line naming the selected groups whose list failed. */
	failure: string | null;
	/** The union of the selected groups' artists in artist sort-name order. */
	artists: GuiMusicArtist[];
	/** The Hero's facts; `null` while no album of the union is picked. */
	hero: GuiMusicHero | null;
	/** The Hero's square artwork, present exactly when `hero` is. */
	art: GuiLibraryArt | null;
	/** The id of the Emby track playing at the playhead (the Library's
	 * playing row): the Queue panel's own authority, carried so a tracklist
	 * row and a list track row paint what the Queue's active slot paints.
	 * `null` while idle or while the active slot holds no Emby item. */
	playingTrackId: string | null;
	/** The right-click item menu the window content last opened (the
	 * row-menu command): the item it named with the TUI menu builder's full
	 * entries for it. `null` while no right-click has named an item. */
	rowMenu: GuiMusicRowMenu | null;
}

/** The Library's right-click item menu (the row-menu command): the item the
 * entries are about, and the TUI menu builder's entries in the builder's
 * order. The core keeps the action, so a chosen entry sends only its index. */
export interface GuiMusicRowMenu {
	/** The item's stable Service id the command named. */
	itemId: string;
	/** The TUI menu builder's entries for the item. */
	entries: GuiMusicMenuEntryFrame[];
}

export interface GuiMusicPill {
	/** The group's stable Service id: the set-collections command keys it. */
	groupId: string;
	name: string;
	/** Whether the core's stored selection holds the group. */
	selected: boolean;
	/** Whether the pill is a collection pill (multi-select) rather than a
	 * single-select one. Every Music pill is a collection pill. */
	collection: boolean;
}

export interface GuiMusicArtist {
	artistKey: GuiArtistKey;
	name: string;
	/** The letter heading above the row, and the rail's letter, or `#`. */
	sortLetter: string;
	albums: GuiMusicAlbum[];
}

/** An artist's stable identity: the Service-resolved one, or the fallback name
 * when the Service resolves none. */
export type GuiArtistKey = { kind: 'service'; id: string } | { kind: 'fallback'; id: string };

export interface GuiMusicAlbum {
	/** The album's stable Service id: the commands that name it, and the key
	 * its artwork and its track fetch use. */
	albumId: string;
	name: string;
	/** The release year; empty when the Service holds none. */
	year: string;
	/** The cached tracklist; `null` until the album node's open loads it. */
	tracks: GuiMusicTrack[] | null;
	/** The album's track count and total time, present exactly when `tracks`
	 * is. */
	total: GuiMusicTrackTotal | null;
}

export interface GuiMusicTrack {
	/** The track's stable Service id. */
	trackId: string;
	name: string;
	/** The number the row paints. */
	number: number;
	/** The runtime, `M:SS` or `H:MM:SS`; empty when unknown. */
	duration: string;
}

export interface GuiMusicTrackTotal {
	count: number;
	duration: string;
}

/** The Hero's facts for the picked album (design D5). */
export interface GuiMusicHero {
	artist: string;
	/** The selected collection names in pill order. */
	collections: string[];
	/** The artist's album count in the union. */
	albumCount: number;
	/** The artist's track count and total time, present once every one of the
	 * artist's union albums holds a cached tracklist. */
	artistTracks: GuiMusicTrackTotal | null;
	album: GuiMusicAlbum;
	/** The picked item's item-menu entries in the shared builder's order
	 * (design D8): More actions paints them, and a chosen entry sends its
	 * index back. Empty while the item offers no menu — the Library panel does
	 * not hold Panel focus, or the item is gone — so the window content offers
	 * More actions only when the frame carries entries. */
	menu: GuiMusicMenuEntryFrame[];
}

/** One entry of the picked item's item menu (design D8), as the window paints
 * it: the label, the key hint the row-menu face shows when the entry has one,
 * and the entry's index in the shared builder's order — the index a chosen
 * entry sends back, so a front end that paints only some entries still
 * executes the one it painted. The core keeps the action. */
export interface GuiMusicMenuEntryFrame {
	label: string;
	keyHint: string | null;
	/** The entry's index in the shared builder's order (design D8). */
	index: number;
}

/** The Library Hero's square artwork (design D5). The image and placeholder
 * payloads are `data:` URLs the core encoded from the mbv image cache. */
export type GuiLibraryArt =
	| { kind: 'image'; detail: string }
	| { kind: 'placeholder'; detail: string }
	| { kind: 'loading' };

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

/** The Home destination's content (design D12, the USER-APPROVED GUI Home
 * design), present while Home is the active destination: the flat list's
 * rows, the core's selected row, and its Hero. No pill, no letter rail,
 * and no Workspace ride here; the top bar marks the Home icon current
 * exactly while this field is present. */
export interface GuiHomeFrame {
	/** The flat list's rows in paint order (design D12): the two sections'
	 * heading rows and item rows, with the spacer between two non-empty
	 * sections. */
	rows: GuiHomeRow[];
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
	| { row: 'item'; id: GuiHomeRowId; title: string; subtitle: string | null; date: string | null; kind: GuiHomeRowKind; state: GuiHomeRowState; art: GuiLibraryArt | null };

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
	metaRows: string[];
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
	art: GuiLibraryArt | null;
}

/** One view mode of the Shows destination's pill row (design D12, row
 * 7.1). The TUI's letter-range buckets are not pills here — the All series
 * list's letter rail replaces them. */
export type GuiShowsMode = 'latest' | 'upcoming' | 'all';

/** The Shows destination's content (design D12, row 7.1), present while a
 * Shows library is the active destination: the view-mode pills, the active
 * mode's rows, the core's selected row, and its Hero. */
export interface GuiShowsFrame {
	/** The Shows library index the Shows commands name (design D2, D12). */
	libIdx: number;
	/** The three view-mode pills in the design's order (design D12). */
	pills: GuiShowsPill[];
	/** The active mode's name, as the selected collection's name the
	 * Library Status bar's location reads (design D10). */
	collectionNames: string[];
	/** Whether the active mode's list has not landed yet. */
	loading: boolean;
	/** Whether the selected series' detail or the selected season's episodes
	 * fetch has not landed yet (the All series mode only). */
	workspaceLoading: boolean;
	/** The active mode's rows in the list's order. */
	rows: GuiShowsRow[];
	/** The core's selected row (design D2, D12), as the frame painted it:
	 * a select command sends it back. The first row is the Hero default. */
	selected: string | null;
	/** The selected row's Hero facts; `null` in the Upcoming mode and while
	 * no row is selected. */
	hero: GuiShowsHero | null;
}

/** One view-mode pill of the Shows destination (design D12):
 * single-select, the selected one the core's active mode. */
export interface GuiShowsPill {
	/** The mode the pill selects: the set-mode command carries it back. */
	mode: GuiShowsMode;
	name: string;
	/** Whether the core's active mode is this one. */
	selected: boolean;
	/** Whether the pill is a collection pill (multi-select) rather than a
	 * single-select one. Every Shows pill is single-select. */
	collection: boolean;
}

/** One row of the Shows list (design D12): a series row in the All series
 * mode, or a flat episode or premiere row in the Latest and Upcoming
 * modes. The identity is the stable Service id a select or play command
 * sends, never a row position. */
export interface GuiShowsRow {
	/** The row's stable identity. */
	id: string;
	/** The row's primary label: the series' name in the All series mode,
	 * the show's name on a flat episode row, the series' name on a premiere
	 * row. */
	title: string;
	/** The row's secondary label: the episode's own title on a flat row. */
	subtitle: string | null;
	/** The gutter date on a flat row; `null` on a series row. */
	date: string | null;
	/** The row's rail letter (design D7), or `#`; `null` on the date-sorted
	 * flat rows, whose list shows no rail. */
	sortLetter: string | null;
}

/** The Hero's facts for the selected row (design D12), with the series
 * Hero's Workspace. */
export interface GuiShowsHero {
	title: string;
	metaRows: string[];
	durationRow: number | null;
	progressRow: number | null;
	overview: string | null;
	/** The artwork box the Hero reserves (the artwork policy's declared
	 * arm: landscape for a series and a thumb-declared episode, portrait
	 * for a poster-declared one). */
	artShape: 'landscape' | 'square' | 'portrait';
	art: GuiLibraryArt | null;
	/** The series Hero's Workspace; `null` on the Latest mode's episode
	 * Hero and while the selected series' detail has not landed. */
	workspace: GuiShowsWorkspace | null;
}

/** A series Hero's Workspace (design D12): the season pills and the
 * selected season's episodes. The season pills start on the first season. */
export interface GuiShowsWorkspace {
	/** The season pills in the detail's order. */
	seasons: GuiShowsSeasonPill[];
	/** The selected season's id; `null` while the detail holds no season. */
	selectedSeason: string | null;
	/** The selected season's episodes in their fetch order; empty while
	 * that fetch has not landed, which the frame's `workspaceLoading`
	 * names. */
	episodes: GuiShowsEpisodeRow[];
}

/** One season pill of a series' Hero (design D12). */
export interface GuiShowsSeasonPill {
	/** The season's stable Service id: the select-season command carries
	 * it. */
	seasonId: string;
	name: string;
	/** Whether this is the Workspace's selected season. */
	selected: boolean;
}

/** One episode row of the Hero's Workspace (design D12). */
export interface GuiShowsEpisodeRow {
	/** The episode's stable Service id. */
	episodeId: string;
	/** The number the row paints. */
	number: number;
	name: string;
	/** The runtime, `M:SS` or `H:MM:SS`; empty when unknown. */
	duration: string;
	/** The played/progress state, the shared media-list row state the Home
	 * rows carry (`GuiHomeRowState`). */
	state: GuiHomeRowState;
}
