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
    /// Projected image state (task 5.10, design D9): set only by the shell's
    /// projection; the painters read it and never fetch. Producers build the
    /// default (`None`) and the owner copies the projection's state in.
    pub image: HeroImageState,
}

impl HeroArtwork {
    /// The shape of the artwork that will actually paint: the declared
    /// policy shape until the projection has decoded the fetched image,
    /// then the decoded source's own pixel-aspect class — wider than 5:4
    /// stays Landscape, near-square (between 4:5 and 5:4) is Square, and
    /// anything taller is Portrait. A declared landscape image the provider
    /// cannot serve (the fetch chain falls through to the `Primary` poster)
    /// therefore re-arms the side-by-side layout instead of painting a
    /// portrait poster — uncropped in the overlay, cover-cropped in Wide —
    /// inside a 16:9 box; genuinely 16:9 artwork keeps the Landscape arm.
    pub(in crate::app) fn painted_shape(&self) -> ArtworkShape {
        let decoded = match &self.image {
            HeroImageState::Ready { decoded, .. } => *decoded,
            _ => None,
        };
        let Some((w, h)) = decoded else {
            return self.shape;
        };
        if w * 4 >= h * 5 {
            ArtworkShape::Landscape
        } else if w * 5 > h * 4 {
            ArtworkShape::Square
        } else {
            ArtworkShape::Portrait
        }
    }
}

/// The projection's image state for one hero (task 5.10, design D9): the
/// fetch/encode runs in the shell projection; painting reads this state only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) enum HeroImageState {
    /// No image will arrive (no artwork source, images disabled, or a fetch
    /// that resolved empty): the shared placeholder is final.
    None,
    /// A fetch/encode for the artwork source is in flight: the painters
    /// reserve the box with the shared placeholder.
    Loading,
    /// The image is cached under `cache_key` (cover-fit keyed by the Wide
    /// header's projected box); the painters reserve the box and the shell
    /// paints the protocol into it after view, showing the placeholder at
    /// most one frame while an encode completes. `decoded` is the cached
    /// cached source image's pixel size for aspect-aware artwork placement.
    Ready {
        cache_key: String,
        decoded: Option<(u32, u32)>,
    },
}

/// Plain hero facts (design D3): one title, ordered plain-text meta rows
/// (coloured by position by the panel, never styled by the destination),
/// and the policy-built artwork.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) struct HeroFacts {
    pub title: String,
    pub meta_rows: Vec<String>,
    pub links: Vec<HeroLink>,
    pub artwork: HeroArtwork,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) struct HeroLink {
    pub name: String,
    pub url: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) struct HeroCredit {
    pub name: String,
    pub role: String,
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

/// One List controls row (design D8): a plain-text label, such as the
/// home-video item count. Selectable pills belong only in the Selector row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) struct ListControls {
    /// Plain-text label, right-aligned in the row.
    pub label: String,
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

/// The hero pane's Workspace (design D3): an optional header row and
/// selector row over one list. A hero with a Workspace is focusable;
/// `focused` selects the focused surfaces (design D6).
pub(in crate::app) struct Workspace<'a> {
    /// One-row box title, painted in the foam metadata role with the Hero
    /// separator line and one blank row below it, before the list (Grouped
    /// Music's `Tracks`). Omitted when the box has no room for a list row.
    pub header: Option<&'static str>,
    pub selector: Option<SelectorRow>,
    pub list: &'a mut dyn PanelList,
    pub focused: bool,
}

/// One hero pane's content (design D3). Breakpoint-neutral: the Wide header
/// and the Library Hero overlay both derive from it.
pub(in crate::app) struct HeroContent<'a> {
    pub facts: HeroFacts,
    pub overview: Option<String>,
    pub credits: Option<Vec<HeroCredit>>,
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

impl<'a> LibraryPanelContent<'a> {
    /// Whether the Hero pane's Workspace list holds focus (design D6): the
    /// one owner of this fact, so every consumer reads it instead of
    /// re-deriving it from `hero`/`workspace` at each call site.
    pub(in crate::app) fn workspace_focused(&self) -> bool {
        self.hero
            .as_ref()
            .and_then(|hero| hero.workspace.as_ref())
            .is_some_and(|workspace| workspace.focused)
    }
}

/// The closed paint policy the panel sets on its lists (design D3/D6): the
/// focus bit and selected-row surface are fixed by the owning presentation
/// policy; destinations pass none.
pub(in crate::app) enum PanelListPaintPolicy {
    /// The Wide browser presentation's policy: focus only. Selected rows use
    /// the list backdrop surface.
    Wide { focused: bool },
    /// The Wide library Workspace presentation. Its selected row belongs to
    /// the owning library pane surface rather than the list backdrop.
    WideWorkspace { focused: bool },
    /// The non-Wide library list. It owns the surface it sits on — the same
    /// identity its zebra stripe resolves from — so the painter fills its own
    /// body and resolves the scrollbar column through it. Wide keeps
    /// `Wide`/`WideWorkspace`: its pane body is painted by the skeleton, not by
    /// the list.
    Narrow { focused: bool },
}

/// Object-safe view over one canonical media-list presentation flow
/// (design D3), implemented once by the shared media-list carrier for every
/// `Target` (task 5.8). The panel configures the fixed-row owner for its
/// breakpoint geometry, sets the paint policy, views it into the list slot's
/// rect, and reads retained selection geometry and point claims back.
/// Resolving a point to a typed target stays with the owning carrier's typed
/// surface — targets are erased here, so no per-destination `ListSlot` or
/// `Workspace` arm can grow.
pub(in crate::app) trait PanelList {
    /// Clamp the viewport to the current geometry without transferring
    /// owner state; selection state remains with the owner.
    fn sync_viewport(&mut self, viewport_height: usize);

    /// Clear interaction selection when this owner is replaced as the active
    /// destination. Overlay focus changes do not call this method.
    fn clear_selection(&mut self) {}

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

    /// Whether the current view's retained geometry claims `point` (D6 frame
    /// invalidation: a presentation that has not completed its view claims
    /// nothing).
    fn claims_point(&self, point: ratatui::layout::Position) -> bool {
        let _ = point;
        false
    }
}

/// The hero image paint the panel retained from one view (task 5.10, design
/// D9): the projected protocol's cache key and the box rect it paints into.
/// The shell paints it right after `view` returns — the same
/// defer-the-pixel-paint seam every destination component already uses
/// (`take_image_paint`); the painters themselves only read projected state
/// and never fetch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app) struct PanelHeroImagePaint {
    /// The artwork box the painter reserved for the projected image.
    pub area: Rect,
    /// The projected cache key (`HeroImageState::Ready`'s key).
    pub cache_key: String,
    /// `true` for a centered artwork box; `false` for right-aligned placement.
    pub centered: bool,
}
