//! The Queue panel's Dioxus root (D7 and D9 of `blitz-queue-panel-spike`). A
//! `use_future` drains the section batches from the GUI core and writes each
//! section into its own signal. The playhead ticks (D8) share the same task,
//! so one future owns the anchor and the tick gate.

mod artwork;
mod playhead;
mod queue_list;
mod scrubber;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use dioxus_native::prelude::*;
use futures_util::{StreamExt, stream};
use mbv_gui::bridge::frame::playback::GuiQueuePlayback;
use mbv_gui::bridge::frame::queue::GuiQueue;
use mbv_gui::bridge::sections::GuiSectionBatch;

use crate::launch::BatchSlot;
use playhead::{Anchor, playhead_write};

/// The Queue panel's stylesheet, with its six `@font-face` rules (D11, D12).
const QUEUE_CSS: &str = include_str!("../assets/queue.css");

/// The window's section signals (D7). Each one holds the latest value its
/// section carried. The children read only the signals they paint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sections {
    queue: Signal<Option<GuiQueue>>,
    playback: Signal<Option<GuiQueuePlayback>>,
    anchor: Signal<Option<Anchor>>,
    position: Signal<f64>,
}

/// The root component. It writes no DOM itself: each child reads the signal
/// it paints.
pub fn root() -> Element {
    let sections = Sections {
        queue: use_signal(|| None),
        playback: use_signal(|| None),
        anchor: use_signal(|| None),
        position: use_signal(|| 0.0),
    };
    let slot = use_hook(consume_context::<BatchSlot>);
    use_future(move || {
        let slot = slot.clone();
        async move {
            if let Some(batches) = slot.take() {
                drain(sections, batches).await;
            }
        }
    });
    rsx! {
        style { {QUEUE_CSS} }
        div { class: "queue-panel",
            artwork::Artwork { playback: sections.playback }
            scrubber::Scrubber { sections }
            queue_list::QueueList { sections }
        }
    }
}

/// One event for the drain task: a section batch or a playhead tick.
enum Event {
    Batch(Box<GuiSectionBatch>),
    Tick,
}

/// Drain the batches and the ticks until the batch sender drops (D7, D8). The
/// gate tells the tick thread whether the anchor plays.
async fn drain(sections: Sections, batches: UnboundedReceiver<GuiSectionBatch>) {
    let gate = Arc::new(AtomicBool::new(false));
    let ticks = playhead::spawn_ticker(Arc::clone(&gate));
    let mut events = stream::select(
        batches.map(|batch| Event::Batch(Box::new(batch))),
        ticks.map(|()| Event::Tick),
    );
    while let Some(event) = events.next().await {
        match event {
            Event::Batch(batch) => apply_batch(sections, &gate, *batch),
            Event::Tick => tick(sections, Instant::now()),
        }
    }
}

/// Write each section the batch carries into its signal. A new playhead
/// anchor sets the position at once and the tick gate for the anchor.
fn apply_batch(sections: Sections, gate: &AtomicBool, batch: GuiSectionBatch) {
    let mut sections = sections;
    if let Some(section) = batch.queue {
        sections.queue.set(Some(section.value));
    }
    if let Some(section) = batch.playback {
        sections.playback.set(Some(section.value));
    }
    if let Some(section) = batch.playhead {
        let now = Instant::now();
        let anchor = Anchor::new(section.value, now);
        gate.store(anchor.plays(), Ordering::Relaxed);
        sections.position.set(anchor.position_at(now));
        sections.anchor.set(Some(anchor));
    }
}

/// Write the predicted position for a tick, if the anchor plays (D8).
fn tick(sections: Sections, now: Instant) {
    let written = sections
        .anchor
        .peek()
        .as_ref()
        .and_then(|anchor| playhead_write(anchor, now));
    if let Some(position) = written {
        let mut position_signal = sections.position;
        position_signal.set(position);
    }
}
