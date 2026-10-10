//! The surface table: one `(resting, focused)` slot pair per surface
//! (change `theme-slot-model`, design D4).
//!
//! Kept apart from the identity set (`surface.rs`) and the resolver
//! (`surface_resolve.rs`) so the table is readable in one place and the enum
//! stays complete by construction. A fixed row states the same slot twice; a
//! focus-reactive row states its resting and focused slots. There is no
//! nesting level, focus source, soft variant or deviation list: each row
//! fully describes its surface's two fills. Surfaces whose pair is identical
//! share one arm (the workspace denies duplicate match arms); each arm's
//! comment names every surface it covers.

use super::slot::Slot;
use super::surface::Surface;

/// The surface's `(resting, focused)` slots: the one fill table (design D4).
pub(super) const fn slots(surface: Surface) -> (Slot, Slot) {
    match surface {
        // --- column/pane: the surface a content body sits on ---
        // The queue column's gutter and the queue boundary strip resolve the
        // queue column's focus; the TV Workspace list's stripe shares the pair
        // (resting the panel's resting fill, focused the stripe the focused
        // Workspace's rows alternate against).
        Surface::QueueColumn | Surface::WorkspaceStripe => (Slot::Bg1, Slot::Bg2),
        // The right column's whole gutter and body (`shell/library_panel.rs`'s
        // `library_body_fill` passes the panel's own focus bit), and the
        // Selector row's spacer band — the one chrome band that follows the
        // panel's focus, because it is a reserved row *inside* the panel.
        // Resting the app backdrop; focused the column fill.
        Surface::LibraryColumn | Surface::PillRowGap => (Slot::Bg0, Slot::Bg2),
        // The Wide hero's sheet: the dark chrome the hero content repaints its
        // own background with, fixed in both focus states. The fixed chrome
        // bands share it: the queue card's visualizer (so the reserved slot
        // never flashes a fill the panel around it does not have), the
        // status/tab bars and panel bands, the Queue-only playback strip
        // (mode-driven, not focus-driven), the pill row and chips, and the
        // confirm modal's buttons.
        Surface::HeroPane
        | Surface::QueueCardVisualizer
        | Surface::StatusBar
        | Surface::StatusBarPill
        | Surface::QueuePanelBand
        | Surface::QueueOnlyPlaybackPanel
        | Surface::SidebarBand
        | Surface::TabBar
        | Surface::PillRow
        | Surface::PillChip
        | Surface::ModalButton => (Slot::BgDim, Slot::BgDim),
        // --- content body: a focusable content region ---
        // The wide-hero rail body and frame, and the same identity across
        // Movies, TV, Music, ABS books/podcasts and Feeds; focused it takes
        // the soft content-body sheet.
        Surface::LibraryPanel => (Slot::Bg1, Slot::Bg3),
        // The queue panel body and a pane's content box (TV's episode listing,
        // Music's track listing, the generic pane inset): soft focused
        // variant, backdrop resting.
        Surface::QueuePanel | Surface::MainContentBox => (Slot::Bg0, Slot::Bg3),
        // The now-playing panel body, fixed at the app backdrop (user decision
        // 2026-10-05); the playback status pill and artwork placeholder
        // insets share the backdrop.
        Surface::PlaybackPanel | Surface::PlaybackStatusPill | Surface::ArtworkPlaceholder => {
            (Slot::Bg0, Slot::Bg0)
        }
        // The queue column's transport row (its controls row) and the seek
        // gauge's own background in `queue_band.rs`. It started on the
        // backdrop slot next to `PlaybackPanel`; the user's 2026-10-10
        // colour decision moved it one tier below the backdrop, so it now
        // owns a slot of its own rather than sharing `Bg0` (invariant 07).
        Surface::TransportRow => (Slot::BgDim1, Slot::BgDim1),
        // The expanded (F1-F4) sidebar body paints the sidebar fill.
        Surface::SidebarBody => (Slot::Bg3, Slot::Bg3),
        // The artwork-loading inset's fill keeps a foreground-ladder slot as a
        // fill; that is today's colour and stays so.
        Surface::ArtworkLoadingPlaceholder => (Slot::FgMuted, Slot::FgMuted),
        // --- fills that replaced roles (design D5) ---
        // The selection bar (media lists, tree browser, playlists, the context
        // menu's selected row).
        Surface::SelectedRow => (Slot::Green, Slot::Green),
        // The secondary zebra stripe shared by the playlists, settings and
        // sessions lists, and the modal frame background.
        Surface::ListStripe | Surface::PopupFrame => (Slot::Bg2, Slot::Bg2),
        // The hero overview's alternating credits row and the modal frame's
        // border rule.
        Surface::CreditsStripe | Surface::PopupBorder => (Slot::Bg1, Slot::Bg1),
        // The selected library pill chip and hint chip 0 (blue), and the
        // queue's selected scope pill (the Direct-remote aqua, `CONTEXT.md`,
        // "Direct remote control" — its own row, not the library pill's blue).
        Surface::PillChipSelected | Surface::HintChip0 => (Slot::Blue, Slot::Blue),
        Surface::QueueScopePillSelected => (Slot::Aqua, Slot::Aqua),
        Surface::HintChip1 => (Slot::Yellow, Slot::Yellow),
        Surface::HintChip2 => (Slot::Orange, Slot::Orange),
    }
}
