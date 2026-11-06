//! The `mbv-gui` program: a native window Client of the Owner process. The
//! core thread (design D3 of `add-tauri-gui-client`) attaches to the Owner
//! and runs behind the window; the three bridge commands push into its
//! channel, and the `mbv://frame` event carries its frames to the window
//! (design D4, task row 2.5).

use std::sync::mpsc::Sender;

use mbv_gui::bridge::frame::GuiFrame;
use mbv_gui::bridge::keys::{DomKey, neutral_key};
use mbv_gui::bridge::layout::LayoutReport;
use mbv_gui::bridge::pointer::{DomPointer, neutral_pointer};
use mbv_gui::core_thread::{CoreInput, FrameSink};
use mimalloc::MiMalloc;
use tauri::{Emitter, Manager, State};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// The event the core thread's frames reach the window through (design D4).
const FRAME_EVENT: &str = "mbv://frame";

/// The managed sender of the core thread's channel (design D3): every
/// command pushes through it and never touches the `App`. Its drop — the app
/// exiting — ends the core thread's run, together with the waker's sender
/// that dies with the `App`.
struct CoreChannel(Sender<CoreInput>);

/// The `key` command (design D4): the front end sends every keyboard event
/// with the default action prevented, this translates it to the neutral
/// `KeyEvent` (design D6), and the translated chord pushes into the core
/// channel.
#[expect(
    clippy::needless_pass_by_value,
    reason = "the Tauri command macro fixes the by-value argument shapes (CommandArg, State); a sync command keeps input events ordered on the main thread (same class as the mpris interface signatures, issue #804)"
)]
#[tauri::command]
fn key(channel: State<'_, CoreChannel>, event: DomKey) {
    push(&channel, CoreInput::Key(neutral_key(&event)));
}

/// The `pointer` command (design D4): the DOM pointer event translates to
/// the neutral `PointerEvent`, and an event the neutral vocabulary cannot
/// represent — the middle button, a secondary-button release, a
/// horizontal-only scroll — drops, exactly as the TUI's terminal edge drops
/// it.
#[expect(
    clippy::needless_pass_by_value,
    reason = "the Tauri command macro fixes the by-value argument shapes (CommandArg, State); a sync command keeps input events ordered on the main thread (same class as the mpris interface signatures, issue #804)"
)]
#[tauri::command]
fn pointer(channel: State<'_, CoreChannel>, event: DomPointer) {
    if let Some(event) = neutral_pointer(&event) {
        push(&channel, CoreInput::Pointer(event));
    }
}

/// The `report_layout` command (design D4): the report pushes into the core
/// channel, where the core applies it to the capacity and the hit map and
/// emits the current frame, changed or not (design D3).
#[expect(
    clippy::needless_pass_by_value,
    reason = "the Tauri command macro fixes the by-value argument shapes (CommandArg, State); a sync command keeps input events ordered on the main thread (same class as the mpris interface signatures, issue #804)"
)]
#[tauri::command]
fn report_layout(channel: State<'_, CoreChannel>, report: LayoutReport) {
    push(&channel, CoreInput::Layout(report));
}

/// Push one input into the core channel. A failed push means the core thread
/// is gone — an Owner attach failure ends it, and row 2.6 owns that window
/// state — so the input has no receiver left and logs its drop.
fn push(channel: &CoreChannel, input: CoreInput) {
    if channel.0.send(input).is_err() {
        tracing::warn!(
            target: "gui_commands",
            "the core thread is gone; the input drops"
        );
    }
}

fn main() -> tauri::Result<()> {
    // The core thread logs its failures through the app subscriber: install
    // it before the thread spawns, into the GUI's own log file so it does
    // not interleave with the TUI's. A window has no terminal, so no stderr
    // sink runs.
    mbv_core::applog::init(
        false,
        Some(mbv_config::state_dir().join("mbv-gui.log")),
        &mbv_core::applog::LogSpec::default(),
    );
    tauri::Builder::default()
        // The webview's own context menu is off (design D11): the script runs
        // before the document parses on every page load, reload included, so
        // a right click reaches the `pointer` command instead of a webview
        // menu.
        .append_invoke_initialization_script(
            "document.addEventListener('contextmenu', (event) => { event.preventDefault(); });",
        )
        .setup(|app| {
            // The frames emit from the core thread, any thread, through the
            // window event loop (design D3).
            let handle = app.handle().clone();
            let sink: FrameSink = Box::new(move |frame: GuiFrame| {
                if let Err(error) = handle.emit(FRAME_EVENT, frame) {
                    tracing::warn!(
                        target: "gui_commands",
                        error = %error,
                        "the frame emit failed"
                    );
                }
            });
            let channel = mbv_gui::core_thread::spawn(sink);
            app.manage(CoreChannel(channel));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![key, pointer, report_layout])
        .run(tauri::generate_context!())
}
