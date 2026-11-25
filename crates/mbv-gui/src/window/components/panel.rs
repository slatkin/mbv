//! The player panel (`replace-tauri-with-blitz` task 5.1, the port of
//! `Panel`): the panel's four states — the expanded canvas artboard,
//! its queue folded to one line (`q`), the compact strip for a container
//! shorter than 520 px, and the 72 px rail (`z`).
//!
//! The state comes from [`panel_state`] — the measured panel container size
//! plus the Client core's two toggles — so the window picks the layout from
//! the panel's own size, never the window's, and never from interaction
//! state of its own (the `gui-client` spec). The measurement is the one `App`
//! reports to the core as `LayoutReport.containerSize`, so the painted state
//! and the core's capacity and hit map follow one size.
//!
//! The artwork is not held here: every playback section carries the art
//! reference (design D7), so each state reads it from the section, and a
//! state that mounts later paints the same reference.

use dioxus_native::prelude::*;

use crate::bridge::layout::LayoutSize;
use crate::window::components::collapsed_rail::CollapsedRail;
use crate::window::components::compact_strip::CompactStrip;
use crate::window::components::folded_queue::FoldedQueue;
use crate::window::components::queue_footer::QueueFooter;
use crate::window::components::queue_panel::QueuePanel;
use crate::window::components::queue_playback_panel::QueuePlaybackPanel;
use crate::window::panel::{self, GuiPanelState};
use crate::window::sections::Sections;

/// The props of the player panel (the `Panel` props, in order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct PanelProps {
    /// The panel container's measured painted size (design D10), as `App`
    /// measures and reports it.
    pub container: LayoutSize,
    /// Whether the Library panel shows beside this one (design D10): the
    /// Library Status bar owns the armed prefix and live toast then, so the
    /// queue footer line paints neither.
    #[props(default)]
    pub library_visible: bool,
}

/// The player panel (task 5.1). The panel reads the `panel`, `summary`, and
/// `playback` sections; the state the panel paints follows the container size
/// prop and the `panel` section's toggles.
#[component]
pub fn Panel(props: PanelProps) -> Element {
    let sections = consume_context::<Sections>();

    // The state the panel paints: the toggles and the container size (design
    // D10). The collapse toggle applies only while the Library panel shows
    // beside this one: a queue-only window has nothing to give the freed
    // width to, so a rail there would leave the rest of the window blank.
    let mut toggles = sections.panel.cloned();
    toggles.collapsed &= props.library_visible;
    let layout = panel::panel_state(props.container, &toggles);

    // The compact strip's next-item title: the core's summary names the
    // slot after the playhead independently of the painted viewport (review
    // of this row, finding P2) — the strip paints no Queue list, so its own
    // rows cannot name it.
    let next = sections.summary.cloned().next_title;

    // The art the panel's states adopt (design D7).
    let art = sections.playback.cloned().artwork;

    match layout {
        GuiPanelState::Rail => {
            // The collapsed rail (`z`): the ring, the transport, the
            // remaining time, and the control that expands the panel again.
            rsx! {
                CollapsedRail { art }
            }
        }
        GuiPanelState::CompactStrip => {
            // The compact strip: the container is shorter than 520 px. The
            // strip paints from the playback and summary sections; the queue
            // and the footer are absent, and the core keeps their state.
            rsx! {
                div {
                    class: "flex h-full min-h-0 flex-col",
                    "data-panel-state": "{layout.name()}",
                    CompactStrip { next, art }
                }
            }
        }
        state => {
            // The expanded artboard, with the queue rows or (`q`) their
            // one-line summary.
            rsx! {
                div {
                    class: "flex h-full min-h-0 flex-col",
                    "data-panel-state": "{state.name()}",
                    QueuePlaybackPanel {}
                    div {
                        class: "min-h-0 flex-1",
                        if state == GuiPanelState::ExpandedWithoutQueue {
                            FoldedQueue {}
                        } else {
                            QueuePanel {}
                        }
                    }
                    QueueFooter { library_visible: props.library_visible }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
