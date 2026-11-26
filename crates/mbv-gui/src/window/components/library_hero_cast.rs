//! The Hero's cast grid (gui-hero-design-polish design D2, D3): the movie's
//! cast as the artboard's two-column grid of members, wrapped to one column
//! when the Hero is too narrow. A member paints the photo in the round box,
//! or the initials in it while no photo shows — the photo reference carries
//! the fetch's loading state, and the initials fill paints until it lands.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiHeroCastMemberFrame;
use crate::window::components::art_image::ArtImage;
use crate::window::components::library_hero_frame::own;

/// The round box's side, in CSS pixels.
const PHOTO_BOX_PX: f32 = 40.0;

/// The member's look: the artboard's row padding around the box and the
/// name-and-role column.
const MEMBER_STYLE: &str = "gap: 12px; padding: 10px 12px; border-radius: 8px;";

/// The props of the cast grid (the `LibraryHeroCast` props, in order).
#[derive(Props, Clone, PartialEq)]
pub struct LibraryHeroCastProps {
    /// The cast members, in Emby order.
    pub members: Vec<GuiHeroCastMemberFrame>,
    /// The destination's attribute prefix: empty for Music, as in `shows` or
    /// `emby` for the rest.
    pub attr: &'static str,
    /// The grid's own attribute part, as in `cast`.
    pub hook: &'static str,
}

impl std::fmt::Debug for LibraryHeroCastProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryHeroCastProps")
            .field("members", &self.members)
            .field("attr", &self.attr)
            .field("hook", &self.hook)
            .finish()
    }
}

/// The Hero's cast: the two-column grid that wraps to one column, each member
/// the photo's round box (or the initials in it) beside the name and role.
#[component]
pub fn LibraryHeroCast(props: LibraryHeroCastProps) -> Element {
    let muted = crate::window::theme::canvas_var("muted");
    rsx! {
        div {
            class: "flex flex-wrap items-center",
            style: "gap: 4px 8px;",
            ..vec![Attribute::new(own(props.attr, props.hook), "", None, false)],
            for (index, member) in props.members.iter().enumerate() {
                {
                    let initials = member.initials.clone();
                    rsx! {
                        div {
                            key: "{index}",
                            class: "flex min-w-0 items-center",
                            style: "flex: 1 1 340px; max-width: 100%; {MEMBER_STYLE}",
                            ..vec![Attribute::new(own(props.attr, "cast-member"), "", None, false)],
                            div {
                                class: "flex flex-none items-center justify-center overflow-hidden",
                                style: "width: {PHOTO_BOX_PX}px; height: {PHOTO_BOX_PX}px; border-radius: 999px; background: var(--canvas-library-panel); border: 1px solid var(--canvas-library-control);",
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
                                            style: "font-size: 13px; color: {muted};",
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
                                    style: "font-size: 14px; color: var(--canvas-text);",
                                    ..vec![Attribute::new(own(props.attr, "cast-name"), "", None, false)],
                                    "{member.name}"
                                }
                                span {
                                    class: "font-mono min-w-0",
                                    style: "font-size: 13px; color: var(--canvas-muted);",
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
