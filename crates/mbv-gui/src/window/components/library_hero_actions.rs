//! The Hero's action buttons (`replace-tauri-with-blitz` task 7.5, the port
//! of `LibraryHeroActions`; add-gui-library-panel design D15): Play, Shuffle,
//! and Enqueue, each painted only when the destination supplies its handler.
//! The one set every Hero's action row paints, inside the row
//! `LibraryHeroSummary` or `LibraryHeroFrame` owns, so a fix to one look lands
//! in every Hero.

use dioxus_native::prelude::*;

use crate::window::components::library_hero_frame::own;
use crate::window::icons::{Play, Plus, Shuffle};

/// The props of the action buttons (the `LibraryHeroActions` props, in
/// order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryHeroActionsProps {
    /// The destination's own attribute prefix, as in `shows` or `home`;
    /// empty for Music, whose attributes are `data-library-hero-*`.
    pub attr: &'static str,
    /// Play the selected row; a destination that paints no play omits it.
    #[props(default)]
    pub on_play: Option<Callback<()>>,
    /// Shuffle the selected unit; only a destination that shuffles supplies it.
    #[props(default)]
    pub on_shuffle: Option<Callback<()>>,
    /// Add the selected row to the queue.
    #[props(default)]
    pub on_queue: Option<Callback<()>>,
}

/// The accent Play pill's style, shared with the Playlists hero. The buttons
/// stay a step above the fact pills and under the heading's own size (user
/// request 2026-11-26): 13px text in a 34px pill.
pub(crate) const ACCENT_STYLE: &str = "height: 34px; padding: 0 14px 0 11px; gap: 6px; background: var(--canvas-accent); border: 0; border-radius: 17px; color: var(--canvas-ink); font-size: 13px; font-weight: 600; cursor: pointer;";

/// The destructive action pill's style: the raised pill with the destructive
/// outline and text, for the Playlists hero's Delete.
pub(crate) const DESTRUCTIVE_STYLE: &str = "height: 34px; padding: 0 12px; gap: 6px; background: none; border: 1px solid var(--canvas-destructive-border); border-radius: 17px; color: var(--canvas-destructive); font-size: 13px; cursor: pointer;";

/// The raised action pill's style: the Shuffle and Enqueue buttons' look.
pub(crate) const RAISED_STYLE: &str = "height: 34px; padding: 0 12px; gap: 6px; background: var(--canvas-library-raise); border: 1px solid var(--canvas-library-control); border-radius: 17px; color: var(--canvas-text); font-size: 13px; cursor: pointer;";

/// The Hero's action buttons: the accent Play pill, then the raised Shuffle
/// and Enqueue pills, in that order (user direction 2026-11-27).
#[component]
pub fn LibraryHeroActions(props: LibraryHeroActionsProps) -> Element {
    let play_attrs = vec![Attribute::new(own(props.attr, "play"), "", None, false)];
    let add_attrs = vec![Attribute::new(own(props.attr, "add"), "", None, false)];
    let on_play = props.on_play;
    let on_queue = props.on_queue;
    let on_shuffle = props.on_shuffle;
    let shuffle_attrs = vec![Attribute::new(own(props.attr, "shuffle"), "", None, false)];

    rsx! {
        if let Some(on_play) = on_play {
            button {
                "type": "button",
                class: "flex items-center",
                style: ACCENT_STYLE,
                onclick: move |_| on_play.call(()),
                ..play_attrs,
                Play { size: 14, fill: "var(--canvas-ink)".to_owned(), stroke_width: 0.0 }
                "Play"
            }
            if let Some(on_shuffle) = on_shuffle {
                button {
                    "type": "button",
                    class: "flex items-center",
                    style: RAISED_STYLE,
                    onclick: move |_| on_shuffle.call(()),
                    ..shuffle_attrs,
                    Shuffle { size: 14 }
                    "Shuffle"
                }
            }
            if let Some(on_queue) = on_queue {
                button {
                    "type": "button",
                    class: "flex items-center",
                    style: RAISED_STYLE,
                    onclick: move |_| on_queue.call(()),
                    ..add_attrs,
                    Plus { size: 14 }
                    "Enqueue"
                }
            }
        }
    }
}
