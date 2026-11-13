//! Blitz spike: the Queue panel as a Dioxus-native window, in process beside
//! the GUI core thread. The root component is a placeholder until task 5.1 of
//! `blitz-queue-panel-spike` adds the Queue panel.

mod art_net;
mod launch;

use dioxus_native::prelude::*;
use futures_channel::mpsc::unbounded;
use futures_util::StreamExt;
use mbv_gui::bridge::sections::GuiSectionBatch;
use mbv_gui::core_thread::{self, CoreInput, QuitSink, SectionSink};

use crate::launch::BatchSlot;

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
    launch::launch(root, core.clone(), batch_rx);
    // The window has closed. Close the core thread, then hold the process
    // until its exit teardown completes. The core may have ended already, so
    // a failed send is expected.
    let _ = core.send(CoreInput::Close);
    // A panic in the core thread has already reported through the panic hook.
    if core_handle.join().is_err() {
        std::process::exit(1);
    }
}

/// The root component. Until task 5.1 it only drains the batch receiver.
fn root() -> Element {
    let slot = use_hook(consume_context::<BatchSlot>);
    use_future(move || {
        let slot = slot.clone();
        async move {
            if let Some(mut batches) = slot.take() {
                while batches.next().await.is_some() {}
            }
        }
    });
    rsx! { div { "mbv" } }
}
