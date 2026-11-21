//! The Hero's facts block (`replace-tauri-with-blitz` task 7.5, the port of
//! `LibraryHeroSummary`; add-gui-library-panel design D15): the one
//! summary every Library destination's Hero paints — the artwork arm, the
//! title, the meta rows with their duration and progress roles, the hover
//! actions, and the overview. A destination supplies its facts, its own
//! attribute prefix, and the actions it consumes: Podcasts and Feeds paint
//! Play and Add to queue, Shows and Audiobooks paint no action at all because
//! their Workspace owns the row actions, and the rest paint all three.
//!
//! The artwork and the text beside it paint inside `LibraryHeroDetail`, the
//! one detail block every destination's Hero uses, so Home's art takes the same
//! size and place as the art of every other Hero.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiHomeHeroFrame;
use crate::window::components::library_hero_actions::LibraryHeroActions;
use crate::window::components::library_hero_detail::LibraryHeroDetail;
use crate::window::components::library_hero_frame::own;
use crate::window::components::typed_text::TypedText;

/// The props of the summary (the `LibraryHeroSummary` props, in
/// order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryHeroSummaryProps {
    /// The destination's own attribute prefix, as in `shows` or `abs`;
    /// empty for Music, whose attributes are `data-library-hero-*`.
    pub attr: &'static str,
    /// The section's own attribute: the hook the destination's tests take.
    pub section: &'static str,
    /// The selected row's facts, present exactly while the pane paints.
    pub hero: GuiHomeHeroFrame,
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

/// One meta row's role, as the frame's indices mark it: the row the runtime
/// marks paints the duration role, the row the progress marks paints the
/// progress role, and the rest paint the meta role.
fn row_role(
    index: usize,
    duration_row: Option<usize>,
    progress_row: Option<usize>,
) -> (&'static str, bool, &'static str) {
    if Some(index) == duration_row {
        ("duration", true, "var(--canvas-muted)")
    } else if Some(index) == progress_row {
        ("progress", true, "var(--canvas-accent)")
    } else {
        ("meta", false, "var(--canvas-muted)")
    }
}

/// The Hero's facts block: the
/// artwork arm, the title, the meta rows with their role hooks, the hover
/// actions, and the overview.
#[component]
pub fn LibraryHeroSummary(props: LibraryHeroSummaryProps) -> Element {
    rsx! {
        LibraryHeroDetail {
            attr: props.attr,
            block_attr: props.section,
            label: props.hero.title.clone(),
            art_shape: props.hero.art_shape,
            art: props.hero.art.clone(),
            art_title: props.hero.title.clone(),
            h2 {
                style: "margin: 0; font-size: 26px; font-weight: 600; letter-spacing: -0.01em; color: var(--canvas-heading);",
                ..vec![Attribute::new(
                    own(props.attr, "title"),
                    "",
                    None,
                    false,
                )],
                "{props.hero.title}"
            }
            div {
                class: "flex flex-col",
                style: "gap: 4px;",
                for (index, meta) in props.hero.meta_rows.iter().enumerate() {
                    {
                        let (role, tabular, color) = row_role(
                            index,
                            props.hero.duration_row,
                            props.hero.progress_row,
                        );
                        let class = if tabular {
                            "font-mono tabular-nums"
                        } else {
                            "font-mono"
                        };
                        let role_attrs = vec![Attribute::new(
                            own(props.attr, "role"),
                            role,
                            None,
                            false,
                        )];
                        rsx! {
                            span {
                                key: "{index}",
                                class: "{class}",
                                style: "font-size: 13px; color: {color};",
                                ..role_attrs,
                                TypedText { segments: meta.clone() }
                            }
                        }
                    }
                }
            }
            if let Some(on_play) = props.on_play {
                {
                    let on_play_next = props.on_play_next;
                    let on_queue = props.on_queue;
                    rsx! {
                        div {
                            class: "relative flex flex-wrap items-center",
                            style: "gap: 8px;",
                            LibraryHeroActions {
                                attr: props.attr,
                                on_play: Some(on_play),
                                on_play_next,
                                on_queue,
                            }
                        }
                    }
                }
            }
            if let Some(overview) = props.hero.overview.as_ref() {
                {
                    let overview_attrs = vec![Attribute::new(
                        own(props.attr, "overview"),
                        "",
                        None,
                        false,
                    )];
                    rsx! {
                        p {
                            class: "min-w-0",
                            style: "margin: 0; font-size: 14px; line-height: 1.5; color: var(--canvas-muted);",
                            ..overview_attrs,
                            "{overview}"
                        }
                    }
                }
            }
        }
    }
}
