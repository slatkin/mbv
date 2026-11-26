//! The Hero's section band (gui-hero-design-polish design D2): a titled
//! header — the artboard's small caps word with the rule that fills the rest
//! of the line — over the section's children. The Overview, Cast, and list
//! bands (Tracks, Episodes, Chapters, Entries) each paint through it, so one
//! header look carries every Hero section.

use dioxus_native::prelude::*;

use crate::window::components::library_hero_frame::own;

/// The section title's look: the kicker's label font (design D2), in the
/// artboard's small muted capitals.
const TITLE_STYLE: &str = "font-size: 12px; letter-spacing: 0.08em; text-transform: uppercase; color: var(--canvas-muted);";

/// The props of the section band (the `LibraryHeroSection` props, in order).
#[derive(Props, Clone, PartialEq)]
pub struct LibraryHeroSectionProps {
    /// The section's title word, as the header paints it.
    pub title: String,
    /// The destination's attribute prefix: empty for Music, as in `shows` or
    /// `emby` for the rest.
    pub attr: &'static str,
    /// The section's own attribute part, as in `overview`, `cast`, or `list`.
    pub hook: &'static str,
    /// The section's content.
    pub children: Element,
}

impl std::fmt::Debug for LibraryHeroSectionProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryHeroSectionProps")
            .field("title", &self.title)
            .field("attr", &self.attr)
            .field("hook", &self.hook)
            .finish_non_exhaustive()
    }
}

/// The Hero's section: the titled header with a rule over its children.
#[component]
pub fn LibraryHeroSection(props: LibraryHeroSectionProps) -> Element {
    rsx! {
        section {
            class: "flex flex-col",
            style: "gap: 14px;",
            "aria-label": "{props.title}",
            ..vec![Attribute::new(own(props.attr, props.hook), "", None, false)],
            div {
                class: "flex items-center",
                style: "gap: 12px;",
                span {
                    class: "font-mono",
                    style: "{TITLE_STYLE}",
                    "{props.title}"
                }
                div {
                    style: "flex: 1; height: 1px; background: var(--canvas-border);",
                }
            }
            { props.children }
        }
    }
}
