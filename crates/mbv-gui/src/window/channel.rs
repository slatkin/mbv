//! The window channel and the core link (D4 of `replace-tauri-with-blitz`):
//! one channel carries everything that reaches the window's Dioxus tree from
//! outside it, and [`CoreLink`] holds the core's channel as the window's
//! context value, with one method per command. The one fact that
//! does not travel it is a resize's layout size, which the launch copy writes
//! into the tree directly so that it lands before the frame that resolves at
//! the new size (`window::layout::ViewportSize`, D9).

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender};
use mbv_ui_msg::{KeyEvent, PointerEvent};

use crate::bridge::keys::KeyTarget;
use crate::bridge::layout::LayoutReport;
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
    /// value; no wire encoding runs.
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
    /// The window resized or its scale factor changed (D5, D9): the scale
    /// factor the art caps divide by (D7) is the drain's. The layout size does
    /// not travel here — the launch copy writes it into the tree before the
    /// frame that resolves at the new size (D9), because a channel hop lands a
    /// turn after that resolve.
    Viewport,
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

/// The core thread's channel as the window holds it (D4): every
/// command is a method, and each push is the command's whole body. A failed
/// send means the core thread is gone — the window's close ended it — so the
/// input logs its drop, as a full channel does.
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

    /// The `key` command (D4): the already-translated neutral chord, with the
    /// panel the window's pointer resolved for it (design D2 of
    /// `gui-pointer-target-interaction`). The input layer (D8) builds both from
    /// the Dioxus key event and the key router's answer.
    pub fn key(&self, key: KeyEvent, target: KeyTarget) {
        self.push(CoreInput::Key { key, target }, "key");
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
    /// the neutral command here, with `into`.
    pub fn library(&self, command: GuiLibraryCommand) {
        self.push(CoreInput::Library(command.into()), "library");
    }

    /// The `settings` command (D4): a request this build cannot address — a
    /// setting name no key declares — logs and drops,
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
