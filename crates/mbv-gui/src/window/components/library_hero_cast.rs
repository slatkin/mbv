//! The Hero's cast grid (gui-hero-design-polish design D2, D3): the movie's
//! cast as the artboard's two-column grid of members, wrapped to one column
//! when the Hero is too narrow. A member paints the photo in the round box,
//! or the initials in it while no photo shows — the photo reference carries
//! the fetch's loading state, and the initials fill paints until it lands.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiHeroCastMemberFrame;
use crate::window::components::art_image::ArtImage;
use crate::window::components::library_hero_frame::{FALLBACK_PANE_WIDTH, HeroPaneWidth, own};

/// The round box's side, in CSS pixels (the artboard's 52 px at two times).
const PHOTO_BOX_PX: f32 = 26.0;

/// The card's look: one bordered, rounded frame that clips its flush rows.
const CARD_STYLE: &str =
    "border: 1px solid var(--canvas-border); border-radius: 10px; overflow: hidden;";

/// The member's look: the artboard's row padding around the box and the
/// name-and-role column.
const MEMBER_STYLE: &str = "gap: 12px; padding: 10px 14px;";

/// The pane width from which the grid paints two columns: two members of 340
/// px and the Hero's side padding.
const TWO_COLUMN_MIN_PANE_PX: f32 = 744.0;

/// The props of the cast grid (the `LibraryHeroCast` props, in order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryHeroCastProps {
    /// The cast members, in Emby order.
    pub members: Vec<GuiHeroCastMemberFrame>,
    /// The destination's attribute prefix: empty for Music, as in `shows` or
    /// `emby` for the rest.
    pub attr: &'static str,
    /// The grid's own attribute part, as in `cast`.
    pub hook: &'static str,
}

/// The Hero's cast: one bordered card of flush rows, two members to a row when
/// the pane is wide and one when it is narrow. The rows alternate two fills
/// (the artboard's zebra), each member the photo's round box (or the
/// initials in it) beside the name and role.
#[component]
pub fn LibraryHeroCast(props: LibraryHeroCastProps) -> Element {
    let muted = crate::window::theme::canvas_var("muted");
    let pane =
        try_consume_context::<HeroPaneWidth>().map_or(FALLBACK_PANE_WIDTH, |pane| *pane.0.read());
    let columns: usize = if pane >= TWO_COLUMN_MIN_PANE_PX { 2 } else { 1 };
    rsx! {
        div {
            style: "display: grid; grid-template-columns: repeat({columns}, minmax(0, 1fr)); {CARD_STYLE}",
            ..vec![Attribute::new(own(props.attr, props.hook), "", None, false)],
            for (index, member) in props.members.iter().enumerate() {
                {
                    let initials = member.initials.clone();
                    let fill = if (index / columns).is_multiple_of(2) {
                        "var(--canvas-library-panel)"
                    } else {
                        "var(--canvas-queue-column)"
                    };
                    rsx! {
                        div {
                            key: "{index}",
                            class: "flex min-w-0 items-center",
                            style: "background: {fill}; {MEMBER_STYLE}",
                            ..vec![Attribute::new(own(props.attr, "cast-member"), "", None, false)],
                            div {
                                class: "flex flex-none items-center justify-center overflow-hidden",
                                style: "width: {PHOTO_BOX_PX}px; height: {PHOTO_BOX_PX}px; border-radius: 999px; background: var(--canvas-library-control);",
                                ..vec![Attribute::new(own(props.attr, "cast-photo"), "", None, false)],
                                ArtImage {
                                    art: member.photo.clone(),
                                    alt: member.name.clone(),
                                    uncapped: true,
                                    class: "block".to_owned(),
                                    style: "width: 100%; height: 100%; object-fit: cover;".to_owned(),
                                    fallback: Some(rsx! {
                                        span {
                                            class: "font-mono",
                                            style: "font-size: 10px; color: {muted};",
                                            ..vec![Attribute::new(own(props.attr, "cast-initials"), "", None, false)],
                                            "{initials}"
                                        }
                                    }),
                                }
                            }
                            div {
                                class: "flex min-w-0 flex-col",
                                style: "gap: 2px;",
                                span {
                                    class: "min-w-0",
                                    style: "font-size: 15px; color: var(--canvas-text);",
                                    ..vec![Attribute::new(own(props.attr, "cast-name"), "", None, false)],
                                    "{member.name}"
                                }
                                span {
                                    class: "font-mono min-w-0",
                                    style: "font-size: 12px; color: var(--canvas-muted);",
                                    ..vec![Attribute::new(own(props.attr, "cast-role"), "", None, false)],
                                    "{member.role}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
