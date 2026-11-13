//! Blitz spike: the Queue panel as a Dioxus-native window, in process beside
//! the GUI core thread. The root component lives in `app.rs`.

mod app;
mod art_net;
mod launch;

use futures_channel::mpsc::unbounded;
use mbv_gui::bridge::sections::GuiSectionBatch;
use mbv_gui::core_thread::{self, CoreInput, QuitSink, SectionSink};

// `profiling` drops the allocator so heaptrack sees the real heap (D16).
#[cfg(not(feature = "profiling"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    // The Client declaration comes first: the core thread's Owner connections
    // send the plain Hello that it makes (D15).
    mbv::declare_gui_client();
    let (batch_tx, batch_rx) = unbounded::<GuiSectionBatch>();
    let sink: SectionSink = Box::new(move |batch| {
        // The receiver lives in the window's root. Once the window has closed,
        // a batch has no reader, so the failed send is expected.
        let _ = batch_tx.unbounded_send(batch);
    });
    // The spike has no input (D10), so the core's quit edge has no window
    // handle to close. The window stays open until the user closes it.
    let quit: QuitSink = Box::new(|| {});
    let (core, core_handle) = core_thread::spawn(sink, quit);
    launch::launch(app::root, core.clone(), batch_rx);
    // The window has closed. Close the core thread, then hold the process
    // until its exit teardown completes. The core may have ended already, so
    // a failed send is expected.
    let _ = core.send(CoreInput::Close);
    // A panic in the core thread has already reported through the panic hook.
    if core_handle.join().is_err() {
        std::process::exit(1);
    }
}
