// The Library wire types of the GUI bridge contract (openspec change
// add-gui-library-panel, design D2). Each type mirrors one serde DTO in
// crates/mbv-gui/src/bridge/ field for field: the names are the serde wire
// names, so a rename on the Rust side is a rename here. The Home, Podcasts,
// Feeds, and Search DTOs live in their `library-types-*.ts` files and are
// re-exported below, so every call site keeps importing from the one bridge
// module.

import type { GuiFeedsFrame, GuiFeedsLibraryCommand } from './library-types-feeds';
import type { GuiHomeFrame, GuiHomeRowId, GuiHomeRowState } from './library-types-home';
import type { GuiPodcastsFrame } from './library-types-podcasts';
import type {
	GuiSearchFrame,
	GuiSearchLibraryCommand,
} from './library-types-search';

/** The Library panel's section (add-gui-library-panel design D2). A frame
 * re-carries `content` only when `revision` changes; a `reportLayout` reload
 * re-carries it under the revision the window already holds, so the window
 * keeps its last carried content while `content` is `null`. */
export interface GuiLibraryFrame {
	revision: number;
	content: GuiLibraryContent | null;
	/** The Search facts while a search runs or its results hold (design D12,
	 * row 12.1); `null` while the section is closed. It rides the frame, not
	 * `content`, so the revision gating of the carried content never holds a
	 * stale results section back. */
	search: GuiSearchFrame | null;
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
	/** The plain Emby-library destination's content while a plain Emby
	 * library is active (design D12, row 8.1); `null` while any other
	 * destination is active. */
	emby: GuiEmbyFrame | null;
	/** The Audiobooks destination's content while an Audiobookshelf book
	 * library is active (design D12, row 9.1); `null` while any other
	 * destination is active. */
	audiobooks: GuiAudiobooksFrame | null;
	/** The Podcasts destination's content while an Audiobookshelf podcast
	 * library is active (design D12, row 10.1); `null` while any other
	 * destination is active. */
	podcasts: GuiPodcastsFrame | null;
	/** The Feeds destination's content while Feeds is the active destination
	 * (design D12, row 11.1); `null` while any other destination is active. A
	 * Client holds one feed tab, so no library index names it. */
	feeds: GuiFeedsFrame | null;
	/** The right-click item menu the window content last opened (the row-menu
	 * command), whichever destination's row named it (design D8, D15): the
	 * item with the TUI menu builder's entries for it. `null` while no
	 * right-click has named an item. It rides the content, not a destination's
	 * section, because every destination's rows open the same menu. */
	rowMenu: GuiLibraryRowMenu | null;
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
	  }
	| {
			/** Select a plain Emby-library row (design D12, row 8.1): the row
			 * the Hero shows. A row the active list no longer holds changes
			 * nothing. */
			command: 'embySelect';
			libIdx: number;
			row: string;
	  }
	| {
			/** Set the feed-view library's selected group (design D12, row
			 * 8.1): the TUI's own selection path. Feed-group mode only; a
			 * library that presents none changes nothing. */
			command: 'embySetFeedGroup';
			libIdx: number;
			group: GuiEmbyFeedGroup;
	  }
	| {
			/** Play a plain Emby-library row (design D12, row 8.1): Enter on
			 * the row, or the double-click a front end sends with this same
			 * command. A folder or a boxset plays through the TUI's
			 * folder-play rule, so no nested list opens. */
			command: 'embyPlay';
			libIdx: number;
			itemId: string;
	  }
	| {
			/** Play a plain Emby-library row next (design D12, row 8.1). */
			command: 'embyPlayNext';
			libIdx: number;
			itemId: string;
	  }
	| {
			/** Add a plain Emby-library row to the queue (design D12, row
			 * 8.1). */
			command: 'embyQueue';
			libIdx: number;
			itemId: string;
	  }
	| {
			/** Select an Audiobooks book (design D12, row 9.1): the book the
			 * Hero shows. A book the published catalog no longer carries
			 * changes nothing. */
			command: 'absSelect';
			libIdx: number;
			row: string;
	  }
	| {
			/** Play an Audiobooks book (design D12, row 9.1): Enter on the
			 * book row, or the double-click a front end sends with this same
			 * command. The queue is replaced with the book at its saved
			 * position. */
			command: 'absPlay';
			libIdx: number;
			itemId: string;
	  }
	| {
			/** Play or seek from an Audiobooks chapter (design D12, row 9.1):
			 * Enter on a Workspace chapter row, or the double-click a front
			 * end sends with this same command. */
			command: 'absPlayChapter';
			libIdx: number;
			bookId: string;
			chapter: number;
	  }
	| {
			/** Add an Audiobooks book to the queue (design D12, row 9.1). */
			command: 'absQueue';
			libIdx: number;
			itemId: string;
	  }
	| {
			/** Set the Podcasts destination's pill (design D12, row 10.1):
			 * Latest, or one show by its stable `library_item_id`. */
			command: 'podcastsSetPill';
			libIdx: number;
			pill: GuiPodcastPill;
	  }
	| {
			/** Select the Podcasts episode the Hero shows (design D12, row
			 * 10.1): the row id the frame carried. */
			command: 'podcastsSelect';
			libIdx: number;
			row: string;
	  }
	| {
			/** Play a Podcasts episode (design D12, row 10.1): Enter on the
			 * row, or the double-click a front end sends with this same
			 * command. */
			command: 'podcastsPlay';
			libIdx: number;
			itemId: string;
	  }
	| {
			/** Add a Podcasts episode to the queue (design D12, row 10.1). */
			command: 'podcastsQueue';
			libIdx: number;
			itemId: string;
	  }
	| GuiSearchLibraryCommand
	| GuiFeedsLibraryCommand;

/** One Podcasts pill (design D12, row 10.1): the library's merged Latest
 * view, or one show by its stable `library_item_id`. The pill row is
 * single-select; the state pills are dropped. */
export type GuiPodcastPill =
	| { kind: 'latest' }
	| { kind: 'state'; id: GuiPodcastState }
	| { kind: 'show'; id: string };

/** One Podcasts state pill (design D12, row 10.1): the play state the view
 * filters on, named as the TUI's `AudiobookshelfEpisodeFilter` names it. */
export type GuiPodcastState = 'all' | 'unplayed' | 'played';

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
}

/** The Library's right-click item menu (the row-menu command): the item the
 * entries are about, and the TUI menu builder's entries in the builder's
 * order. The core keeps the action, so a chosen entry sends only its index. */
export interface GuiLibraryRowMenu {
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

/** One view mode of the Shows destination's pill row (design D12, row
 * 7.1). The TUI's letter-range buckets are not pills here — the All series
 * list's letter rail replaces them. */
export type GuiShowsMode = 'latest' | 'upcoming' | 'all';

/** The Shows destination's content (design D14), present while a Shows
 * library is the active destination: the view-mode pills, the active mode's
 * rows — the All mode's tree or the flat lists — the core's selected row, and
 * its Hero. */
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
	/** The All series mode's tree (design D14): the series nodes with their
	 * seasons and loaded episodes. Empty in the flat modes. */
	series: GuiShowsSeries[];
	/** Whether the Upcoming mode's eager fetch failed after all its retries:
	 * the list paints the failure line and the pill is the retry. */
	upcomingFailed: boolean;
	/** The active mode's rows in the list's order (the flat lists). */
	rows: GuiShowsRow[];
	/** The core's selected row (design D2, D12), as the frame painted it:
	 * a select command sends it back. The first row is the Hero default. */
	selected: string | null;
	/** The selected row's Hero facts; `null` in the Upcoming mode and while
	 * no row is selected. */
	hero: GuiShowsHero | null;
}

/** One series node of the All series tree (design D14), the Music artist
 * node's counterpart: the series row, its rail letter, and its seasons. */
export interface GuiShowsSeries {
	/** The series' stable Service id: the select command names it. */
	id: string;
	/** The series' display name, the row's label. */
	title: string;
	/** The row's rail letter, or `#`. */
	sortLetter: string;
	/** Whether the node opens. A series whose detail has not landed counts as
	 * expandable; its open asks the core to load the detail. */
	expandable: boolean;
	/** The series' seasons in the detail's order; empty while the detail has
	 * not landed. */
	seasons: GuiShowsSeasonNode[];
}

/** One season node under a series (design D14), the Music album node's
 * counterpart: the season row and its episodes, or `null` while that
 * season's episode load has not landed. */
export interface GuiShowsSeasonNode {
	/** The season's stable Service id: the select-season command names it. */
	id: string;
	name: string;
	/** The season's episodes in their fetch order; `null` until the load
	 * lands, which an open season asks for. */
	episodes: GuiShowsEpisodeRow[] | null;
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
	title: string;
	metaRows: string[];
	durationRow: number | null;
	progressRow: number | null;
	overview: string | null;
	/** The artwork box the Hero reserves (the artwork policy's declared
	 * arm). */
	artShape: 'landscape' | 'square' | 'portrait';
	art: GuiLibraryArt | null;
}

/** The Audiobooks destination's content (design D12, row 9.1), present
 * while an Audiobookshelf book library is the active destination: the flat
 * book list with its rail letters, the core's selected book, and its Hero
 * with the chapters Workspace. No pill rides here — the letter rail
 * replaces the TUI's surname buckets. */
export interface GuiAudiobooksFrame {
	/** The Audiobooks library index the Audiobooks commands name (design
	 * D2, D12). */
	libIdx: number;
	/** Whether the book catalog has not landed yet. */
	loading: boolean;
	/** Whether the catalog fetch failed. */
	failed: boolean;
	/** Whether the selected book's detail fetch has not landed yet: the
	 * Workspace's loading state. */
	workspaceLoading: boolean;
	/** The catalog's books in the TUI's sort order: author sort key, then
	 * title. */
	rows: GuiAudiobooksRow[];
	/** The core's selected book (design D2, D12), as the frame painted it:
	 * a select command sends it back. The first book is the Hero default. */
	selected: string | null;
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
	metaRows: string[];
	durationRow: number | null;
	progressRow: number | null;
	/** The cleaned description, when the catalog record carries one: the
	 * overview the TUI's Hero paints. */
	overview: string | null;
	/** The artwork box the Hero reserves: the book artwork policy's
	 * portrait. */
	artShape: 'landscape' | 'square' | 'portrait';
	art: GuiLibraryArt | null;
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
	/** Whether this chapter is playing. A frame re-carries the content when
	 * the mark moves, so the Workspace paints what the Queue's active slot
		 * paints. */
		current: boolean;
}

export type {
	GuiHomeContentId,
	GuiHomeFrame,
	GuiHomeHero,
	GuiHomeRow,
	GuiHomeRowId,
	GuiHomeRowKind,
	GuiHomeRowState,
} from './library-types-home';

export type {
	GuiSearchFrame,
	GuiSearchLibraryCommand,
	GuiSearchRow,
} from './library-types-search';

export type {
	GuiFeedPill,
	GuiFeedsFrame,
	GuiFeedsHero,
	GuiFeedsLibraryCommand,
	GuiFeedsPill,
	GuiFeedsRow,
} from './library-types-feeds';

export type {
	GuiPodcastsFrame,
	GuiPodcastsHero,
	GuiPodcastsPill,
	GuiPodcastsRow,
} from './library-types-podcasts';
