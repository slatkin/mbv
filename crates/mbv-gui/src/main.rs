//! The `mbv-gui` program: a native window Client of the Owner process. The
//! core thread (design D3 of `add-tauri-gui-client`) attaches to the Owner
//! and runs behind the window; the three bridge commands push into its
//! channel, and the `mbv://frame` event carries its frames to the window
//! (design D4, task row 2.5).

use std::ops::Deref;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use mbv_gui::bridge::frame::GuiWindowFrame;
use mbv_gui::bridge::keys::{DomKey, neutral_key};
use mbv_gui::bridge::layout::LayoutReport;
use mbv_gui::bridge::library::GuiLibraryCommand;
use mbv_gui::bridge::pointer::{DomPointer, neutral_pointer};
use mbv_gui::core_thread::{CoreInput, FrameSink, QuitSink};
use mimalloc::MiMalloc;
use tauri::{Emitter, Manager, State};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// The event the core thread's frames reach the window through (design D4).
const FRAME_EVENT: &str = "mbv://frame";

/// The managed sender of the core thread's channel (design D3): every
/// command pushes through it and never touches the `App`. The window's
/// `Destroyed` event pushes a [`CoreInput::Close`] through it too (design
/// D9's close delivery): the App's waker keeps the channel connected for
/// the core thread's whole life, so the close travels as an input, not as
/// a sender drop.
struct CoreChannel(Sender<CoreInput>);

/// The `key` command (design D4): the front end sends every keyboard event
/// with the default action prevented, this translates it to the neutral
/// `KeyEvent` (design D6), and the translated chord pushes into the core
/// channel.
#[tauri::command]
fn key(channel: State<'_, CoreChannel>, event: DomKey) {
    push(channel, CoreInput::Key(neutral_key(event)));
}

/// The `pointer` command (design D4): the DOM pointer event translates to
/// the neutral `PointerEvent`, and an event the neutral vocabulary cannot
/// represent — the middle button, a secondary-button release, a
/// horizontal-only scroll — drops, exactly as the TUI's terminal edge drops
/// it.
#[tauri::command]
fn pointer(channel: State<'_, CoreChannel>, event: DomPointer) {
    if let Some(event) = neutral_pointer(event) {
        push(channel, CoreInput::Pointer(event));
    }
}

/// The `report_layout` command (design D4): the report pushes into the core
/// channel, where the core applies it to the capacity and the hit map and
/// emits the current frame, changed or not (design D3).
#[tauri::command]
fn report_layout(channel: State<'_, CoreChannel>, report: LayoutReport) {
    push(channel, CoreInput::Layout(report));
}

/// The `library` command (add-gui-library-panel design D2): the Library
/// panel's window content sends one semantic request, the bridge translates
/// it to the neutral command, and it pushes into the core channel as a
/// [`CoreInput::Library`].
#[tauri::command]
fn library(channel: State<'_, CoreChannel>, command: GuiLibraryCommand) {
    push(channel, CoreInput::Library(command.into()));
}

/// Push one input into the core channel (design D3): the command hands the
/// state guard over — the Tauri command contract passes the managed state as
/// an owned guard, so the push takes it for the send and reads the channel
/// through it. A failed send means the core thread is gone — the window's
/// close ended it — so the input logs its drop.
fn push(channel: impl Deref<Target = CoreChannel>, input: CoreInput) {
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
    // The core thread's join handle, set in `setup` and joined after the
    // event loop ends: `main` holds the process until the run's exit
    // teardown completes.
    let core_thread: Arc<Mutex<Option<JoinHandle<()>>>> = Arc::new(Mutex::new(None));
    let core_thread_slot = Arc::clone(&core_thread);
    let app = tauri::Builder::default()
        // The webview's own context menu is off (design D11): the script runs
        // before the document parses on every page load, reload included, so
        // a right click reaches the `pointer` command instead of a webview
        // menu.
        .append_invoke_initialization_script(
            "document.addEventListener('contextmenu', (event) => { event.preventDefault(); });",
        )
        // The window's close ends the core thread (design D9): the
        // `Destroyed` event delivers a [`CoreInput::Close`] through the
        // managed channel, because the App's waker keeps the channel
        // connected for the thread's whole life and a sender drop cannot
        // carry the close. The quit edge's own close lands here too, after
        // the run it ends has already returned.
        .on_window_event(|window, event| {
            if window.label() == "main" && matches!(event, tauri::WindowEvent::Destroyed) {
                push(window.app_handle().state::<CoreChannel>(), CoreInput::Close);
            }
        })
        .setup(move |app| {
            // The frames emit from the core thread, any thread, through the
            // window event loop (design D3).
            let handle = app.handle().clone();
            let sink: FrameSink = Box::new(move |frame: GuiWindowFrame| {
                if let Err(error) = handle.emit(FRAME_EVENT, frame) {
                    tracing::warn!(
                        target: "gui_commands",
                        error = %error,
                        "the frame emit failed"
                    );
                }
            });
            // The quit edge closes the window (design D9): the mismatch
            // state's Quit and a routed quit key close it through here, and
            // the join after the event loop holds the process until the
            // core thread's exit teardown completed.
            let quit_handle = app.handle().clone();
            let quit: QuitSink = Box::new(move || {
                if let Some(window) = quit_handle.get_webview_window("main")
                    && let Err(error) = window.close()
                {
                    tracing::warn!(
                        target: "gui_core",
                        error = %error,
                        "the quit edge's window close failed"
                    );
                }
            });
            let (channel, core_handle) = mbv_gui::core_thread::spawn(sink, quit);
            app.manage(CoreChannel(channel));
            *core_thread_slot.lock().unwrap() = Some(core_handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            key,
            pointer,
            report_layout,
            library
        ])
        .build(tauri::generate_context!())?;
    // The event loop ends on the window's destroy or the quit edge's close.
    // The core thread's exit teardown must complete before the process exits
    // (design D9's quit carries the shared teardown), so main joins the core
    // thread here: the Tauri event loop would otherwise exit the process
    // under a teardown still in flight and lose the state it persists.
    let exit_code = app.run_return(|_, _| {});
    if let Some(handle) = core_thread.lock().unwrap().take()
        && let Err(panic) = handle.join()
    {
        tracing::error!(target: "gui_core", panic = ?panic, "the core thread panicked");
    }
    std::process::exit(exit_code)
}
