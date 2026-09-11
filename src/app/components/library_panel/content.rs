//! Library panel content types (design D3, task 5.1): the only paint input
//! of the Library panel. Plain data — no `App`, Service client, `Config`, or
//! protocol object, and no styling primitive (`HeroFacts` carries no
//! `Span`/`Style`/width; the header painter owns colours, truncation and
//! wrapping, design D5).

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::app::components::inline_search::InlineSearch;

/// The artwork shape the header reserves a box for (spec: the Wide Hero
/// header's three types). Chosen by the artwork policy (design D5, task
/// 5.4); never by a destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum ArtworkShape {
    /// 16:9 artwork spanning the pane's content width above title/meta.
    Landscape,
    /// 1:1 artwork right of title/meta.
    Square,
    /// 2:3 artwork right of title/meta.
    Portrait,
}

/// Where a header's artwork comes from (design D3/D5). Built only by the
/// artwork policy (task 5.4); plain data — an image-type chain and a cache
/// key — so the shell projection (task 5.10) can fetch at projection time
/// while the painter reads projected state only. The Audiobookshelf arm
/// carries the Service-scoped library item id the projection resolves to its
/// server-scoped cache key (`App::audiobookshelf_{,book_}cover_key`); its
/// image type is the fixed `Primary` cover chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) enum ArtworkSource {
    /// An Emby image: fetch walks `image_types` (priority order, e.g. `Thumb`
    /// then `Primary`) under `cache_key`, with `series_id` for the fetch's
    /// series-fallback input.
    Emby {
        item_id: String,
        series_id: String,
        image_types: Vec<String>,
        cache_key: String,
    },
    /// An Audiobookshelf cover (podcast episode/show `:cover:`, book
    /// `:bookcover:`).
    AudiobookshelfCover { library_item_id: String, book: bool },
}

/// Declared artwork for one hero (design D3/D5). `source: None` means no
/// artwork source exists for the item (Feeds entries, an item with no
/// declared image): the header renders the shared placeholder at the policy
/// shape's box size.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) struct HeroArtwork {
    pub shape: ArtworkShape,
    pub source: Option<ArtworkSource>,
}

/// Plain hero facts (design D3): one title, ordered plain-text meta rows
/// (coloured by position by the panel, never styled by the destination),
/// and the policy-built artwork.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) struct HeroFacts {
    pub title: String,
    pub meta_rows: Vec<String>,
    pub artwork: HeroArtwork,
}

/// The Wide Hero header (spec: exactly three types). Its arm is the artwork
/// policy's shape and [`HeroHeader::from`] is the only constructor, so no
/// destination can choose an arm (design D5); the painter dispatches on it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) struct HeroHeader {
    arm: HeroHeaderArm,
}

/// The closed arm set; private so only the policy can construct a header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app) enum HeroHeaderArm {
    /// 16:9 artwork full content width above title/meta.
    Landscape,
    /// 2:3 artwork right of title/meta.
    Portrait,
    /// 1:1 artwork right of title/meta.
    Square,
}

impl HeroHeader {
    /// The one constructor (design D5): the arm is the policy's shape.
    pub(in crate::app) fn from(shape: ArtworkShape) -> Self {
        Self {
            arm: match shape {
                ArtworkShape::Landscape => HeroHeaderArm::Landscape,
                ArtworkShape::Square => HeroHeaderArm::Square,
                ArtworkShape::Portrait => HeroHeaderArm::Portrait,
            },
        }
    }

    /// The arm, for the header painter's dispatch — not a constructor.
    pub(in crate::app) fn arm(self) -> HeroHeaderArm {
        self.arm
    }
}

/// One Selector row (design D8): a single pill bar followed by the panel's
/// spacer row. The panel retains the bar's `HitRegions`; an empty pill list
/// paints no bar, but the row's place stays reserved (the Inline Search box
/// takes it while a search is active).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) struct SelectorRow {
    pub pills: Vec<String>,
    /// The selected pill's index; `None` paints no active pill.
    pub active: Option<usize>,
}

/// One List controls row (design D8): optional secondary pills plus an
/// optional plain-text label (the Feeds Watched filter, the home-video item
/// count). No per-destination arm; absent content renders the row absent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) struct ListControls {
    /// Secondary pill bar: labels plus the active pill's index.
    pub pills: Option<(Vec<String>, usize)>,
    /// Plain-text label, right-aligned in the row.
    pub label: Option<String>,
}

/// The list box's content for one frame (design D3).
pub(in crate::app) enum ListSlot<'a> {
    /// The active canonical media-list presentation.
    Media(&'a mut dyn PanelList),
    /// A placeholder in the list box: the loading state plus plain text.
    Empty { loading: bool, text: String },
    /// An active Inline Search session. The panel places the search box in
    /// the Selector row's rect (reserved even with no `SelectorRow`) and the
    /// results in the list box; the session keeps its own state and painter.
    Search(&'a mut InlineSearch),
}

/// The hero pane's Workspace (design D3): an optional selector row over one
/// list. A hero with a Workspace is focusable; `focused` selects the
/// focused surfaces (design D6).
pub(in crate::app) struct Workspace<'a> {
    pub selector: Option<SelectorRow>,
    pub list: &'a mut dyn PanelList,
    pub focused: bool,
}

/// One hero pane's content (design D3). Breakpoint-neutral: the Wide header
/// and the Narrow inline hero both derive from it.
pub(in crate::app) struct HeroContent<'a> {
    pub facts: HeroFacts,
    pub overview: Option<String>,
    pub workspace: Option<Workspace<'a>>,
}

/// The Library panel's only paint input for one frame (design D3). Plain
/// content plus `&mut` views into the active owner's media-list flow and
/// search session; no rect, surface, style, focus-kind, header-arm or
/// variant field.
pub(in crate::app) struct LibraryPanelContent<'a> {
    /// The Browser pane's primary browse selector, if the destination
    /// supplies one.
    pub selector: Option<SelectorRow>,
    /// The optional List controls row.
    pub controls: Option<ListControls>,
    /// The list box's content.
    pub list: ListSlot<'a>,
    /// The Hero pane's content, if the destination shows one.
    pub hero: Option<HeroContent<'a>>,
}

/// Object-safe view over one canonical media-list presentation flow
/// (design D3).
///
/// Provisional surface for this unit (tasks 5.1–5.3): the skeleton views the
/// active presentation into the list slot rect it computed. Task 5.8
/// formalizes the full D3 surface — `set_presentation(Wide | Inline, anchor)`,
/// the paint policy, retained slot geometry — and implements the trait once
/// for the shared media-list carrier.
pub(in crate::app) trait PanelList {
    /// View the active presentation into `rect`, retaining its paint
    /// geometry.
    fn view(&mut self, frame: &mut Frame, rect: Rect);

    /// View the active presentation with the panel's paint policy: the
    /// focus bit and the slot's fixed selected-row surface (design D3/D6).
    /// Provisional surface for this unit (task 5.6): the Workspace passes
    /// the Wide policy vocabulary until task 5.8 formalizes the full D3
    /// trait; the default keeps `view`'s behaviour.
    fn view_with_policy(
        &mut self,
        frame: &mut Frame,
        rect: Rect,
        policy: crate::app::components::media_list::WideMediaListPaintPolicy,
    ) {
        let _ = policy;
        self.view(frame, rect);
    }
}
