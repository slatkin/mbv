//! The Library Hero's frame (`replace-tauri-with-blitz` task 7.5, the port of
//! `LibraryHeroFrame`; add-gui-library-panel design D5, D8, D15): the
//! one layout Music, Shows, Podcasts, Audiobooks, and Feeds paint, as the TUI
//! paints them through one `HeroContentData` and one Workspace. Three bands,
//! top to bottom:
//!
//!   1. the header — the parent's kicker, title, and facts (Music's artist,
//!      Shows' series), with an optional action on its right;
//!   2. the detail block — the artwork box and, beside it, the picked item's
//!      eyebrow, name, facts, action row, and overview (Music's album, Shows'
//!      selected season with the series' overview). A Hero of one selected
//!      row (Podcasts, Audiobooks, Feeds) paints its kicker and title here,
//!      as Shows does for an episode, and keeps its meta rows in the block;
//!   3. the list — the destination's rows (Music's tracklist, Shows' season
//!      pills over the episodes), passed as the children.
//!
//! A destination supplies facts and rows and never restates the layout, so a
//! fix here lands in all of them. The detail block — the artwork box and the
//! text beside it — is `LibraryHeroDetail`'s, the one block the Home Hero paints
//! through too, so the art's size and place are the same in every destination.
//!
//! `attr` names the destination's own attributes: the Music Hero's are
//! `data-library-hero-*` (`attr` empty) and the Shows Hero's are
//! `data-library-shows-hero-*`, so a destination's tests keep their hooks.

use dioxus_native::prelude::*;

use crate::art::GuiArtRef;
use crate::bridge::frame::library::GuiArtShapeFrame;
use crate::window::components::library_attr::library_attr;
use crate::window::components::library_hero_detail::LibraryHeroDetail;
use crate::window::components::typed_text::TypedText;
use crate::window::library::hero::{HeroMetaRole, HeroMetaRow, Segments};

/// The props of the frame (the `LibraryHeroFrame` props, in order).
#[derive(Props, Clone, PartialEq)]
pub struct LibraryHeroFrameProps {
    /// The destination's attribute prefix: empty for Music, `shows` for
    /// Shows.
    pub attr: &'static str,
    /// The root's own attribute, the hook the panel's tests take.
    pub root: &'static str,
    /// The header band: always painted, so every Hero shares the one look.
    /// The kicker is a label, and the facts line carries each value's data
    /// type (design D14).
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
    /// The list band.
    #[props(default)]
    pub children: Option<Element>,
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
            .field("children", &self.children.is_some())
            .finish()
    }
}

/// The header band's lines.
#[derive(Debug, Clone, PartialEq)]
pub struct HeroHeader {
    /// The section label's line.
    pub kicker: Segments,
    /// The big title: the largest unit.
    pub title: String,
    /// The plain facts on one line.
    pub facts: Segments,
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
    /// The plain facts on one line.
    pub facts: Option<Segments>,
    /// The meta rows that carry a role, for a Hero whose facts are rows.
    pub meta: Vec<HeroMetaRow>,
    /// The item's overview.
    pub overview: Option<String>,
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

/// The header facts line's own attribute.
fn facts_attrs(attr: &'static str) -> Vec<Attribute> {
    vec![Attribute::new(own(attr, "facts"), "", None, false)]
}

/// The block facts line's own attribute.
fn block_facts_attrs(attr: &'static str) -> Vec<Attribute> {
    vec![Attribute::new(own(attr, "block-facts"), "", None, false)]
}

/// The frame's header from the one `hero_layout` built, so a destination that
/// maps its facts through [`hero_layout`](crate::window::library::hero::hero_layout)
/// hands the frame the layout's own lines.
impl From<crate::window::library::hero::HeroLayoutHeader> for HeroHeader {
    fn from(header: crate::window::library::hero::HeroLayoutHeader) -> Self {
        Self {
            kicker: header.kicker,
            title: header.title,
            facts: header.facts,
        }
    }
}

/// The frame's block from the one `hero_layout` built: the layout's block
/// carries no eyebrow and no plain facts line, which only the destinations
/// that build their block by hand supply.
impl From<crate::window::library::hero::HeroLayoutBlock> for HeroBlock {
    fn from(block: crate::window::library::hero::HeroLayoutBlock) -> Self {
        Self {
            label: block.label,
            eyebrow: None,
            name: block.name,
            facts: None,
            meta: block.meta,
            overview: block.overview,
        }
    }
}

/// One meta row's painted role: the class and colour the role paints, as the
/// the former client ternaries route them.
fn row_paint(role: &HeroMetaRow) -> (&'static str, &'static str) {
    match role.role {
        HeroMetaRole::Progress => ("font-mono tabular-nums", "var(--canvas-accent)"),
        HeroMetaRole::Duration => ("font-mono tabular-nums", "var(--canvas-muted)"),
        HeroMetaRole::Meta => ("font-mono", "var(--canvas-muted)"),
    }
}

/// The Library Hero's frame:
/// the header band, the detail block with the artwork and the picked item's
/// text, and the list band.
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
        children,
    } = props;

    let root_attrs = vec![Attribute::new(root, "", None, false)];
    let kicker_attrs = vec![Attribute::new(own(attr, "kicker"), "", None, false)];
    let title_attrs = vec![Attribute::new(own(attr, "title"), "", None, false)];
    let overview_attrs = vec![Attribute::new(own(attr, "overview"), "", None, false)];

    rsx! {
        div {
            class: "flex flex-col",
            style: "padding: 28px 32px; gap: 28px;",
            ..root_attrs,
            div {
                class: "flex items-end justify-between",
                style: "gap: 24px; padding-bottom: 20px; border-bottom: 1px solid var(--canvas-border);",
                div {
                    class: "flex min-w-0 flex-col",
                    style: "gap: 6px;",
                    span {
                        class: "font-mono",
                        style: "font-size: 12px; letter-spacing: 0.08em;",
                        ..kicker_attrs,
                        TypedText { segments: header.kicker.clone() }
                    }
                    h1 {
                        style: "margin: 0; font-size: 32px; font-weight: 600; letter-spacing: -0.01em; color: var(--canvas-heading);",
                        ..title_attrs,
                        "{header.title}"
                    }
                    if !header.facts.is_empty() {
                        span {
                            class: "font-mono",
                            style: "font-size: 13px; color: var(--canvas-muted);",
                            ..facts_attrs(attr),
                            TypedText { segments: header.facts.clone() }
                        }
                    }
                }
                if let Some(action) = header_action {
                    { action }
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
                    || painted(block.facts.as_ref())
                    || !block.meta.is_empty()
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
                                style: "margin: 0; font-size: 24px; font-weight: 600; color: var(--canvas-heading);",
                                ..vec![Attribute::new(
                                    own(attr, "block-name"),
                                    "",
                                    None,
                                    false,
                                )],
                                "{name}"
                            }
                        }
                        if let Some(facts) = block.facts.as_ref().filter(|facts| !facts.is_empty()) {
                            span {
                                class: "font-mono",
                                style: "font-size: 13px; color: var(--canvas-muted);",
                                ..block_facts_attrs(attr),
                                TypedText { segments: facts.clone() }
                            }
                        }
                        for (index, row) in block.meta.iter().enumerate() {
                            {
                                let (class, color) = row_paint(row);
                                let role_attrs = vec![Attribute::new(
                                    own(attr, "role"),
                                    row.role.wire(),
                                    None,
                                    false,
                                )];
                                rsx! {
                                    span {
                                        key: "{index}",
                                        class: "{class}",
                                        style: "font-size: 13px; color: {color};",
                                        ..role_attrs,
                                        TypedText { segments: row.segments.clone() }
                                    }
                                }
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
                if let Some(overview) = block.overview.as_ref().filter(|overview| !overview.is_empty()) {
                    p {
                        class: "min-w-0",
                        style: "margin: 0; font-size: 14px; line-height: 1.5; color: var(--canvas-muted);",
                        ..overview_attrs,
                        "{overview}"
                    }
                }
            }
            if let Some(children) = children {
                { children }
            }
        }
    }
}

#[cfg(test)]
mod tests;
