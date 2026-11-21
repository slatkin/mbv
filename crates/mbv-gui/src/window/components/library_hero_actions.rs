//! The Hero's action buttons (`replace-tauri-with-blitz` task 7.5, the port
//! of `LibraryHeroActions`; add-gui-library-panel design D15): Play,
//! Play next (`n`), and Add to queue (`a`), each painted only when the
//! destination supplies its handler. The one set every Hero's action row
//! paints, inside the row `LibraryHeroSummary` or `LibraryHeroFrame` owns,
//! so a fix to one look lands in every Hero.

use dioxus_native::prelude::*;

use crate::window::components::library_hero_frame::own;
use crate::window::icons::Play;

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
    /// Play the selected row next in the queue.
    #[props(default)]
    pub on_play_next: Option<Callback<()>>,
    /// Add the selected row to the queue.
    #[props(default)]
    pub on_queue: Option<Callback<()>>,
}

/// The hint span a key-hinted button paints after its label (`n`, `a`).
fn paint_hint(hint: &str) -> Element {
    rsx! {
        span {
            class: "font-mono",
            style: "font-size: 12px; color: var(--canvas-muted);",
            "{hint}"
        }
    }
}

/// The accent Play pill's style, shared with the Playlists hero.
pub(crate) const ACCENT_STYLE: &str = "height: 44px; padding: 0 18px 0 14px; gap: 8px; background: var(--canvas-accent); border: 0; border-radius: 22px; color: var(--canvas-ink); font-size: 15px; font-weight: 600; cursor: pointer;";

/// The destructive action pill's style: the raised pill with the destructive
/// outline and text, for the Playlists hero's Delete.
pub(crate) const DESTRUCTIVE_STYLE: &str = "height: 44px; padding: 0 14px; gap: 8px; background: none; border: 1px solid var(--canvas-destructive-border); border-radius: 22px; color: var(--canvas-destructive); font-size: 14px; cursor: pointer;";

/// The raised action pill's style: the Play-next and Add-to-queue buttons'
/// shared look.
pub(crate) const RAISED_STYLE: &str = "height: 44px; padding: 0 14px; gap: 8px; background: var(--canvas-library-raise); border: 1px solid var(--canvas-library-control); border-radius: 22px; color: var(--canvas-text); font-size: 14px; cursor: pointer;";

/// The Hero's action buttons:
/// the accent Play pill, and the raised Play-next and Add-to-queue pills with
/// their muted mono key hints.
#[component]
pub fn LibraryHeroActions(props: LibraryHeroActionsProps) -> Element {
    let play_attrs = vec![Attribute::new(own(props.attr, "play"), "", None, false)];
    let next_attrs = vec![Attribute::new(
        own(props.attr, "play-next"),
        "",
        None,
        false,
    )];
    let add_attrs = vec![Attribute::new(own(props.attr, "add"), "", None, false)];
    let on_play = props.on_play;
    let on_play_next = props.on_play_next;
    let on_queue = props.on_queue;

    rsx! {
        if let Some(on_play) = on_play {
            button {
                "type": "button",
                class: "flex items-center",
                style: ACCENT_STYLE,
                onclick: move |_| on_play.call(()),
                ..play_attrs,
                Play { size: 16, fill: "var(--canvas-ink)".to_owned(), stroke_width: 0.0 }
                "Play"
            }
            if let Some(on_play_next) = on_play_next {
                button {
                    "type": "button",
                    class: "flex items-center",
                    style: RAISED_STYLE,
                    onclick: move |_| on_play_next.call(()),
                    ..next_attrs,
                    "Play next"
                    { paint_hint("n") }
                }
                if let Some(on_queue) = on_queue {
                    button {
                        "type": "button",
                        class: "flex items-center",
                        style: RAISED_STYLE,
                        onclick: move |_| on_queue.call(()),
                        ..add_attrs,
                        "Add to queue"
                        { paint_hint("a") }
                    }
                }
            }
        }
    }
}
