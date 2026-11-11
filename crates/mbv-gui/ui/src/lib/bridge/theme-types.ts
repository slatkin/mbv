// The theme wire types of the GUI bridge contract (openspec change
// add-gui-settings-screen, design D7). Each type mirrors one serde DTO in
// crates/mbv-gui/src/bridge/frame/theme.rs field for field, so a rename on the
// Rust side is a rename here. They live in their own file and are re-exported
// by `types.ts`, so every call site keeps importing from the one bridge
// module, as the `library-types-*.ts` split does.

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
