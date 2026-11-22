//! Library panel content types (design D3, task 5.1): the only paint input
//! of the Library panel. Plain data — no `App`, Service client, `Config`, or
//! protocol object, and no styling primitive (`HeroFacts` carries no
//! `Span`/`Style`/width; the header painter owns colours, truncation and
//! wrapping, design D5).
//!
//! The hero's plain artwork data — `ArtworkShape`, `ArtworkSource`,
//! `HeroArtwork`, `HeroFacts`, and `PanelHeroImagePaint` — lives in
//! `mbv_ui_model::library_hero` (task 1.7); this module re-exports it so the
//! panel's existing paths keep resolving until task 1.8 deletes the
//! re-exports.

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::inline_search::InlineSearch;
#[doc(inline)]
pub use mbv_ui_model::library_hero::{
    ArtworkShape, ArtworkSource, HeroArtwork, HeroFacts, HeroLink, PanelHeroImagePaint,
    painted_shape_for_pixels,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeroCredit {
    pub name: String,
    pub role: String,
}

/// The Wide Hero header (spec: exactly three types). Its arm is the artwork
/// policy's shape and [`HeroHeader::from`] is the only constructor, so no
/// destination can choose an arm (design D5); the painter dispatches on it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeroHeader {
    arm: HeroHeaderArm,
}

/// The closed arm set; private so only the policy can construct a header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeroHeaderArm {
    /// 16:9 artwork full content width above title/meta.
    Landscape,
    /// 2:3 artwork right of title/meta.
    Portrait,
    /// 1:1 artwork right of title/meta.
    Square,
}

impl HeroHeader {
    /// The one constructor (design D5): the arm is the policy's shape.
    #[must_use]
    pub fn from(shape: ArtworkShape) -> Self {
        Self {
            arm: match shape {
                ArtworkShape::Landscape => HeroHeaderArm::Landscape,
                ArtworkShape::Square => HeroHeaderArm::Square,
                ArtworkShape::Portrait => HeroHeaderArm::Portrait,
            },
        }
    }

    /// The arm, for the header painter's dispatch — not a constructor.
    #[must_use]
    pub fn arm(self) -> HeroHeaderArm {
        self.arm
    }
}

/// One Selector row (design D8): a single pill bar followed by the panel's
/// spacer row. The panel retains the bar's `HitRegions`; an empty pill list
/// paints no bar, but the row's place stays reserved (the Inline Search box
/// takes it while a search is active).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectorRow {
    pub pills: Vec<String>,
    /// Parallel semantic marker flags; an empty vector leaves every pill unmarked.
    pub markers: Vec<bool>,
    /// The selected pill's index; `None` paints no active pill.
    pub active: Option<usize>,
}

/// The list box's content for one frame (design D3).
pub enum ListSlot<'a> {
    /// The active canonical media-list presentation.
    Media(&'a mut dyn PanelList),
    /// A placeholder in the list box: the loading state plus plain text.
    Empty { loading: bool, text: String },
    /// An active Inline Search session. The panel places the search box in
    /// the Selector row's rect (reserved even with no `SelectorRow`) and the
    /// results in the list box; the session keeps its own state and painter.
    Search(&'a mut InlineSearch),
}

impl std::fmt::Debug for ListSlot<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Media(_) => f.write_str("Media(..)"),
            Self::Empty { loading, text } => f
                .debug_struct("Empty")
                .field("loading", loading)
                .field("text", text)
                .finish(),
            Self::Search(_) => f.write_str("Search(..)"),
        }
    }
}

/// The hero pane's Workspace (design D3): an optional header row and
/// selector row over one list. A hero with a Workspace is focusable;
/// `focused` selects the focused surfaces (design D6).
pub struct Workspace<'a> {
    /// One-row box title with the Hero separator line and one blank row
    /// below it, before the list. Omitted when the box has no room for a
    /// list row; the painter resolves the style from the semantic arm, never
    /// from the label text.
    pub header: Option<WorkspaceHeader>,
    pub selector: Option<SelectorRow>,
    pub list: &'a mut dyn PanelList,
    pub focused: bool,
}

impl std::fmt::Debug for Workspace<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workspace")
            .field("header", &self.header)
            .field("selector", &self.selector)
            .field("focused", &self.focused)
            .finish_non_exhaustive()
    }
}

/// One hero pane's content (design D3). Breakpoint-neutral: the Wide header
/// and the Library Hero overlay both derive from it.
#[derive(Debug)]
pub struct HeroContent<'a> {
    pub facts: HeroFacts,
    pub overview: Option<String>,
    /// The overview flow's title row (the focused TV Workspace's selected
    /// episode): painted yellow above the overview text, scrolling with it.
    pub overview_title: Option<String>,
    pub credits: Option<Vec<HeroCredit>>,
    pub workspace: Option<Workspace<'a>>,
}

/// The Library panel's only paint input for one frame (design D3). Plain
/// content plus `&mut` views into the active owner's media-list flow and
/// search session; no rect, surface, style, focus-kind, header-arm or
/// variant field.
#[derive(Debug)]
pub struct LibraryPanelContent<'a> {
    /// The Browser pane's primary browse selector, if the destination
    /// supplies one.
    pub selector: Option<SelectorRow>,
    /// The list box's content.
    pub list: ListSlot<'a>,
    /// The Hero pane's slot: whether the list carries media, and the hero.
    pub hero: HeroSlot<'a>,
}

/// Whether the Hero pane exists for the current list. The living requirement
/// is `library-panel` "A list of non-media rows has no Hero pane". The panel
/// reads "no Hero pane" from this type, never from the destination's key.
#[derive(Debug)]
pub enum HeroSlot<'a> {
    /// The list carries media. Wide paints the Hero pane, empty when the
    /// selection has no hero.
    ///
    /// The hero is boxed to keep the enum small; build the variant with
    /// [`HeroSlot::pane`].
    Pane(Option<Box<HeroContent<'a>>>),
    /// The list carries no media. Wide paints no Hero pane, the list takes the
    /// whole content area, and the overlay never opens.
    Absent,
}

impl<'a> HeroSlot<'a> {
    /// A Hero pane holding `hero`, empty when the selection has none.
    #[must_use]
    pub fn pane(hero: Option<HeroContent<'a>>) -> Self {
        Self::Pane(hero.map(Box::new))
    }

    /// Whether the Hero pane participates in the Wide layout.
    #[must_use]
    pub fn shows_pane(&self) -> bool {
        match self {
            Self::Pane(_) => true,
            Self::Absent => false,
        }
    }

    /// The hero content, if the slot holds any.
    #[must_use]
    pub fn content(&self) -> Option<&HeroContent<'a>> {
        match self {
            Self::Pane(hero) => hero.as_deref(),
            Self::Absent => None,
        }
    }

    /// The hero content for painting, if the slot holds any.
    pub fn content_mut(&mut self) -> Option<&mut HeroContent<'a>> {
        match self {
            Self::Pane(hero) => hero.as_deref_mut(),
            Self::Absent => None,
        }
    }
}

impl LibraryPanelContent<'_> {
    /// Whether the Hero pane's Workspace list holds focus (design D6): the
    /// one owner of this fact, so every consumer reads it instead of
    /// re-deriving it from `hero`/`workspace` at each call site.
    #[must_use]
    pub fn workspace_focused(&self) -> bool {
        self.hero
            .content()
            .and_then(|hero| hero.workspace.as_ref())
            .is_some_and(|workspace| workspace.focused)
    }
}

/// The closed paint policy the panel sets on its lists (design D3/D6): the
/// focus bit and selected-row surface are fixed by the owning presentation
/// policy; destinations pass none.
#[derive(Debug)]
pub enum PanelListPaintPolicy {
    /// The Wide browser presentation's policy. `focused` is the interaction
    /// bit: it drives the selected-row bar, the marquee, and the scrollbar.
    /// `palette_focused` is the appearance bit: it selects the zebra stripe
    /// tone. Mini view rests the palette (`palette_focused: false`) while the
    /// displayed panel keeps its cursor (`focused: true`), so the single
    /// panel's selected row stays visible against the resting stripe.
    /// Selected rows use the list backdrop surface.
    Wide {
        focused: bool,
        palette_focused: bool,
    },
    /// The library Workspace presentation — one unified look in both the
    /// Wide Hero pane and the Library Hero overlay: the selected row takes
    /// the sheet chrome Ink bar, the box rests at Slate, and the zebra
    /// stripes rest at the fixed resting-content Storm in both focus states.
    /// The focus bit is the Workspace's own: it drives the list's cursor
    /// emphasis, marquee, and scrollbar.
    WideWorkspace { focused: bool },
}

/// Object-safe view over one canonical media-list presentation flow
/// (design D3), implemented once by the shared media-list carrier for every
/// `Target` (task 5.8). The panel configures the fixed-row owner for its
/// breakpoint geometry, sets the paint policy, views it into the list slot's
/// rect, and reads retained selection geometry and point claims back.
/// Resolving a point to a typed target stays with the owning carrier's typed
/// surface — targets are erased here, so no per-destination `ListSlot` or
/// `Workspace` arm can grow.
pub trait PanelList {
    /// Clamp the viewport to the current geometry without transferring
    /// owner state; selection state remains with the owner.
    fn clamp_viewport(&mut self, viewport_height: usize);

    /// Configure the paint policy used by the next `view` (design D3/D6: the
    /// panel sets focus and the slot's fixed selected-row surface).
    fn set_paint_policy(&mut self, policy: PanelListPaintPolicy);

    /// View the active presentation into `rect`, retaining its paint
    /// geometry.
    fn view(&mut self, frame: &mut Frame, rect: Rect);

    /// Configure a split claim/content geometry for the next `view` (the
    /// canonical-list contract: the selected row's background reaches the
    /// full-width `claim_rect` while the row flow and hit geometry stay on
    /// the inset `content_rect`, matching the pre-migration wide rail). A
    /// destination that always paints one rect keeps the no-op default.
    fn set_geometry(&mut self, claim_rect: Rect, content_rect: Rect) {
        let _ = (claim_rect, content_rect);
    }

    /// The selected row's rect from the current view, when one is visible.
    fn selected_row_rect(&self) -> Option<Rect> {
        None
    }

    /// Returns the destination-owned search bar projection, when the list
    /// remains the browser owner during an in-place search session.
    fn search_bar(&self) -> Option<(String, bool)> {
        None
    }
}

/// The closed set of typed Workspace header rows. The Hero painter resolves
/// each arm's style here as a semantic role — never by matching the label
/// text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WorkspaceHeader {
    /// Grouped Music's track listing under an album Hero.
    Tracklist,
}

impl WorkspaceHeader {
    /// The row's painted label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Tracklist => "TRACKLIST",
        }
    }
}
