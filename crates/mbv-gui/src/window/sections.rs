//! The window's section stores (D6 of `replace-tauri-with-blitz`): the port
//! of `lib/bridge/sections.ts`. One signal for each window section, fed by
//! the one section batch the core thread sends per tick. [`Sections::apply_batch`]
//! writes a section's signal only when the batch's revision differs from the
//! one the window holds, so a `sync` re-send of a section the window already
//! holds writes nothing, and every write of one batch happens in one task
//! poll, so Dioxus renders the batch once and no frame shows half a batch
//! (ADR 0038).
//!
//! The stores hold no interaction state — the core thread owns that — so a
//! window start calls `CoreLink::sync` and repaints from the full batch the
//! core answers with (D4). Components read their sections from the
//! [`Sections`] context value; a signal write notifies only that signal's
//! readers, so a `queue` batch re-renders only the queue.

use dioxus_native::prelude::*;

use mbv::NowPlayingMediaKind;

use crate::bridge::frame::chrome::{GuiFooterPill, GuiQueueFooter, GuiStatusBar, GuiToasts};
use crate::bridge::frame::library::{
    GuiLibraryDetail, GuiLibraryList, GuiLibraryRowMenuFrame, GuiSearchFrame,
};
use crate::bridge::frame::overlay::GuiOverlayFrame;
use crate::bridge::frame::playback::{
    GuiPlayerControls, GuiQueuePlayback, GuiScrubberHover, GuiTransportAvailability,
};
use crate::bridge::frame::playlists::GuiPlaylistsFrame;
use crate::bridge::frame::queue::{GuiQueue, GuiQueueHeaderFacts, GuiQueueSummary};
use crate::bridge::frame::settings::GuiSettingsFrame;
use crate::bridge::frame::theme::{GuiRole, GuiTheme};
use crate::bridge::frame::{GuiAttachState, GuiPanelToggles};
use crate::bridge::sections::{GuiSection, GuiSectionBatch};

mod shared;

pub use shared::SectionRef;
use shared::shared_option;

/// The window's section stores (D6): one signal per section, the
/// context-provided port of the store module. Create once in the root's
/// `use_hook` and provide as context; the signals need the root's scope.
#[derive(Clone)]
pub struct Sections {
    /// The pre-attach state the window shows, `None` once the attach
    /// succeeds and before the first batch lands.
    pub attach: Signal<Option<GuiAttachState>>,
    /// The role-to-colour map of the last carried theme section, `None`
    /// before the first batch. The colours the window paints live in the CSS
    /// custom properties the theme element (`window/theme.rs`) writes.
    pub theme: Signal<Option<GuiTheme>>,
    /// The Queue rows inside the viewport, with the cursor, marks, and the
    /// active slot, as the one handle every reader clones.
    pub queue: Signal<SectionRef<GuiQueue>>,
    /// The queue summary: the remaining count, the finish clock, the right
    /// column's mode, and the filter.
    pub summary: Signal<GuiQueueSummary>,
    /// The panel toggles the window picks the layout with, and the
    /// UI-reset counter that resets its interaction stores.
    pub panel: Signal<GuiPanelToggles>,
    /// The open menu or popover, `None` while none is open.
    pub overlay: Signal<Option<GuiOverlayFrame>>,
    /// The transport band's facts, without the hover preview and without the
    /// playhead facts (D6: they ride the `playhead` anchor,
    /// `window/playhead.rs`).
    pub playback: Signal<GuiQueuePlayback>,
    /// The scrubber's hover preview, `None` while the pointer is away from
    /// the bar.
    pub hover: Signal<Option<GuiScrubberHover>>,
    /// The Queue footer's status pills.
    pub footer: Signal<GuiQueueFooter>,
    /// The Status bar projection.
    pub status: Signal<GuiStatusBar>,
    /// The live toasts the toast stack paints as floating cards, oldest
    /// first (design D6 of `gui-floating-toasts`).
    pub toasts: Signal<GuiToasts>,
    /// The Library list: the destination tabs and the active destination's
    /// list part, `None` while no presented destination is active, as the one
    /// handle every reader clones.
    pub library_list: Signal<Option<SectionRef<GuiLibraryList>>>,
    /// The Library detail: the active destination's selection, Hero,
    /// Workspace, and playing marks, `None` while no presented destination
    /// is active, as the one handle every reader clones.
    pub library_detail: Signal<Option<SectionRef<GuiLibraryDetail>>>,
    /// The right-click item menu the window last opened, `None` while none
    /// is open.
    pub row_menu: Signal<Option<GuiLibraryRowMenuFrame>>,
    /// The Search section's results, `None` while the field is closed, as the
    /// one handle every reader clones.
    pub search: Signal<Option<SectionRef<GuiSearchFrame>>>,
    /// The Settings screen, `None` while the screen is closed, as the one
    /// handle every reader clones.
    pub settings: Signal<Option<SectionRef<GuiSettingsFrame>>>,
    /// The Playlists screen, `None` while the screen is closed.
    pub playlists: Signal<Option<GuiPlaylistsFrame>>,
    /// Whether a section batch has arrived: the window paints the main
    /// content while this holds and the `attach` section is `None`, and the
    /// attach screen before that — the wait before the first batch is not
    /// the attached state's own `None` (D6).
    pub connected: Signal<bool>,
    /// The revision of each section the window last wrote (D6): a batch
    /// section at a revision the window holds writes nothing. Not
    /// presentation state — no component reads it.
    held: Signal<Held>,
}

impl std::fmt::Debug for Sections {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sections")
            .field("connected", &self.connected.peek().clone())
            .finish_non_exhaustive()
    }
}

/// The revision of each section the window last wrote (D6): a batch section
/// at a revision the window holds writes nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Held {
    attach: Option<u64>,
    theme: Option<u64>,
    queue: Option<u64>,
    summary: Option<u64>,
    panel: Option<u64>,
    overlay: Option<u64>,
    playback: Option<u64>,
    hover: Option<u64>,
    footer: Option<u64>,
    status: Option<u64>,
    toasts: Option<u64>,
    library_list: Option<u64>,
    library_detail: Option<u64>,
    row_menu: Option<u64>,
    search: Option<u64>,
    settings: Option<u64>,
    playlists: Option<u64>,
}

impl Default for Sections {
    fn default() -> Self {
        Self::new()
    }
}

impl Sections {
    /// The fresh-window stores (D6): the value each section signal holds
    /// before the first batch arrives. The window paints no main content
    /// before that batch, so these are the bare values a start paints from,
    /// not painted state. Call from a component (the signals need its
    /// scope) — [`Sections::default`] is the same constructor under the
    /// clippy-mandated name.
    #[must_use]
    pub fn new() -> Self {
        Self {
            attach: Signal::new(None),
            theme: Signal::new(None),
            queue: Signal::new(SectionRef::new(fresh_queue())),
            summary: Signal::new(fresh_summary()),
            panel: Signal::new(fresh_panel()),
            overlay: Signal::new(None),
            playback: Signal::new(fresh_playback()),
            hover: Signal::new(None),
            footer: Signal::new(fresh_footer()),
            status: Signal::new(fresh_status()),
            toasts: Signal::new(fresh_toasts()),
            library_list: Signal::new(None),
            library_detail: Signal::new(None),
            row_menu: Signal::new(None),
            search: Signal::new(None),
            settings: Signal::new(None),
            playlists: Signal::new(None),
            connected: Signal::new(false),
            held: Signal::new(Held::default()),
        }
    }

    /// The revision of the `libraryList` section the window holds (D6),
    /// `None` before the first batch carries one. The row memo keys the
    /// carried artists on it: a section's value at one revision is immutable,
    /// so equal revisions carry equal content and the memo's hit test stays
    /// cheap in the content's size.
    #[must_use]
    pub fn library_list_revision(&self) -> Option<u64> {
        self.held.peek().library_list
    }

    /// Write a batch into the section signals (D6): each present section
    /// whose revision the window does not hold replaces its signal's value.
    /// Sections the batch leaves out leave their signals as they are. Every
    /// write of one batch happens here, in the caller's one task poll, so
    /// Dioxus renders the batch once.
    pub fn apply_batch(&mut self, batch: &GuiSectionBatch) {
        self.connected.set(true);
        let mut held = self.held.write();
        write(batch.attach.as_ref(), &mut held.attach, self.attach);
        write(batch.theme.as_ref(), &mut held.theme, self.theme);
        write_shared(
            batch.queue.as_ref(),
            &mut held.queue,
            self.queue,
            SectionRef::new,
        );
        write(batch.summary.as_ref(), &mut held.summary, self.summary);
        write(batch.panel.as_ref(), &mut held.panel, self.panel);
        write(batch.overlay.as_ref(), &mut held.overlay, self.overlay);
        write(batch.playback.as_ref(), &mut held.playback, self.playback);
        write(batch.hover.as_ref(), &mut held.hover, self.hover);
        write(batch.footer.as_ref(), &mut held.footer, self.footer);
        write(batch.status.as_ref(), &mut held.status, self.status);
        write(batch.toasts.as_ref(), &mut held.toasts, self.toasts);
        write_shared(
            batch.library_list.as_ref(),
            &mut held.library_list,
            self.library_list,
            shared_option,
        );
        write_shared(
            batch.library_detail.as_ref(),
            &mut held.library_detail,
            self.library_detail,
            shared_option,
        );
        write(batch.row_menu.as_ref(), &mut held.row_menu, self.row_menu);
        write_shared(
            batch.search.as_ref(),
            &mut held.search,
            self.search,
            shared_option,
        );
        write_shared(
            batch.settings.as_ref(),
            &mut held.settings,
            self.settings,
            shared_option,
        );
        write(
            batch.playlists.as_ref(),
            &mut held.playlists,
            self.playlists,
        );
    }
}

/// Write one carried section when its revision is new (D6): a section at the
/// revision the window holds is ignored, and a section at any other revision
/// — lower or higher — is written.
fn write<T: Clone + 'static, S: From<T> + 'static>(
    carried: Option<&GuiSection<T>>,
    held: &mut Option<u64>,
    signal: Signal<S>,
) {
    write_shared(carried, held, signal, S::from);
}

/// Write one carried section as one shared handle (`share-gui-sections-by-rc`
/// design D2): the batch carries an owned value across the core channel, and
/// the window builds the one [`SectionRef`] its readers clone here, inside the
/// write, so a section update copies its content once however many readers it
/// has.
fn write_shared<T: Clone + 'static, S: 'static>(
    carried: Option<&GuiSection<T>>,
    held: &mut Option<u64>,
    mut signal: Signal<S>,
    share: impl FnOnce(T) -> S,
) {
    let Some(section) = carried else {
        return;
    };
    if *held == Some(section.revision) {
        return;
    }
    *held = Some(section.revision);
    signal.set(share(section.value.clone()));
}

/// The fresh queue: an empty viewport.
fn fresh_queue() -> GuiQueue {
    GuiQueue {
        rows: Vec::new(),
        cursor: None,
        marks: Vec::new(),
        active: None,
        hover: None,
        drag: None,
        scroll: None,
    }
}

/// The fresh summary: nothing played, no filter, no chooser entry, and the
/// queue's total unknown.
fn fresh_summary() -> GuiQueueSummary {
    GuiQueueSummary {
        remaining: 0,
        next_title: None,
        tail_secs: None,
        filter: String::new(),
        filter_active: false,
        total_secs: 0,
        total_partial: false,
        marked_secs: None,
        marked_partial: false,
        header: GuiQueueHeaderFacts {
            shown_name: String::new(),
            shown_len: 0,
            close_disabled: true,
        },
    }
}

/// The fresh panel toggles: expanded and unfolded.
fn fresh_panel() -> GuiPanelToggles {
    GuiPanelToggles {
        collapsed: false,
        queue_folded: false,
        ui_reset_epoch: 0,
    }
}

/// The fresh transport band: idle, with nothing to show.
fn fresh_playback() -> GuiQueuePlayback {
    GuiQueuePlayback {
        host: String::new(),
        host_is_remote: false,
        shown: false,
        show_controls: false,
        now_playing_title: None,
        now_playing_media: NowPlayingMediaKind::default(),
        parent_line: None,
        episode_badge: None,
        availability: GuiTransportAvailability {
            stop: false,
            next: false,
            previous: false,
        },
        status_indicators: None,
        artwork: None,
        track_chips: None,
        target_choices: Vec::new(),
        controls: GuiPlayerControls::default(),
    }
}

/// The fresh footer: an empty playlist pill, no autosave, no dirty marker.
fn fresh_footer() -> GuiQueueFooter {
    GuiQueueFooter {
        playlist: GuiFooterPill {
            text: String::new(),
            role: GuiRole::TextPrimary,
            bold: false,
        },
        autosave: None,
        dirty: None,
    }
}

/// The fresh Status bar: no armed prefix.
fn fresh_status() -> GuiStatusBar {
    GuiStatusBar {
        prefix_armed: false,
    }
}

/// The fresh toast stack: no live toast.
fn fresh_toasts() -> GuiToasts {
    GuiToasts::default()
}

#[cfg(test)]
mod tests;
