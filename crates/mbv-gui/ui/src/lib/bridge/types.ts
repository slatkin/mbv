// The wire types of the GUI bridge contract (openspec change
// add-tauri-gui-client, design D4). Each type mirrors one serde DTO in
// crates/mbv-gui/src/bridge/ field for field: the names are the serde wire
// names, so a rename on the Rust side is a rename here.

/** One `mbv://frame` payload: the attached slice's frame, or the state of a window whose Owner attach has not finished (design D9). */
export type GuiWindowFrame = { slice: GuiFrame } | { attach: GuiAttachFrame };

/** The attached slice's frame: every section the window paints. */
export interface GuiFrame {
	focus: GuiFocus;
	queue: GuiQueue;
	playback: GuiQueuePlayback;
	footer: GuiQueueFooter;
	status: GuiStatusBar;
	breakpoint: GuiBreakpoint;
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

export type GuiBreakpoint = 'mini' | 'narrow' | 'wide';

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

/** The Queue section: the viewport rows, cursor, marks, and active slot. */
export interface GuiQueue {
	rows: GuiQueueRow[];
	/** The slot under the cursor (`QueueSlotId::raw`), if any. */
	cursor: number | null;
	/** The slots marked for a bulk action (`QueueSlotId::raw`). */
	marks: number[];
	/** The slot that carries the now-playing marker, if any. */
	active: number | null;
}

export type GuiQueueRow =
	| {
			row: 'item';
			/** The stable slot (`QueueSlotId::raw`). */
			slot: number;
			primary: string;
			secondary: string | null;
			trailing: GuiTrailing | null;
			duration: string | null;
			kind: GuiMediaKind;
			state: GuiSemanticState;
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
	showControls: boolean;
	nowPlayingTitle: string | null;
	availability: GuiTransportAvailability;
	/** The status-indicator cluster the queue band paints on its controls
	 * row: `null` while the local player is inactive — the cluster's data
	 * source is the local player's stream, so a band whose playback rides a
	 * remote output paints none, as the TUI's does. */
	statusIndicators: GuiStatusIndicator[] | null;
	/** The visual slot's artwork; `null` unless the art changed for this
	 * window (design D10) — the component keeps the last carried value. */
	artwork: GuiCardArtwork | null;
}

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

export type DomPointerKind = 'press' | 'release' | 'move' | 'wheel';

/** The layout report the front end sends after each paint that moved a region (design D4). */
export interface LayoutReport {
	lists: ListRows[];
	regions: LayoutRegion[];
	/** The window's inner width in logical pixels (design D10). */
	width: number;
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
	| { surface: 'attach'; id: AttachTargetId };

/** The pre-attach screen's button targets (design D9). */
export type AttachTargetId = 'restart' | 'quit';

export type QueueTargetId = { slot: number } | 'playlistPill';

export type QueuePlaybackTargetId =
	| 'playPause'
	| 'stop'
	| 'previous'
	| 'next'
	| 'seekbar'
	| 'playbackTarget'
	| 'visualSlot';

export type StatusBarTargetId = 'volumePill' | 'mutePill' | 'visualClear';
