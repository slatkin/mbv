//! The window channel and the core link (D4 of `replace-tauri-with-blitz`):
//! one channel carries everything that reaches the window's Dioxus tree from
//! outside it, and [`CoreLink`] holds the core's channel as the window's
//! context value, with one method per former Tauri command.

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender};
use mbv_ui_msg::{KeyEvent, PointerEvent};

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
    /// resolved. While frames are wanted it also paces the frame loop; a
    /// paused window still presents one for every redraw, which is what the
    /// drain's mount-time re-measure requests (D12) read.
    Frame,
    /// The window moved, resized, or its scale factor changed (D5, D9). The
    /// layout epoch reads it from task 3.5 on; the root needs no reaction
    /// before then.
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
