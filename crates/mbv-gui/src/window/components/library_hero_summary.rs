//! The Hero's facts block (`replace-tauri-with-blitz` task 7.5, the port of
//! `LibraryHeroSummary.svelte`; add-gui-library-panel design D15): the one
//! summary every Library destination's Hero paints — the artwork arm, the
//! title, the meta rows with their duration and progress roles, the hover
//! actions, and the overview. A destination supplies its facts, its own
//! attribute prefix, and the actions it consumes: Podcasts and Feeds paint
//! Play and Add to queue, Shows and Audiobooks paint no action at all because
//! their Workspace owns the row actions, and the rest paint all three.
//!
//! The landscape arm takes the TUI's layout here, once (design D15): the 16:9
//! still fills the Hero content width above the title and meta block, and the
//! portrait and square arms keep the canvas's 300 px box beside the text. A
//! destination that paints a Hero adopts that rule rather than restating it.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiArtShapeFrame, GuiHomeHeroFrame};
use crate::window::components::library_hero_actions::LibraryHeroActions;
use crate::window::components::library_hero_art::LibraryHeroArt;
use crate::window::components::library_hero_frame::own;
use crate::window::components::typed_text::TypedText;

/// The props of the summary (the `LibraryHeroSummary.svelte` props, in
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

/// Whether the section stacks the still above the facts.
fn band(hero: &GuiHomeHeroFrame) -> bool {
    hero.art_shape == GuiArtShapeFrame::Landscape
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

/// The Hero's facts block (the Svelte markup, character for character): the
/// artwork arm, the title, the meta rows with their role hooks, the hover
/// actions, and the overview.
#[component]
pub fn LibraryHeroSummary(props: LibraryHeroSummaryProps) -> Element {
    let band = band(&props.hero);
    let attrs = vec![Attribute::new(props.section, "", None, false)];

    rsx! {
        section {
            class: if band { "flex flex-col" } else { "flex items-start" },
            style: if band { "gap: 18px;" } else { "gap: 28px;" },
            "aria-label": "{props.hero.title}",
            ..attrs,
            LibraryHeroArt {
                shape: props.hero.art_shape,
                art: props.hero.art.clone(),
                title: props.hero.title.clone(),
                attr: props.attr,
                placeholder: None,
            }
            div {
                class: "flex min-w-0 flex-1 flex-col",
                style: if band { "gap: 14px; padding-top: 0;" } else { "gap: 14px; padding-top: 8px;" },
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
}
