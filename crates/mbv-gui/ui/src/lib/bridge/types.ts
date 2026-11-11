// The wire types of the GUI bridge contract (openspec change
// add-tauri-gui-client, design D4). Each type mirrors one serde DTO in
// crates/mbv-gui/src/bridge/ field for field: the names are the serde wire
// names, so a rename on the Rust side is a rename here. The Library family
// lives in `library-types.ts`; this module re-exports it so every consumer
// keeps importing from the one bridge module.

import type { GuiLibraryFrame } from './library-types';

export type {
	GuiArtistKey,
	GuiAudiobooksChapterRow,
	GuiAudiobooksFrame,
	GuiAudiobooksHero,
	GuiAudiobooksRow,
	GuiAudiobooksWorkspace,
	GuiHomeContentId,
	GuiHomeFrame,
	GuiHomeHero,
	GuiHomeRow,
	GuiHomeRowId,
	GuiHomeRowKind,
	GuiHomeRowState,
	GuiLibraryArt,
	GuiLibraryCommand,
	GuiLibraryContent,
	GuiLibraryFrame,
	GuiLibraryTab,
	GuiMusicAlbum,
	GuiMusicArtist,
	GuiMusicFrame,
	GuiMusicHero,
	GuiMusicMenuEntryFrame,
	GuiMusicPill,
	GuiLibraryRowMenu,
	GuiMusicTrack,
	GuiMusicTrackTotal,
	GuiPodcastPill,
	GuiPodcastsFrame,
	GuiPodcastsHero,
	GuiPodcastsPill,
	GuiPodcastsRow,
	GuiEmbyFeedGroup,
	GuiEmbyFrame,
	GuiEmbyHero,
	GuiEmbyHeroCredit,
	GuiEmbyPill,
	GuiEmbyRow,
	GuiFeedPill,
	GuiFeedsFrame,
	GuiFeedsHero,
	GuiFeedsLibraryCommand,
	GuiFeedsPill,
	GuiFeedsRow,
	GuiSearchFrame,
	GuiSearchLibraryCommand,
	GuiSearchRow,
	GuiEmbyRowKind,
	GuiShowsEpisodeRow,
	GuiShowsFrame,
	GuiShowsHero,
	GuiShowsMode,
	GuiShowsPill,
	GuiShowsRow,
	GuiShowsSeasonNode,
	GuiShowsSeasonPill,
	GuiShowsSeries,
	GuiShowsWorkspace,
} from './library-types';

/** One `mbv://frame` payload: the attached slice's frame, or the state of a window whose Owner attach has not finished (design D9). */
export type GuiWindowFrame = { slice: GuiFrame } | { attach: GuiAttachFrame };

/** The attached slice's frame: every section the window paints. */
export interface GuiFrame {
	focus: GuiFocus;
	queue: GuiQueue;
	/** The queue header's facts: the remaining count, the finish clock, the
	 * right column's mode, and the filter text with its caret fact. */
	summary: GuiQueueSummary;
	/** The panel toggles the front end picks the layout with (design D10). */
	panel: GuiPanelToggles;
	/** The open menu or popover; `null` while none is open (design D4). */
	overlay: GuiOverlayFrame | null;
	playback: GuiQueuePlayback;
	footer: GuiQueueFooter;
	status: GuiStatusBar;
	/** The Library panel's section (add-gui-library-panel design D2): the
	 * active destination's content under its revision. `null` while no Library
	 * panel shows. */
	library: GuiLibraryFrame | null;
	/** `null` unless the theme changed for this window (design D7). */
	theme: GuiTheme | null;
}

/** The Owner attach state the window shows before a slice exists (design D9). */
export interface GuiAttachFrame {
	state: GuiAttachState;
	theme: GuiTheme | null;
}

/** One variant per Owner attach outcome, with the text the TUI Client gets. */
export type GuiAttachState =
	| { kind: 'refusal'; detail: string }
	| { kind: 'exclusiveRefusal'; detail: string }
	| { kind: 'waiting' }
	| { kind: 'mismatch'; detail: string }
	| { kind: 'failed'; detail: string };

/** The focused surface and the Panel that holds focus. */
export interface GuiFocus {
	surface: GuiSurface | null;
	panel: GuiPanel;
}

export type GuiSurface =
	| 'uiRoot'
	| 'tabPanel'
	| 'statusBarPanel'
	| 'libraryPlaybackPanel'
	| 'queuePlaybackPanel'
	| 'queue'
	| 'queueBoundary'
	| 'library'
	| { overlay: GuiOverlay }
	| { modal: GuiModal }
	| { popup: GuiPopup };

export type GuiOverlay = 'search' | 'settings' | 'sessions' | 'playlists' | 'help' | 'contextMenu';
export type GuiModal = 'confirm' | 'daemonLost' | 'savePlaylist' | 'twitchUrl';
export type GuiPopup = 'multiselect' | 'libraryRoutes' | 'feedManage';
export type GuiPanel = 'queue' | 'library';

/** The queue summary the header paints: the remaining count, the queue
 * finish clock, the right column's mode, and the filter text. */
export interface GuiQueueSummary {
	/** The slots after the active index; the whole queue while no slot
	 * carries the playhead. */
	remaining: number;
	/** The title of the first slot after the playhead (review of row 6.6,
	 * finding P2): the compact strip's next-item line. It comes from the
	 * queue, not from the painted rows, because the strip paints no Queue
	 * list. `null` while no slot follows the playhead. */
	nextTitle: string | null;
	/** The wall-clock time the shown queue finishes; `null` while no slot
	 * carries the playhead or a pending runtime in the chain is unknown. */
	finishClock: string | null;
	/** Which value the rows' right column paints. */
	rightColumn: GuiRightColumn;
	/** The queue filter's text, which narrows the projected rows locally;
	 * empty text filters nothing. */
	filter: string;
	/** Whether the filter's text entry holds the keyboard: the caret fact
	 * the filter control paints. */
	filterActive: boolean;
}

/** The canvas panel's collapse and queue-fold facts (design D10): the `z`
 * and `q` toggles the Client core holds. */
export interface GuiPanelToggles {
	collapsed: boolean;
	queueFolded: boolean;
}

/** The open menu or popover (design D4). */
export type GuiOverlayFrame =
	| {
			kind: 'rowMenu';
			/** The row the menu opened on (`QueueSlotId::raw`): the window
			 * anchors the menu there instead of guessing from hover/cursor. */
			anchorSlot: number;
			cursor: number;
			entries: GuiRowMenuEntry[];
	  }
	| {
			kind: 'trackPopover';
			chip: GuiTrackChipKind;
			cursor: number;
			choices: GuiTrackPopoverChoice[];
	  }
	| {
			/** The playback-target chooser (row 6.4): the cursor and the
			 * cast/session choices — the same list `playback.targetChoices`
			 * carries. */
			kind: 'targetChooser';
			cursor: number;
			choices: GuiTargetChoice[];
	  }
	| {
			/** The shared confirmation modal the App raised: the message and
			 * the button row, each with its key hint and tone. The action
			 * stays in the core's held state, so a press or key answers what
			 * the window painted. */
			kind: 'confirm';
			message: string;
			buttons: GuiConfirmButton[];
	  }
	| {
			/** The daemon-lost recovery dialog the App raised
			 * (`daemon-disconnect-handling`: an unannounced connection loss
			 * raises a recovery dialog): the diagnostics the TUI's modal
			 * paints and the recovery choices, each with its key hint and
			 * tone. The dialog stays in the core's held state, so a press or
			 * key answers what the window painted. */
			kind: 'daemonLost';
			/** The title of whatever was playing right before the Owner
			 * vanished; `null` when nothing was. */
			lastPlayingTitle: string | null;
			/** Where the local daemon's own log lives. */
			daemonLogPath: string;
			/** The last restart attempt's failure: the dialog's
			 * `Restart failed:` line. */
			restartError: string | null;
			buttons: GuiConfirmButton[];
	  };

/** One button of the confirm overlay: the label, the key hint inside the
 * pill (`Enter`, `Esc`), and how the canvas paints it. */
export interface GuiConfirmButton {
	label: string;
	keys: string;
	tone: 'affirmative' | 'cancel';
}

/** One row-menu entry: the label, the key hint that selects it directly
 * when it has one, and whether the entry is destructive (the canvas paints
 * it in the destructive tone under a divider). */
export interface GuiRowMenuEntry {
	label: string;
	keyHint: string | null;
	destructive: boolean;
}

/** Which track chip a popover belongs to. */
export type GuiTrackChipKind = 'audio' | 'subtitles';

/** One choice of an open track popover: the mpv track id a choice sends,
 * its label, whether a subtitle track is forced, and whether it plays now. */
export interface GuiTrackPopoverChoice {
	id: number;
	label: string;
	forced: boolean;
	chosen: boolean;
}

/** Which value the queue rows' right column paints. */
export type GuiRightColumn = 'length' | 'endsAt';

/** The Queue section: the viewport rows, cursor, marks, and active slot. */
export interface GuiQueue {
	rows: GuiQueueRow[];
	/** The slot under the cursor (`QueueSlotId::raw`), if any. */
	cursor: number | null;
	/** The slots marked for a bulk action (`QueueSlotId::raw`). */
	marks: number[];
	/** The slot that carries the now-playing marker, if any. */
	active: number | null;
	/** The row the pointer rests on; the front end paints that row's grip and
	 * actions button. `null` while the pointer rests on no row. */
	hover: number | null;
	/** The row drag in progress (the ghost and insertion line); `null` while
	 * none runs. */
	drag: GuiQueueDrag | null;
}

/** The row drag one frame paints: the dragged row's content as the ghost, the
 * final index the drop would leave it at, and the insertion line's anchor. */
export interface GuiQueueDrag {
	/** The dragged slot (`QueueSlotId::raw`). */
	slot: number;
	/** The dragged row's content, painted as the drag ghost. */
	ghost: GuiQueueRow;
	/** The slot's final queue index if the drop applied now. */
	dropIndex: number;
	/** The insertion line's anchor. */
	insertion: GuiQueueInsertion;
}

/** The insertion line: the stable slot it anchors on and the side it paints. */
export type GuiQueueInsertion =
	| { at: 'before'; slot: number }
	| { at: 'after'; slot: number };

export type GuiQueueRow =
	| {
			row: 'item';
			/** The stable slot (`QueueSlotId::raw`). */
			slot: number;
			/** The row's 1-based position in the queue: the canvas's `01`-style
			 * row number, read from the canonical queue order rather than the
			 * painted viewport, so a scrolled or filtered viewport still numbers
			 * rows by their queue positions. */
			ordinal: number;
			primary: string;
			secondary: string | null;
			trailing: GuiTrailing | null;
			/** The right column's "Length" value. */
			duration: string | null;
			/** The right column's "Ends at" value; `null` for a slot before the
			 * playhead and for a slot whose finish chain an unknown runtime
			 * broke. */
			endsAt: string | null;
			kind: GuiMediaKind;
			state: GuiSemanticState;
			/** Whether the row takes the alternating `QueueColumn` fill — the
			 * TUI's zebra walk, carried from the core so the stripe stays stable
			 * while the viewport moves. */
			striped: boolean;
	  }
	| { row: 'heading'; text: string }
	| { row: 'spacer' };

export type GuiTrailing = { gutter: string };
export type GuiMediaKind = 'collection' | 'media';

export type GuiSemanticState =
	| { state: 'ordinary' }
	| { state: 'played' }
	| { state: 'active'; progress: number | null }
	| { state: 'nowPlaying'; progress: number | null };

/** The Queue playback transport band's facts. */
export interface GuiQueuePlayback {
	status: GuiNowPlayingStatus;
	host: string;
	hostIsRemote: boolean;
	shown: boolean;
	playback: GuiPlaybackState;
	/** The seek row's left clock (the playhead's elapsed time), formatted by
	 * the TUI's shared duration formatter. */
	elapsedClock: string;
	/** The seek row's right clock (the time left in the active item), in the
	 * same format. The window paints the minus sign before it. */
	remainingClock: string;
	showControls: boolean;
	nowPlayingTitle: string | null;
	/** The now-playing parent line: the series of an episode, the artist of
	 * a track, the subscription of a feed entry. */
	parentLine: string | null;
	/** The season-and-episode badge the artwork paints (`"S2 · E5"`); `null`
	 * for every kind but an Emby episode with an ordinal known. */
	episodeBadge: string | null;
	/** The still slot's caption while the artwork has no image (design
	 * D4a's still slot): the image source the card-image fetch names for the
	 * active item's kind. `null` while nothing to caption — idle, a
	 * slotless output, or an item kind with no image source. */
	stillCaption: string | null;
	/** The active item's wall-clock end time; the scrubber's finish clock.
	 * `null` while no item plays or the runtime is unknown. */
	finishClock: string | null;
	/** The active item's chapter boundaries with their names (design D4a):
	 * the scrubber's boundary ticks and the hover preview's chapter name.
	 * Empty while the item's fetch has not landed or carries none. */
	chapters: GuiChapter[];
	/** The intro mark's tick bounds (the MediaSegments `Intro`); `null`
	 * while unknown. */
	introMark: GuiSegmentMark | null;
	/** The credits mark's tick bounds (the MediaSegments `Outro`); `null`
	 * while unknown. */
	creditsMark: GuiSegmentMark | null;
	/** The trickplay source description (design D4a): one tile grid per
	 * available width. `null` while unknown; the tile bytes are task 6.7's
	 * fetch. */
	trickplay: GuiTrickplayGrid[] | null;
	availability: GuiTransportAvailability;
	/** The status-indicator cluster the queue band paints on its controls
	 * row: `null` while the local player is inactive — the cluster's data
	 * source is the local player's stream, so a band whose playback rides a
	 * remote output paints none, as the TUI's does. */
	statusIndicators: GuiStatusIndicator[] | null;
	/** The visual slot's artwork; `null` unless the art changed for this
	 * window (design D10) — the component keeps the last carried value. */
	artwork: GuiCardArtwork | null;
	/** The track chips under the transport: the video stream's resolved
	 * label, and the audio and subtitle tracks each with its choices and the
	 * chosen one. `null` while the local player is inactive, as the
	 * status-indicator cluster is. */
	trackChips: GuiTrackChips | null;
	/** The playback-target chooser's choices: the Emby sessions then the
	 * cast receivers the Owner offers, each marked with whether it is the
	 * Owner's attached output. */
	targetChoices: GuiTargetChoice[];
	/** The scrubber's hover preview (row 6.4): the pointer's fraction along
	 * the bar and the time and chapter it names. `null` while the pointer
	 * is away from the bar. */
	hover: GuiScrubberHover | null;
}

/** The scrubber's hover preview (design D4): the position a move over the
 * painted bar carried, resolved to the time and chapter it names, and the
 * trickplay tile the preview paints (task 6.7). */
export interface GuiScrubberHover {
	/** The pointer's fraction along the painted bar. */
	fraction: number;
	/** The time at the pointer's position, formatted by the TUI's shared
	 * duration formatter. The window paints the text verbatim. */
	timeLabel: string;
	/** The chapter covering the pointer's position, when the active item's
	 * chapters cover it. */
	chapter: string | null;
	/** The covering chapter's 1-based ordinal in the active item's chapter
	 * list: the canvas preview label's `Ch. 2` form. `null` while no chapter
	 * covers the pointer. */
	chapterNumber: number | null;
	/** The trickplay tile at the pointer's position; `null` while the item
	 * offers none, its sheet is not cached yet, or the server does not hold
	 * it. */
	tile: GuiScrubberTile | null;
}

/** The hovered trickplay tile (task 6.7): the thumbnail's place in its sheet,
 * the thumbnail's pixel size, and the sheet's bytes as a `data:image/jpeg`
 * URL. The window fetches nothing — the core's cache supplies the bytes, so
 * the CSP keeps its `data:` image source (design D11). */
export interface GuiScrubberTile {
	/** The sheet's opaque identity: the window repaints the sheet it latched
	 * for this key while `sheet` is `null`. */
	sheetKey: string;
	/** The thumbnail's pixel width within the sheet. */
	width: number;
	/** The thumbnail's pixel height within the sheet. */
	height: number;
	/** The sheet's thumbnail columns. */
	columns: number;
	/** The thumbnail's column in the sheet. */
	column: number;
	/** The thumbnail's row in the sheet. */
	row: number;
	/** The sheet image as a `data:image/jpeg` URL; `null` when the frame does
	 * not re-carry it — the hovered sheet is the one already latched. */
	sheet: string | null;
}

/** The trickplay sheet the window latches for the hover preview (task 6.7):
 * the image last carried and the sheet key it belongs to. A frame re-carries
 * the image only when the hovered sheet changes, so the preview repaints the
 * latched one while the tile names the same key. */
export interface GuiTrickplaySheet {
	/** The sheet's opaque identity, as the tile's `sheetKey` names it. */
	key: string;
	/** The sheet image as a `data:image/jpeg` URL. */
	image: string;
}

/** One chapter boundary of the active item: the scrubber's boundary tick
 * and the hover preview's chapter name. */
export interface GuiChapter {
	name: string;
	startTicks: number;
}

/** One intro or credits mark of the active item: the tick bounds the
 * scrubber paints. */
export interface GuiSegmentMark {
	startTicks: number;
	endTicks: number;
}

/** One trickplay tile grid of the active item: the source description the
 * scrubber preview's tile fetch (task 6.7) reads. */
export interface GuiTrickplayGrid {
	/** The thumbnail pixel width this grid answers. */
	width: number;
	/** The thumbnail pixel height. */
	height: number;
	/** Thumbnails per row of one tile sheet. */
	tileWidth: number;
	/** Thumbnail rows of one tile sheet. */
	tileHeight: number;
	/** The total thumbnails the grid holds. */
	thumbnailCount: number;
	/** Milliseconds between consecutive thumbnails. */
	intervalMs: number;
	/** The media source the tile URLs qualify with. */
	mediaSourceId: string;
}

/** The track chips' facts. */
export interface GuiTrackChips {
	/** The video stream's resolved label; `null` for an audio-only item. */
	video: string | null;
	audio: GuiTrackChip;
	subtitles: GuiTrackChip;
}

/** One chip's facts: its choices and the chosen one. */
export interface GuiTrackChip {
	choices: GuiTrackChoice[];
	/** The chosen choice's mpv id; `null` is off or unresolved. */
	chosen: number | null;
}

/** One chip choice: the mpv track id a switch sends, the resolved label,
 * and whether the subtitle track is forced. */
export interface GuiTrackChoice {
	id: number;
	label: string;
	forced: boolean;
}

/** One playback-target choice of the chooser. */
export interface GuiTargetChoice {
	/** The stable identity a selection sends. */
	key: GuiTargetKey;
	label: string;
	/** Whether the target is the Owner's attached playback output. */
	isActive: boolean;
}

/** A playback target's stable identity: this machine's local output, the Emby
 * session id, or the cast receiver id. */
export type GuiTargetKey =
	| { kind: 'local' }
	| { kind: 'emby'; id: string }
	| { kind: 'cast'; id: string };

/** One status indicator of the queue band's cluster: the uppercased label
 * and the foreground role its text paints in. */
export interface GuiStatusIndicator {
	label: string;
	role: GuiRole;
}

/** The visual slot's artwork one frame carries (design D10). The image and
 * placeholder payloads are `data:` URLs the core encoded from its cache. */
export type GuiCardArtwork =
	| { kind: 'image'; detail: string }
	| { kind: 'placeholder'; detail: string }
	| { kind: 'loading' }
	| { kind: 'hidden' };

export type GuiNowPlayingStatus = 'playing' | 'paused' | 'idle';

export interface GuiPlaybackState {
	active: boolean;
	activeIdx: number | null;
	positionTicks: number;
	runtimeTicks: number;
	paused: boolean;
}

export interface GuiTransportAvailability {
	stop: boolean;
	next: boolean;
	previous: boolean;
}

/** The Queue footer's status pills. */
export interface GuiQueueFooter {
	owner: GuiFooterPill;
	/** Whether the shown queue's Player owner is live. The session marker
	 * paints its dot only while this holds (design D4a). */
	ownerLive: boolean;
	playlist: GuiFooterPill;
	autosave: GuiFooterPill | null;
	dirty: GuiFooterPill | null;
}

export interface GuiFooterPill {
	text: string;
	role: GuiRole;
	bold: boolean;
}

/** The Status bar's facts: the armed indicator and the live toast. */
export interface GuiStatusBar {
	prefixArmed: boolean;
	toast: GuiToast | null;
}

export interface GuiToast {
	text: string;
	severity: GuiToastSeverity;
}

export type GuiToastSeverity = 'neutral' | 'success' | 'warning' | 'error';

/**
 * The theme section: one colour for every role plus the chrome fills
 * (design D7). The core sends the full role map; `Partial` keeps consumers
 * honest about the roles they read.
 */
export interface GuiTheme {
	roles: Partial<Record<GuiRole, GuiRgb>>;
	chrome: GuiChrome;
}

/** The chrome fills, mirroring `GuiChrome` in bridge/frame/theme.rs: the fills the shadcn-svelte background, surface, border, and input variables point at. */
export interface GuiChrome {
	background: GuiRgb;
	card: GuiRgb;
	popover: GuiRgb;
	muted: GuiRgb;
	secondary: GuiRgb;
	sidebar: GuiRgb;
	border: GuiRgb;
	input: GuiRgb;
	/** The queue column's backdrop: the fill the TUI paints as the queue
	 * column's gutter around its recessed box, playback band, and footer
	 * band. */
	queueColumn: GuiRgb;
	/** The queue column's focused backdrop: the same surface's focused fill;
	 * the front end picks between the pair on the frame's Panel-focus fact. */
	queueColumnFocused: GuiRgb;
	/** The queue panel's recessed box, the surface the TUI paints the queue
	 * list's box with. */
	queuePanel: GuiRgb;
	/** The queue panel's focused box: the same surface's focused fill. */
	queuePanelFocused: GuiRgb;
	/** The queue column's playback band: the fixed chrome-band fill the TUI
	 * paints the queue header row and transport band with. */
	queuePlaybackBand: GuiRgb;
	/** The queue footer's band, behind its status-bar pills. */
	queueFooterBand: GuiRgb;
}

export interface GuiRgb {
	r: number;
	g: number;
	b: number;
}

/** The closed role set, mirroring `GuiRole` in bridge/frame/theme.rs. */
export type GuiRole =
	| 'textPrimary'
	| 'textSecondary'
	| 'textMuted'
	| 'textStrong'
	| 'textEmphasis'
	| 'tabSelectedUnderline'
	| 'textFocusAccent'
	| 'textHeroTitle'
	| 'workspaceHeaderFg'
	| 'textOnAccent'
	| 'textAccentMuted'
	| 'playlistLoadedFg'
	| 'textDetailMeta'
	| 'textMetadata'
	| 'selectedRowFg'
	| 'selectedRowProgressFg'
	| 'statusError'
	| 'statusAvailable'
	| 'duration'
	| 'splitRowContextFg'
	| 'splitRowTitleFg'
	| 'indicatorResolutionFg'
	| 'indicatorAudioFg'
	| 'playbackValueFg'
	| 'playbackMetaFg'
	| 'playbackTitleFg'
	| 'playbackContextFg'
	| 'playbackHostRemoteFg'
	| 'playbackTargetLocalFg'
	| 'playbackTargetRemoteFg'
	| 'idleFeedTitleFg'
	| 'progressTrack'
	| 'progressPercent'
	| 'groupHeadingFg'
	| 'emptyQueueFg'
	| 'scrollbar'
	| 'sidebarScrollbar'
	| 'heroOverviewSeparator'
	| 'heroCreditsName'
	| 'accent'
	| 'accentActive'
	| 'accentAudiobookshelf'
	| 'pillSelectedFg'
	| 'pillOverflowFg';

/** The DOM `KeyboardEvent` fields the `key` command carries (design D6). */
export interface DomKey {
	key: string;
	code: string;
	metaKey: boolean;
	repeat: boolean;
	shiftKey: boolean;
	ctrlKey: boolean;
	altKey: boolean;
}

/** The DOM pointer fields the `pointer` command carries (design D4). */
export interface DomPointer {
	kind: DomPointerKind;
	/** The DOM `button`: 0 primary, 1 auxiliary, 2 secondary, -1 none. */
	button: number;
	/** The DOM `buttons` bitmask of held buttons. */
	buttons: number;
	/** The pointer position in logical pixels, relative to the window. */
	x: number;
	y: number;
	metaKey: boolean;
	shiftKey: boolean;
	ctrlKey: boolean;
	altKey: boolean;
	/** Wheel deltas in logical pixels; zero for the other kinds. */
	deltaX: number;
	deltaY: number;
}

export type DomPointerKind = 'press' | 'release' | 'move' | 'wheel' | 'cancel';

/** The layout report the front end sends after each paint that moved a region (design D4). */
export interface LayoutReport {
	lists: ListRows[];
	regions: LayoutRegion[];
	/** The panel container's painted size in logical pixels (design D10):
	 * the measured size the front end picks the panel state from. */
	containerSize: LayoutSize;
}

/** A size in logical pixels, as a ResizeObserver measures the panel container. */
export interface LayoutSize {
	width: number;
	height: number;
}

export interface ListRows {
	list: 'queue';
	wholeRows: number;
}

export interface LayoutRegion {
	rect: LayoutRect;
	target: RegionTarget;
}

export interface LayoutRect {
	x: number;
	y: number;
	width: number;
	height: number;
}

export type RegionTarget =
	| { surface: 'queue'; id: QueueTargetId }
	| { surface: 'queuePlayback'; id: QueuePlaybackTargetId }
	| { surface: 'statusBar'; id: StatusBarTargetId }
	| { surface: 'attach'; id: AttachTargetId }
	/** The confirm overlay's button, by index in the overlay's `buttons`. */
	| { surface: 'confirm'; id: { button: number } }
	/** The daemon-lost dialog's button, by index in the overlay's
	 * `buttons`. */
	| { surface: 'daemonLost'; id: { button: number } };

/** The pre-attach screen's button targets (design D9). */
export type AttachTargetId = 'restart' | 'quit';

export type QueueTargetId =
	| { slot: number }
	| 'playlistPill'
	/** The hovered row's drag grip: a press starts the row drag. */
	| { rowGrip: number }
	/** The hovered row's actions button: a press opens the row menu. */
	| { rowActions: number }
	/** One entry of the open row menu, by index in the overlay's `entries`. */
	| { rowMenuEntry: number }
	/** The header's Length / Ends-at switch: the `t` chord's toggle. */
	| 'rightColumn'
	/** The header's filter control: the `/` chord's text entry. */
	| 'filter';

export type QueuePlaybackTargetId =
	| 'playPause'
	| 'stop'
	| 'previous'
	| 'next'
	| 'seekbar'
	| 'playbackTarget'
	| 'visualSlot'
	| 'seekBack'
	| 'seekForward'
	| 'collapse'
	| 'queueFold'
	| { targetChoice: number }
	| 'audioChip'
	| 'subtitleChip'
	| { trackPopoverChoice: number };

export type StatusBarTargetId = 'volumePill' | 'mutePill' | 'visualClear';
