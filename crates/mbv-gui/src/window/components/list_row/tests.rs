//! Headless tests of the row parts (`gui-unified-list-rows` design D1–D4): the
//! top-level row's thumb, the thumb's four art states, the image fit its own
//! shape asks for, the media kind icon while the thumb holds no image, the
//! row's resume run along the thumb's bottom edge, and a row that carries a
//! title alone; and the nested row's level look (its height, inset, face, and
//! open-state hint) and its title tone over its play state. The document paints
//! the part; the harness supplies the art painter's contexts and the utilities
//! the tree sizes its boxes with. No test opens a window, sleeps, or reaches the
//! core.

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use rstest::rstest;

use super::{
    FlatRow, LIBRARY_ROW_HEIGHT, LibraryRowThumb, LibraryTrailPart, NODE_ROW_HEIGHT,
    THUMB_SQUARE_PX, TRACK_ROW_HEIGHT, TreeDepth, TreeLevel, TreeRow,
};
use crate::art::GuiArtRef;
use crate::bridge::frame::library::{GuiHomeRowStateFrame, GuiRowArtFrame, GuiRowMediaKindFrame};
use crate::window::clock::{Clock, FiredTimers, RecordingClock};
use crate::window::components::art_image::{ArtEvents, ScaleFactor};
use crate::window::components::tree_guide::{LEAF_INSET_X, NESTED_INSET_X};
use crate::window::components::tree_row::{LEAF_ROW_FONT_PX, TREE_ROW_FONT_PX};

/// The layout rules the fixture resolves with, in place of the app
/// stylesheet a real window loads: the utilities the row's tree sizes its
/// boxes with.
const FIXTURE_CSS: &str = concat!(
    ".flex { display: flex; }",
    ".flex-col { flex-direction: column; }",
    ".items-center { align-items: center; }",
    ".justify-center { justify-content: center; }",
    ".min-w-0 { min-width: 0; }",
    ".flex-1 { flex: 1 1 0%; }",
    ".flex-none { flex: none; }",
    ".w-full { width: 100%; }",
    ".h-full { height: 100%; }",
    ".relative { position: relative; }",
    ".absolute { position: absolute; }",
    ".overflow-hidden { overflow: hidden; }",
    ".object-cover { object-fit: cover; }",
    ".truncate { width: 100%; }",
);

/// The parts the fixture's rows paint, under the row's `test` prefix.
const ROW: &str = "[data-library-test-row]";
const ROW_ID: &str = "[data-library-test-row-id]";
const TITLE: &str = "[data-library-test-title]";
const SUBTITLE: &str = "[data-library-test-subtitle]";
const DATE: &str = "[data-library-test-date]";
const THUMB: &str = "[data-library-test-thumb]";
const THUMB_PROGRESS: &str = "[data-library-test-thumb-progress]";

/// The progress mark no part paints outside the thumb.
const ROW_PROGRESS: &str = "[data-library-test-progress]";

/// The shared hint a nested parent's row paints, and the mono label a leaf row
/// wraps its title in.
const TREE_HINT: &str = "[data-tree-hint]";
const MONO_LABEL: &str = "[style*=\"font-family: var(--font-mono)\"]";

/// One top-level row the fixture paints.
#[derive(Clone, PartialEq)]
struct FlatSpec {
    art: GuiRowArtFrame,
    media: GuiRowMediaKindFrame,
    play: Option<GuiHomeRowStateFrame>,
    subtitle: Option<String>,
    trail: Option<String>,
}

impl Default for FlatSpec {
    fn default() -> Self {
        Self {
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
            play: None,
            subtitle: None,
            trail: None,
        }
    }
}

/// One nested row the fixture paints.
#[derive(Clone, PartialEq)]
struct TreeSpec {
    level: TreeLevel,
    depth: TreeDepth,
    play: Option<GuiHomeRowStateFrame>,
    open: Option<bool>,
}

/// The root: provides the art painter's contexts and paints one top-level row.
fn flat_app(spec: FlatSpec) -> Element {
    let FlatSpec {
        art,
        media,
        play,
        subtitle,
        trail,
    } = spec;
    let events = use_hook(ArtEvents::new);
    let fired = use_hook(FiredTimers::new);
    provide_context(events);
    provide_context(fired);
    provide_context(ScaleFactor(Signal::new(1.0)));
    let clock: std::sync::Arc<dyn Clock> = std::sync::Arc::new(RecordingClock::new());
    provide_context(clock);
    rsx! {
        FlatRow {
            attr: "test",
            kind: "item".to_owned(),
            id: "row-1".to_owned(),
            title: "Sabrosito".to_owned(),
            subtitle,
            trail,
            trail_part: LibraryTrailPart::Date,
            thumb: Some(LibraryRowThumb { art, media }),
            play,
            selected: false,
            on_press: Callback::new(|()| {}),
            on_play: Callback::new(|()| {}),
        }
    }
}

/// The root: paints one nested row.
fn tree_app(spec: TreeSpec) -> Element {
    let TreeSpec {
        level,
        depth,
        play,
        open,
    } = spec;
    rsx! {
        TreeRow {
            attr: "test",
            kind: "item".to_owned(),
            id: "row-1".to_owned(),
            level,
            depth,
            title: "Sabrosito".to_owned(),
            play,
            selected: false,
            open,
            on_press: Callback::new(|()| {}),
            on_play: Callback::new(|()| {}),
        }
    }
}

struct Fixture {
    doc: DioxusDocument,
}

impl Fixture {
    /// Whether the selector names a node in the painted tree.
    fn presents(&self, selector: &str) -> bool {
        matches!(self.doc.inner().query_selector(selector), Ok(Some(_)))
    }

    /// The attribute of the first node the selector names.
    fn attr_of(&self, selector: &str, attribute: &str) -> Option<String> {
        let dom = self.doc.inner();
        let id = dom.query_selector(selector).ok().flatten()?;
        dom.get_node(id)
            .expect("the node is in the document")
            .attr(LocalName::from(attribute))
            .map(str::to_owned)
    }

    /// The `style` attribute of the first node the selector names.
    fn style_of(&self, selector: &str) -> String {
        self.attr_of(selector, "style")
            .unwrap_or_else(|| panic!("{selector} carries a style"))
    }

    /// The `class` attribute of the first node the selector names.
    fn class_of(&self, selector: &str) -> String {
        self.attr_of(selector, "class")
            .unwrap_or_else(|| panic!("{selector} carries a class"))
    }

    /// The trimmed text of the first node the selector names.
    fn text_of(&self, selector: &str) -> String {
        let dom = self.doc.inner();
        let id = dom
            .query_selector(selector)
            .expect("the selector is valid")
            .unwrap_or_else(|| panic!("{selector} paints"));
        dom.get_node(id)
            .expect("the node is in the document")
            .text_content()
            .trim()
            .to_owned()
    }

    /// The open-state hint's text, `None` while the row paints no hint.
    fn hint(&self) -> Option<String> {
        let dom = self.doc.inner();
        let id = dom.query_selector(TREE_HINT).ok().flatten()?;
        Some(
            dom.get_node(id)
                .expect("the node is in the document")
                .text_content(),
        )
    }

    /// Poll the document until its renders and effects have landed (a fixed
    /// poll count with a tripwire: never an unbounded wait).
    fn poll_until_idle(&mut self) {
        for _ in 0..8 {
            if !self.doc.poll(None) {
                return;
            }
        }
        panic!("document did not become idle");
    }
}

/// The document config the fixture's rows resolve under.
fn config() -> DocumentConfig {
    DocumentConfig {
        viewport: Some(Viewport::new(1200, 900, 1.0, ColorScheme::Light)),
        ua_stylesheets: vec![
            blitz_dom::UserAgentStylesheet::default_sheet(),
            blitz_dom::UserAgentStylesheet::parse(FIXTURE_CSS),
        ],
        ..DocumentConfig::default()
    }
}

/// Paint one top-level row and let its first render land.
fn flat_built(spec: FlatSpec) -> Fixture {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(|spec: FlatSpec| flat_app(spec), spec),
        config(),
    );
    doc.initial_build();
    let mut fixture = Fixture { doc };
    fixture.poll_until_idle();
    fixture
}

/// Paint one nested row and let its first render land.
fn tree_built(spec: TreeSpec) -> Fixture {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(|spec: TreeSpec| tree_app(spec), spec),
        config(),
    );
    doc.initial_build();
    let mut fixture = Fixture { doc };
    fixture.poll_until_idle();
    fixture
}

/// The cached row image the image-state cases paint.
fn cached_art() -> GuiRowArtFrame {
    GuiRowArtFrame::Image {
        art: GuiArtRef::Image {
            src: "mbv-art://localhost/card/abc".to_owned(),
        },
        square: false,
    }
}

/// Contract (design D2): the thumb names its art state, so a destination's
/// reader tells an image from a fetch in flight, a source with no image, and a
/// kind with no art source.
#[rstest]
#[case(cached_art(), "image")]
#[case(GuiRowArtFrame::Loading, "loading")]
#[case(GuiRowArtFrame::Missing, "missing")]
#[case(GuiRowArtFrame::NoSource, "none")]
fn the_thumb_names_its_art_state(#[case] art: GuiRowArtFrame, #[case] state: &str) {
    let fixture = flat_built(FlatSpec {
        art,
        ..FlatSpec::default()
    });

    assert_eq!(
        fixture.attr_of(THUMB, "data-library-test-thumb").as_deref(),
        Some(state)
    );
}

/// Contract (design D2, `library-row-artwork` spec "The thumb fits the image
/// by its shape"): a landscape or portrait image cover-fills the box.
#[test]
fn a_landscape_image_fills_the_thumb() {
    let fixture = flat_built(FlatSpec {
        art: cached_art(),
        ..FlatSpec::default()
    });

    let class = fixture.class_of(&format!("{THUMB} img"));
    assert!(
        class.contains("h-full") && class.contains("w-full") && class.contains("object-cover"),
        "the image fills and crops into the box: {class}"
    );
}

/// Contract (design D2, `library-row-artwork` spec "The thumb fits the image
/// by its shape"): a square image shows whole, centred at the box's height.
#[test]
fn a_square_image_shows_whole_in_the_thumb() {
    let fixture = flat_built(FlatSpec {
        art: GuiRowArtFrame::Image {
            art: GuiArtRef::Image {
                src: "mbv-art://localhost/card/abc".to_owned(),
            },
            square: true,
        },
        ..FlatSpec::default()
    });

    assert_eq!(
        fixture.style_of(&format!("{THUMB} img")),
        format!("width: {THUMB_SQUARE_PX}px; height: {THUMB_SQUARE_PX}px;"),
        "the square image paints at the box's height"
    );
    assert!(
        !fixture.class_of(&format!("{THUMB} img")).contains("h-full"),
        "the square image takes no box width"
    );
}

/// Contract (design D2): a thumb with no image shows the media kind's icon in
/// the muted tone.
#[rstest]
#[case(
    GuiRowArtFrame::Missing,
    GuiRowMediaKindFrame::Audio,
    "svg.lucide-eighth-notes"
)]
#[case(
    GuiRowArtFrame::NoSource,
    GuiRowMediaKindFrame::Video,
    "svg.lucide-monitor-play"
)]
fn the_thumb_shows_the_media_kind_with_no_image(
    #[case] art: GuiRowArtFrame,
    #[case] media: GuiRowMediaKindFrame,
    #[case] icon: &str,
) {
    let fixture = flat_built(FlatSpec {
        art,
        media,
        ..FlatSpec::default()
    });

    assert!(
        fixture.presents(&format!("{THUMB} {icon}")),
        "the {media:?} thumb paints {icon}"
    );
}

/// Contract (design D2, D4, `library-row-artwork` spec "The thumb carries the
/// row's progress"): the row's resume percent paints along the thumb's bottom
/// edge and nowhere else.
#[test]
fn the_rows_progress_paints_along_the_thumbs_bottom_edge() {
    let fixture = flat_built(FlatSpec {
        play: Some(GuiHomeRowStateFrame::Active { progress: Some(40) }),
        ..FlatSpec::default()
    });

    let style = fixture.style_of(THUMB_PROGRESS);
    assert!(
        style.contains("width: 40%") && style.contains("background: var(--canvas-partial)"),
        "the fill holds the row's percent: {style}"
    );
    assert!(
        fixture.presents(&format!("{THUMB} {THUMB_PROGRESS}")),
        "the fill paints inside the thumb"
    );
    assert!(
        !fixture.presents(ROW_PROGRESS),
        "the row paints no progress mark outside its thumb"
    );
}

/// Contract (design D2, D4): a row that carries no play state shows no
/// progress mark.
#[test]
fn a_row_with_no_progress_paints_no_mark() {
    let fixture = flat_built(FlatSpec::default());

    assert!(
        !fixture.presents(THUMB_PROGRESS),
        "the thumb paints no fill"
    );
}

/// Contract (`gui-client` spec "A row with little metadata is complete"): a row
/// with a title alone paints its thumb in the no-source state, its title, and
/// the top-level height, and no empty text line or trailing place.
#[test]
fn a_title_only_row_is_complete() {
    let fixture = flat_built(FlatSpec::default());

    assert_eq!(
        fixture
            .attr_of(ROW_ID, "data-library-test-row-id")
            .as_deref(),
        Some("row-1")
    );
    assert_eq!(fixture.text_of(TITLE), "Sabrosito");
    assert!(
        fixture
            .style_of(ROW)
            .contains(&format!("height: {LIBRARY_ROW_HEIGHT}px")),
        "the row keeps the top-level height"
    );
    assert_eq!(
        fixture.attr_of(THUMB, "data-library-test-thumb").as_deref(),
        Some("none"),
        "the thumb paints in the no-source state"
    );
    assert!(!fixture.presents(SUBTITLE));
    assert!(!fixture.presents(DATE));
}

/// Contract (design D3): the nested row's level picks its look and its depth
/// picks its inset — a parent paints the album look (the node height, the
/// nested inset, the sans face, and the open-state hint), and a leaf paints the
/// track look (the track height, the leaf inset, and the mono label), opening
/// nothing.
#[rstest]
#[case(
    TreeLevel::Parent,
    TreeDepth::One,
    NODE_ROW_HEIGHT,
    NESTED_INSET_X,
    TREE_ROW_FONT_PX,
    false,
    Some("Double-click to expand")
)]
#[case(
    TreeLevel::Leaf,
    TreeDepth::Two,
    TRACK_ROW_HEIGHT,
    LEAF_INSET_X,
    LEAF_ROW_FONT_PX,
    true,
    None
)]
fn a_nested_rows_level_and_depth_pick_its_look(
    #[case] level: TreeLevel,
    #[case] depth: TreeDepth,
    #[case] height: f32,
    #[case] inset: f32,
    #[case] font_px: f32,
    #[case] mono: bool,
    #[case] hint: Option<&str>,
) {
    let fixture = tree_built(TreeSpec {
        level,
        depth,
        play: None,
        open: hint.map(|_| false),
    });

    let style = fixture.style_of(ROW);
    assert!(
        style.contains(&format!("height: {height}px;")),
        "the {level:?} row keeps its level's height: {style}"
    );
    assert!(
        style.contains(&format!("padding-left: {inset}px")),
        "the {depth:?} row insets at its depth: {style}"
    );
    assert!(
        style.contains(&format!("font-size: {font_px}px")),
        "the {level:?} row paints its face's size: {style}"
    );
    assert_eq!(
        fixture.presents(MONO_LABEL),
        mono,
        "the leaf wraps its title in the mono label"
    );
    assert_eq!(fixture.hint().as_deref(), hint);
}

/// Contract (design D4): a nested row's title tone follows its play state — no
/// play state paints the level's own tone, an unplayed row the text tone, and a
/// played row the muted tone.
#[rstest]
#[case(None, "var(--canvas-muted)")]
#[case(Some(GuiHomeRowStateFrame::Ordinary), "var(--canvas-text)")]
#[case(Some(GuiHomeRowStateFrame::Played), "var(--canvas-muted)")]
fn a_nested_rows_title_paints_its_play_tone(
    #[case] play: Option<GuiHomeRowStateFrame>,
    #[case] tone: &str,
) {
    let fixture = tree_built(TreeSpec {
        level: TreeLevel::Leaf,
        depth: TreeDepth::Two,
        play,
        open: None,
    });

    assert!(
        fixture.style_of(TITLE).contains(&format!("color: {tone}")),
        "the title paints {tone}"
    );
}
