//! The scrubber (D9 of `blitz-queue-panel-spike`): the played fill, the chapter
//! ticks, the intro and credits marks, and the elapsed and remaining clocks.
//! The marks read the playback section and the anchor's runtime, so a playhead
//! tick repaints only the played fill and the clocks.

use dioxus_native::prelude::*;
use mbv_emby_model::ticks_to_seconds;
use mbv_gui::bridge::frame::playback::GuiQueuePlayback;
use mbv_ui_model::ui_util::fmt_duration_short;

use super::Sections;
use super::playhead::{Anchor, clock_pair, played_fraction};

/// The scrubber's fixed marks, each as a fraction of the runtime.
#[derive(Debug, Clone, Default, PartialEq)]
struct ScrubberMarks {
    chapters: Vec<f64>,
    intro: Option<f64>,
    credits: Option<f64>,
}

/// The marks of `playback` over a runtime of `runtime` ticks. Empty until the
/// playback section arrives.
fn scrubber_marks(playback: Option<&GuiQueuePlayback>, runtime: f64) -> ScrubberMarks {
    let Some(playback) = playback else {
        return ScrubberMarks::default();
    };
    let fraction = |ticks: i64| played_fraction(ticks_to_seconds(ticks), runtime);
    ScrubberMarks {
        chapters: playback
            .chapters
            .iter()
            .map(|chapter| fraction(chapter.start_ticks))
            .collect(),
        intro: playback.intro_mark.map(|mark| fraction(mark.start_ticks)),
        credits: playback.credits_mark.map(|mark| fraction(mark.start_ticks)),
    }
}

/// The scrubber: the track with its marks, and the clocks under it.
#[component]
pub fn Scrubber(sections: Sections) -> Element {
    let marks = use_memo(move || {
        let anchor = sections.anchor.read();
        let runtime = anchor.as_ref().map_or(0.0, Anchor::runtime_secs);
        let playback = sections.playback.read();
        scrubber_marks(playback.as_ref(), runtime)
    });
    let marks = marks.read().clone();
    rsx! {
        div { class: "scrubber",
            div { class: "scrubber-track",
                PlayedFill { sections }
                for ratio in marks.chapters {
                    div { class: "chapter-tick", style: "left: {ratio * 100.0}%" }
                }
                if let Some(ratio) = marks.intro {
                    div { class: "mark mark-intro", style: "left: {ratio * 100.0}%" }
                }
                if let Some(ratio) = marks.credits {
                    div { class: "mark mark-credits", style: "left: {ratio * 100.0}%" }
                }
            }
            Clocks { sections }
        }
    }
}

/// The played fill. It reads the position, so a playhead tick repaints it.
#[component]
fn PlayedFill(sections: Sections) -> Element {
    let fraction = {
        let anchor = sections.anchor.read();
        let runtime = anchor.as_ref().map_or(0.0, Anchor::runtime_secs);
        played_fraction(*sections.position.read(), runtime)
    };
    rsx! {
        div { class: "played", style: "width: {fraction * 100.0}%" }
    }
}

/// The elapsed and remaining clocks. The memo changes only on a whole-second
/// change, so the text repaints at most once a second.
#[component]
fn Clocks(sections: Sections) -> Element {
    let clock = use_memo(move || {
        let anchor = sections.anchor.read();
        clock_pair(anchor.as_ref(), *sections.position.read())
    });
    let (elapsed, remaining) = *clock.read();
    rsx! {
        div { class: "clocks mono",
            span { class: "elapsed", "{fmt_duration_short(elapsed)}" }
            span { class: "remaining", "\u{2212}{fmt_duration_short(remaining)}" }
        }
    }
}
