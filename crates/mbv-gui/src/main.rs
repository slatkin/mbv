//! The `mbv` program: the GUI Client's window process on Dioxus Native
//! and Blitz (D3, D4 of `replace-tauri-with-blitz`). The core thread attaches
//! to the Owner and runs behind the window; the window channel (D4) carries
//! its batches and the quit edge to the root, and `CoreLink` carries the
//! window's commands back. The window shares this process, so this is the only
//! GUI program.

use std::sync::Arc;
use std::thread::JoinHandle;

use futures_channel::mpsc::unbounded;
use mbv_gui::core_thread::{CoreInput, QuitSink, SectionSink};
use mbv_gui::window::app::App;
use mbv_gui::window::channel::WindowEvent;
use mbv_gui::window::clock::{Clock, TimerClock};
use mbv_gui::window::launch::launch;
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
    // Claim the one-shot swap token an Owner Open launch sets, before any
    // thread or child process spawns: the token is read once and no child
    // inherits it, and the first Owner Hello presents it.
    mbv_client_core::pin::claim_swap_token();
    // The core thread logs its failures through the app subscriber: install
    // it before the thread spawns, into the GUI's own log file so it does not
    // interleave with the TUI's (as the earlier build did). A window
    // has no terminal, so no stderr sink runs.
    mbv_core::applog::init(
        false,
        Some(mbv_config::state_dir().join("mbv-gui.log")),
        &mbv_core::applog::LogSpec::default(),
    );
    // The window channel (D4): the core thread's sinks push into it, and the
    // root's drain task reads it. Once the window has closed, an event has no
    // reader, so a failed send is expected.
    let (events, event_receiver) = unbounded::<WindowEvent>();
    let sink: SectionSink = Box::new({
        let events = events.clone();
        move |batch| {
            let _ = events.unbounded_send(WindowEvent::Batch(Box::new(batch)));
        }
    });
    // The quit edge sends `Quit`; the root answers with the shell's close,
    // which ends the event loop (D4) and lets the join below run.
    let quit: QuitSink = Box::new({
        let events = events.clone();
        move || {
            let _ = events.unbounded_send(WindowEvent::Quit);
        }
    });
    let (core, core_handle) = mbv_gui::core_thread::spawn(sink, quit);
    // The window's clocks (D10): the timer thread and the frame gate. The
    // root's drain task routes a passed timer to the surface that asked for
    // it, and the playhead gates its frames.
    let clock: Arc<dyn Clock> = Arc::new(TimerClock::new(events.clone()));
    launch(App, core.clone(), events, event_receiver, clock);
    // The window has closed. Close the core thread, then hold the process
    // until its exit teardown completes (D4): the run's shared teardown
    // persists its state, and the join would otherwise let the process exit
    // under a teardown still in flight. The core may have ended already, so a
    // failed send is expected.
    let _ = core.send(CoreInput::Close);
    join_core(core_handle);
}

/// Hold the process until the core thread's exit teardown completed (D4). A
/// panic in the core thread has already reported through the panic hook.
fn join_core(handle: JoinHandle<()>) {
    if handle.join().is_err() {
        std::process::exit(1);
    }
}
