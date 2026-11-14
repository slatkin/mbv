//! The window channel and the core link (D4 of `replace-tauri-with-blitz`):
//! one channel carries everything that reaches the window's Dioxus tree from
//! outside it, and [`CoreLink`] holds the core's channel as the window's
//! context value, with one method per former Tauri command.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender};
use mbv_ui_msg::{KeyEvent, PointerEvent};

use crate::bridge::layout::{LayoutReport, LayoutSize};
use crate::bridge::library::GuiLibraryCommand;
use crate::bridge::sections::GuiSectionBatch;
use crate::bridge::settings::GuiSettingsCommand;
use crate::core_thread::CoreInput;
use crate::window::clock::TimerId;
use crate::window::net::ArtEvent;

/// One event that reaches the window's Dioxus tree from outside it (D4).
#[derive(Debug)]
pub enum WindowEvent {
    /// A section batch from the core thread (D4). The batch is the Rust
    /// value; no serde runs.
    Batch(Box<GuiSectionBatch>),
    /// The core's quit edge (D4): the root answers with the shell's close.
    Quit,
    /// An art route answer (D7): the natural size of a found source, or the
    /// failure of a missed one.
    Art(ArtEvent),
    /// A timer the clock (D10) passed: the id routes it to the surface that
    /// asked for it.
    Timer(TimerId),
    /// One presented frame (D10), sent after each `RedrawRequested` the shell
    /// resolved. `changed` is the launch copy's frame fact
    /// ([`crate::window::layout::frame_carries_change`]): whether that
    /// resolve applied a change no measurement has read, so the drain bumps
    /// the layout epoch for exactly the frames that did. While frames are
    /// wanted the report also paces the frame loop; a paused window still
    /// presents one for every redraw it needs, which is what a change that no
    /// frame has resolved yet waits for.
    Frame {
        /// Whether this frame's resolve applied a change.
        changed: bool,
    },
    /// The window resized or its scale factor changed (D5, D9): `size` is the
    /// window's new layout size in CSS pixels, read from the document's
    /// viewport after the shell applied it. The drain writes it as the size the
    /// window picks its panels and arrangement from, before the frame that
    /// resolves at that size, and clears the [`ViewportPending`] handoff that
    /// held that frame.
    Viewport {
        /// The window's layout size, in CSS pixels.
        size: LayoutSize,
    },
}

/// The viewport handoff (D9): whether a viewport change is still waiting for
/// the drain. Blitz applies a resize to the document's viewport inside its own
/// window-event handler, without touching the tree, so a frame that resolved
/// before the drain read the size lays out the old arrangement at the new size
/// and the corrected one appears a frame or two later — the live resize jerk.
/// The launch copy marks a change queued at the resize site and holds each
/// `RedrawRequested` frame while one is queued; the drain marks it applied once
/// the size has reached the tree.
#[derive(Clone, Debug, Default)]
pub struct ViewportPending(Rc<Cell<bool>>);

impl ViewportPending {
    /// A handoff with no change queued.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A viewport change is queued for the drain: the launch copy holds the
    /// frame until it is applied.
    pub fn queue(&self) {
        self.0.set(true);
    }

    /// The drain applied the queued size to the tree.
    pub fn applied(&self) {
        self.0.set(false);
    }

    /// Whether a queued size has not reached the tree yet.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.0.get()
    }
}

/// The frame gate a resize opens (D9): the frame the resize asked for is held
/// exactly once between presented frames, so the drain reads the new size before
/// a frame resolves at it, and a continuous drag still presents.
///
/// One hold per presented frame, never one per resize: winit dispatches the
/// proxy wake-up before the compositor's resize and that resize's redraw
/// (`winit-wayland` `event_loop/mod.rs`), so every iteration of a drag has both
/// a drain and a new size. A hold that a newer resize re-armed would hold every
/// one of those frames and the window would paint nothing until the drag
/// stopped.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewportFrame {
    /// No resize is waiting for a frame.
    #[default]
    Settled,
    /// A resize landed and no frame has been held since the last presented one:
    /// the next frame is held for the drain.
    Queued,
    /// A frame was held since the last presented one.
    Held,
}

impl ViewportFrame {
    /// A resize landed: the next frame is held for the drain, unless a frame was
    /// already held since the last presented one.
    #[must_use]
    pub fn queued(self) -> Self {
        match self {
            Self::Held => Self::Held,
            Self::Settled | Self::Queued => Self::Queued,
        }
    }

    /// Whether the frame about to resolve is held back, given whether the drain
    /// has read the queued size.
    #[must_use]
    pub fn holds(self, pending: bool) -> bool {
        self == Self::Queued && pending
    }
}

/// The window channel's sender, cloned by every producer (D4).
pub type WindowEventSender = UnboundedSender<WindowEvent>;

/// The receiver the root's drain task takes once (D4). Producers hold
/// senders from before the tree builds, so the receiver travels in a slot
/// and the first caller takes it.
#[derive(Clone, Debug)]
pub struct EventSlot(Arc<Mutex<Option<UnboundedReceiver<WindowEvent>>>>);

impl EventSlot {
    /// A slot that holds `receiver` until the first [`EventSlot::take`].
    #[must_use]
    pub fn new(receiver: UnboundedReceiver<WindowEvent>) -> Self {
        Self(Arc::new(Mutex::new(Some(receiver))))
    }

    /// The receiver, for the first caller only.
    #[must_use]
    pub fn take(&self) -> Option<UnboundedReceiver<WindowEvent>> {
        self.0.lock().ok()?.take()
    }
}

/// The core thread's channel as the window holds it (D4): every former Tauri
/// command is a method, and each push is the command's whole body. A failed
/// send means the core thread is gone — the window's close ended it — so the
/// input logs its drop, as the former Tauri `push` did.
#[derive(Clone, Debug)]
pub struct CoreLink {
    core: Sender<CoreInput>,
}

impl CoreLink {
    /// A link over `core`, the sender [`crate::core_thread::spawn`] returned.
    #[must_use]
    pub fn new(core: Sender<CoreInput>) -> Self {
        Self { core }
    }

    /// The `key` command (D4): the already-translated neutral chord. The
    /// input layer (D8) builds it from the Dioxus key event.
    pub fn key(&self, key: KeyEvent) {
        self.push(CoreInput::Key(key), "key");
    }

    /// The `pointer` command (D4): the already-translated neutral event. The
    /// input layer (D8) sends only a pointer the neutral vocabulary can
    /// represent; an unrepresentable one drops there.
    pub fn pointer(&self, pointer: PointerEvent) {
        self.push(CoreInput::Pointer(pointer), "pointer");
    }

    /// The `report_layout` command (D4): the report applies to the Surface
    /// state's capacity and hit map; it marks no section and forces no batch.
    pub fn layout(&self, report: LayoutReport) {
        self.push(CoreInput::Layout(report), "layout");
    }

    /// The `library` command (D4): the window's semantic request converts to
    /// the neutral command here, as the former Tauri command did with `into`.
    pub fn library(&self, command: GuiLibraryCommand) {
        self.push(CoreInput::Library(command.into()), "library");
    }

    /// The `settings` command (D4): a request this build cannot address — a
    /// setting name no key declares — logs and drops, as the former Tauri command
    /// does.
    pub fn settings(&self, command: GuiSettingsCommand) {
        if let Some(command) = crate::bridge::settings::neutral(command) {
            self.push(CoreInput::Settings(command), "settings");
        } else {
            tracing::debug!(
                target: "gui_commands",
                "the settings command names no setting this build declares; the request drops"
            );
        }
    }

    /// The `sync` command (D4): the window asks for a full snapshot once its
    /// section listener is subscribed.
    pub fn sync(&self) {
        self.push(CoreInput::Sync, "sync");
    }

    /// Push one input into the core channel (D4).
    fn push(&self, input: CoreInput, command: &'static str) {
        if self.core.send(input).is_err() {
            tracing::warn!(
                target: "gui_commands",
                command,
                "the core thread is gone; the input drops"
            );
        }
    }
}
