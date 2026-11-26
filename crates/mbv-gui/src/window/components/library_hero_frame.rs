//! The Library Hero's frame (gui-hero-design-polish design D2): the one
//! layout every Hero paints — Music, Shows, Emby, Audiobooks, Podcasts,
//! Feeds, and Home. Top to bottom:
//!
//!   1. the backdrop layer — the blurred copy of the Hero's art (design D4),
//!      cover-fit behind the header band under a fade to the pane's
//!      background, when the destination passes one;
//!   2. the header band — the title and the fact pills row, with an optional
//!      action on its right;
//!   3. the detail block — the artwork box (inset frame)
//!      and, beside it, the picked item's eyebrow, name, block pills, action
//!      row, and labelled fact rows;
//!   4. the bands, each under a titled section header with a rule — the
//!      Overview (when the item has one), the Cast (when the item has cast),
//!      and, when the destination passes a `list_title` and children, the
//!      destination's list band.
//!
//! A destination supplies facts and rows and never restates the layout, so a
//! fix here lands in all of them. The detail block — the artwork box and the
//! text beside it — is `LibraryHeroDetail`'s, so the art's size and place are
//! the same in every destination.
//!
//! `attr` names the destination's own attributes: the Music Hero's are
//! `data-library-hero-*` (`attr` empty) and the Shows Hero's are
//! `data-library-shows-hero-*`, so a destination's tests keep their hooks.

use dioxus_native::prelude::*;

use crate::art::GuiArtRef;
use crate::art::backdrop::BACKDROP_HEIGHT;
use crate::bridge::frame::library::{
    GuiArtShapeFrame, GuiHeroCastMemberFrame, GuiHeroFactRowFrame, GuiHeroPillFrame,
};
use crate::window::components::art_image::ArtImage;
use crate::window::components::library_attr::library_attr;
use crate::window::components::library_hero_cast::LibraryHeroCast;
use crate::window::components::library_hero_detail::LibraryHeroDetail;
use crate::window::components::library_hero_fact_rows::LibraryHeroFactRows;
use crate::window::components::library_hero_pills::LibraryHeroPills;
use crate::window::components::library_hero_section::LibraryHeroSection;
use crate::window::components::typed_text::TypedText;
use crate::window::library::hero::Segments;

/// The props of the frame (the `LibraryHeroFrame` props, in order).
#[derive(Props, Clone, PartialEq)]
pub struct LibraryHeroFrameProps {
    /// The destination's attribute prefix: empty for Music, `shows` for
    /// Shows.
    pub attr: &'static str,
    /// The root's own attribute, the hook the panel's tests take.
    pub root: &'static str,
    /// The header band: always painted, so every Hero shares the one look.
    /// The pills carry each value's data type (design D14).
    pub header: HeroHeader,
    /// The header band's trailing control, when the destination has one.
    #[props(default)]
    pub header_action: Option<Element>,
    /// The detail block's text: each line paints only when it is non-empty.
    pub block: HeroBlock,
    /// The artwork box's shape and its fetched art.
    pub art_shape: GuiArtShapeFrame,
    pub art: Option<GuiArtRef>,
    /// The alternative text of the artwork.
    pub art_title: String,
    /// The artwork box's placeholder while no art is carried.
    #[props(default)]
    pub art_placeholder: Option<String>,
    /// The detail block's action row.
    #[props(default)]
    pub actions: Option<Element>,
    /// The item's overview, under its Overview section (design D2: the
    /// overview leaves the detail block).
    #[props(default)]
    pub overview: Option<String>,
    /// The item's cast, under its Cast section (design D3).
    #[props(default)]
    pub cast: Vec<GuiHeroCastMemberFrame>,
    /// The list band's section title, as in `Episodes`. A titled list band
    /// paints inside a section header; a list band with no title paints its
    /// children bare, as the Music track list does (user direction 2026-11-26).
    #[props(default)]
    pub list_title: Option<String>,
    /// The list band.
    #[props(default)]
    pub children: Option<Element>,
    /// The blurred-artwork backdrop (design D4): the baked image the frame paints
    /// behind the header and the detail block. `None` paints no backdrop.
    #[props(default)]
    pub backdrop: Option<GuiArtRef>,
}

impl std::fmt::Debug for LibraryHeroFrameProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryHeroFrameProps")
            .field("attr", &self.attr)
            .field("root", &self.root)
            .field("header", &self.header)
            .field("header_action", &self.header_action.is_some())
            .field("block", &self.block)
            .field("art_shape", &self.art_shape)
            .field("art", &self.art)
            .field("art_title", &self.art_title)
            .field("art_placeholder", &self.art_placeholder)
            .field("actions", &self.actions.is_some())
            .field("overview", &self.overview)
            .field("cast", &self.cast)
            .field("list_title", &self.list_title)
            .field("children", &self.children.is_some())
            .field("backdrop", &self.backdrop)
            .finish()
    }
}

/// The header band's lines.
#[derive(Debug, Clone, PartialEq)]
pub struct HeroHeader {
    /// The big title: the largest unit.
    pub title: String,
    /// The fact pills, in paint order (design D2: the facts line's
    /// replacement).
    pub pills: Vec<GuiHeroPillFrame>,
}

/// The detail block's text: each line
/// paints only when it is non-empty.
#[derive(Debug, Clone, PartialEq)]
pub struct HeroBlock {
    /// The block's label: the item's title.
    pub label: String,
    /// The block's label line, when the item has one.
    pub eyebrow: Option<Segments>,
    /// The selected item's name, only when a parent took the header.
    pub name: Option<String>,
    /// Whether the name line names an episode, which paints one step smaller
    /// than a series-level item (user request 2026-11-24).
    pub name_episode: bool,
    /// The block's pills, in paint order (design D2: the album's totals).
    pub pills: Vec<GuiHeroPillFrame>,
    /// The labelled fact rows, in paint order (design D2: the `meta` rows'
    /// replacement).
    pub rows: Vec<GuiHeroFactRowFrame>,
}

/// Whether a line has anything to paint: the line
/// carries at least one segment.
fn painted(segments: Option<&Segments>) -> bool {
    segments.is_some_and(|segments| !segments.is_empty())
}

/// Whether a line has a name to paint: the selected item's name, only when a
/// parent took the header (the former client `(block.name ?? '') !== ''`).
fn named(name: Option<&str>) -> bool {
    name.is_some_and(|name| !name.is_empty())
}

/// The attribute name of one part of the destination's Hero (the former client
/// `own` helper): the destination's prefix sits before the shared `hero`
/// segment, and Music's empty prefix names the shared hooks.
#[must_use]
pub(super) fn own(attr: &'static str, part: &str) -> &'static str {
    let prefix = if attr.is_empty() {
        "hero".to_owned()
    } else {
        format!("{attr}-hero")
    };
    // The name interns through [`library_attr`], so each distinct name is
    // built once and the per-render lookup key drops.
    library_attr(&prefix, part)
}

/// The Hero's heading look (user request 2026-11-24, revised 2026-11-25): the
/// queue now-playing block's bold mono face, so a Hero's title and the name
/// line under it read as one heading. Both keep their own case: the capitals
/// read too loud, so no `text-transform` applies.
const HERO_HEADING_STYLE: &str = "margin: 0; font-family: var(--font-mono); font-weight: 700; letter-spacing: 0.06em; color: var(--canvas-heading);";

/// The Hero title's size, in CSS pixels (user request 2026-11-24).
const HERO_TITLE_PX: f32 = 18.0;

/// The name line's size, in CSS pixels, while it names the series-level item:
/// a season, an album, a book (user request 2026-11-24).
const HERO_NAME_PX: f32 = 18.0;

/// The name line's size, in CSS pixels, while it names an episode
/// (user request 2026-11-24).
const HERO_EPISODE_NAME_PX: f32 = 16.0;

/// The pane's painted width in CSS pixels, provided by the Library panel: the
/// Heroes bake the backdrop at the next 200 px step above it.
#[derive(Debug, Clone, Copy)]
pub struct HeroPaneWidth(pub Signal<f32>);

/// The pane width a Hero assumes while no panel provides one.
pub const FALLBACK_PANE_WIDTH: f32 = 1200.0;

/// The Library Hero's frame:
/// the backdrop behind the header band, the detail block with the artwork and the
/// picked item's text, and the destination's bands under their sections.
#[component]
pub fn LibraryHeroFrame(props: LibraryHeroFrameProps) -> Element {
    let LibraryHeroFrameProps {
        attr,
        root,
        header,
        header_action,
        block,
        art_shape,
        art,
        art_title,
        art_placeholder,
        actions,
        overview,
        cast,
        list_title,
        children,
        backdrop,
    } = props;

    let root_attrs = vec![Attribute::new(root, "", None, false)];
    let title_attrs = vec![Attribute::new(own(attr, "title"), "", None, false)];
    let overview_attrs = vec![Attribute::new(own(attr, "overview"), "", None, false)];
    // The name line's size (user request 2026-11-24): an episode's name paints
    // one step under a series-level item's.
    let name_px = if block.name_episode {
        HERO_EPISODE_NAME_PX
    } else {
        HERO_NAME_PX
    };

    rsx! {
        div {
            class: "relative",
            ..root_attrs,
            // The blurred-artwork backdrop (design D4): one opaque image the core
            // baked, pinned to the top of the scrolling content at the pane's
            // width, so it scrolls away with the Hero. It paints nothing while
            // the core bakes it.
            if let Some(backdrop) = backdrop.filter(|backdrop| backdrop.src().is_some()) {
                div {
                    class: "absolute",
                    style: "left: 0; top: 0; width: 100%; height: {BACKDROP_HEIGHT}px; overflow: hidden;",
                    ..vec![Attribute::new(own(attr, "backdrop"), "", None, false)],
                    ArtImage {
                        art: Some(backdrop),
                        alt: String::new(),
                        uncapped: true,
                        class: "block".to_owned(),
                        style: format!("width: 100%; height: {BACKDROP_HEIGHT}px; object-fit: cover;"),
                    }
                }
            }
            div {
                class: "relative flex flex-col",
                style: "padding: 28px 32px; gap: 28px;",
            div {
                class: "relative flex items-end justify-between",
                style: "gap: 24px; padding-bottom: 8px;",
                div {
                    class: "relative flex min-w-0 flex-col",
                    style: "gap: 6px;",
                    h1 {
                        style: format!("{HERO_HEADING_STYLE} font-size: {HERO_TITLE_PX}px;"),
                        ..title_attrs,
                        "{header.title}"
                    }
                    if !header.pills.is_empty() {
                        LibraryHeroPills { attr, hook: "pills", pills: header.pills.clone() }
                    }
                }
                if let Some(action) = header_action {
                    div {
                        // The action's own stacking context (as the detail
                        // block's actions row): the backdrop layer is absolute, so
                        // a bare static action would paint and hit-test under
                        // it (review 2026-11-26).
                        class: "relative flex-none",
                        { action }
                    }
                }
            }
            LibraryHeroDetail {
                attr,
                block_attr: own(attr, "block"),
                label: block.label.clone(),
                art_shape,
                art,
                art_title,
                art_placeholder,
                if painted(block.eyebrow.as_ref())
                    || named(block.name.as_deref())
                    || !block.pills.is_empty()
                    || !block.rows.is_empty()
                {
                    div {
                        class: "flex flex-col",
                        style: "gap: 6px;",
                        if let Some(eyebrow) = block.eyebrow.as_ref().filter(|eyebrow| !eyebrow.is_empty()) {
                            span {
                                class: "font-mono",
                                style: "font-size: 12px; letter-spacing: 0.08em; color: var(--canvas-muted);",
                                TypedText { segments: eyebrow.clone() }
                            }
                        }
                        if let Some(name) = block.name.as_ref().filter(|name| !name.is_empty()) {
                            h2 {
                                style: format!(
                                    "{HERO_HEADING_STYLE} font-size: {name_px}px;"
                                ),
                                ..vec![Attribute::new(
                                    own(attr, "block-name"),
                                    "",
                                    None,
                                    false,
                                )],
                                "{name}"
                            }
                        }
                        if !block.pills.is_empty() {
                            LibraryHeroPills { attr, hook: "block-pills", pills: block.pills.clone() }
                        }
                        if !block.rows.is_empty() {
                            div {
                                style: "margin-top: 8px;",
                                LibraryHeroFactRows { attr, hook: "rows", rows: block.rows.clone() }
                            }
                        }
                    }
                }
                if let Some(actions) = actions {
                    div {
                        class: "relative flex flex-wrap items-center",
                        style: "gap: 8px;",
                        { actions }
                    }
                }
            }
            if let Some(overview) = overview.as_ref().filter(|overview| !overview.is_empty()) {
                LibraryHeroSection {
                    attr,
                    hook: "overview",
                    title: "Overview".to_owned(),
                    p {
                        class: "min-w-0",
                        style: "margin: 0; font-size: 14px; line-height: 1.5; color: var(--canvas-muted);",
                        ..overview_attrs,
                        "{overview}"
                    }
                }
            }
            if !cast.is_empty() {
                LibraryHeroSection {
                    attr,
                    hook: "cast",
                    title: "Cast & Crew".to_owned(),
                    LibraryHeroCast { attr, hook: "cast-grid", members: cast.clone() }
                }
            }
            if let Some(children) = children {
                if let Some(title) = list_title.as_ref().filter(|title| !title.is_empty()) {
                    LibraryHeroSection {
                        attr,
                        hook: "list",
                        title: title.clone(),
                        { children }
                    }
                } else {
                    { children }
                }
            }
            }
        }
    }
}

#[cfg(test)]
mod tests;
