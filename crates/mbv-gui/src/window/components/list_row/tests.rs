//! Headless tests of the row parts (`gui-unified-list-rows` design D1–D4): the
//! top-level row's thumb, the thumb's four art states, the image fit its own
//! shape asks for, the media kind icon while the thumb holds no image, the
//! row's resume run along the thumb's bottom edge, and a row that carries a
//! title alone; and the nested row's level look (its height, inset, face, and
//! open state) and its title tone over its play state. The document paints
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
    THUMB_RADIUS_PX, THUMB_SQUARE_PX, THUMB_WIDTH_PX, TRACK_ROW_HEIGHT, TreeDepth, TreeLevel,
    TreeRow, TreeTrail,
};
use crate::art::GuiArtRef;
use crate::bridge::frame::library::{GuiHomeRowStateFrame, GuiRowArtFrame, GuiRowMediaKindFrame};
use crate::window::clock::{Clock, FiredTimers, RecordingClock};
use crate::window::components::art_image::ArtEvents;
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

/// The mono label a leaf row wraps its title in.
const MONO_LABEL: &str = "[style*=\"font-family: var(--font-mono)\"]";

/// One top-level row the fixture paints.
#[derive(Clone, PartialEq)]
struct FlatSpec {
    art: GuiRowArtFrame,
    media: GuiRowMediaKindFrame,
    play: Option<GuiHomeRowStateFrame>,
    subtitle: Option<String>,
    trail: Option<String>,
    hover_play: bool,
}

impl Default for FlatSpec {
    fn default() -> Self {
        Self {
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
            play: None,
            subtitle: None,
            trail: None,
            hover_play: false,
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

/// The contexts the top-level row's art thumb and clocks read.
fn provide_row_contexts() {
    let events = use_hook(ArtEvents::new);
    let fired = use_hook(FiredTimers::new);
    provide_context(events);
    provide_context(fired);
    let clock: std::sync::Arc<dyn Clock> = std::sync::Arc::new(RecordingClock::new());
    provide_context(clock);
}

/// The root: provides the art painter's contexts and paints one top-level row.
fn flat_app(spec: FlatSpec) -> Element {
    let FlatSpec {
        art,
        media,
        play,
        subtitle,
        trail,
        hover_play,
    } = spec;
    provide_row_contexts();
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
            on_hover_play: hover_play.then_some(Callback::new(|()| {})),
        }
    }
}

/// The root: paints one selected top-level row that offers every hover
/// control and is a tree parent.
fn flat_hover_app() -> Element {
    provide_row_contexts();
    rsx! {
        FlatRow {
            attr: "test",
            kind: "item".to_owned(),
            id: "row-1".to_owned(),
            title: "Sabrosito".to_owned(),
            subtitle: None,
            trail: Some("2026-11-24".to_owned()),
            trail_part: LibraryTrailPart::Date,
            selected: true,
            tree_open: Some(false),
            on_press: Callback::new(|()| {}),
            on_play: Callback::new(|()| {}),
            on_toggle: Some(Callback::new(|()| {})),
            on_hover_play: Some(Callback::new(|()| {})),
            on_hover_queue: Some(Callback::new(|()| {})),
            on_hover_shuffle: Some(Callback::new(|()| {})),
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

/// The root: paints one nested parent row that offers every hover control and
/// carries a trailing date.
fn tree_hover_app() -> Element {
    rsx! {
        TreeRow {
            attr: "test",
            kind: "item".to_owned(),
            id: "row-1".to_owned(),
            level: TreeLevel::Parent,
            depth: TreeDepth::One,
            title: "Sabrosito".to_owned(),
            trail: Some(TreeTrail::Date("1986".to_owned())),
            selected: false,
            open: Some(false),
            on_press: Callback::new(|()| {}),
            on_play: Callback::new(|()| {}),
            on_toggle: Some(Callback::new(|()| {})),
            on_hover_play: Some(Callback::new(|()| {})),
            on_hover_queue: Some(Callback::new(|()| {})),
            on_hover_shuffle: Some(Callback::new(|()| {})),
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

/// Paint one row that offers the hover controls and let its first render land.
fn flat_hover_built() -> Fixture {
    let mut doc = DioxusDocument::new(VirtualDom::new(flat_hover_app), config());
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

/// Paint one nested parent row that offers the hover controls and let its first
/// render land.
fn tree_hover_built() -> Fixture {
    let mut doc = DioxusDocument::new(VirtualDom::new(tree_hover_app), config());
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
/// by its shape"): a square or portrait image shows whole, centred at the box's height, rounded, on no fill.
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
        format!(
            "height: {THUMB_SQUARE_PX}px; width: auto; max-width: {THUMB_WIDTH_PX}px; border-radius: {THUMB_RADIUS_PX}px;"
        ),
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

/// Contract (gui-row-hover-actions design D1, D2): the hover controls are the
/// host's later sibling — never a child of the row button — the host carries
/// the row's fill as `--row-fill` for the button and the controls to read, the
/// trailing text rests as the trail the stylesheet hides, and the Play control
/// is named after its row.
#[test]
fn the_hover_controls_are_the_hosts_sibling_and_the_play_is_named_after_the_row() {
    let fixture = flat_hover_built();

    assert!(
        fixture.presents(".row-hover-host [data-library-test-row]"),
        "the row button sits in the hover host"
    );
    assert!(
        !fixture.presents("[data-library-test-row] [data-row-hover-actions]"),
        "no control nests inside the row button"
    );
    assert!(
        fixture.presents(".row-hover-host [data-row-hover-actions]"),
        "the controls paint in the host"
    );
    assert!(
        fixture.style_of(".row-hover-host").contains("--row-fill:"),
        "the host carries the row's fill"
    );
    assert!(
        fixture
            .style_of(ROW)
            .contains("background: var(--row-fill)"),
        "the row button paints the host's fill"
    );
    assert!(
        fixture.class_of(DATE).contains("row-hover-trail"),
        "the trailing text rests as the trail"
    );
    assert_eq!(
        fixture
            .attr_of("[data-row-action=\"play\"]", "aria-label")
            .as_deref(),
        Some("Play Sabrosito"),
        "the Play control is named after its row"
    );
    assert_eq!(
        fixture
            .attr_of("[data-row-action=\"toggle\"]", "aria-label")
            .as_deref(),
        Some("Expand"),
        "a closed tree parent offers Expand"
    );
}

/// Contract (gui-row-hover-actions design D1, D2): a nested parent row is a
/// hover host like the top-level row — its fill moves to `--row-fill` for the
/// row button and the controls, the trailing text rests as the trail the
/// stylesheet hides, and the controls are the row's later sibling, never its
/// child.
#[test]
fn a_nested_row_is_a_hover_host_with_the_controls_beside_it() {
    let fixture = tree_hover_built();

    assert!(
        fixture.presents(".row-hover-host [data-library-test-row]"),
        "the row button sits in the hover host"
    );
    assert!(
        !fixture.presents("[data-library-test-row] [data-row-hover-actions]"),
        "no control nests inside the row button"
    );
    assert!(
        fixture.presents(".row-hover-host [data-row-hover-actions]"),
        "the controls paint in the host"
    );
    assert!(
        fixture.style_of(".row-hover-host").contains("--row-fill:"),
        "the host carries the row's fill"
    );
    assert!(
        fixture
            .style_of(ROW)
            .contains("background: var(--row-fill)"),
        "the row button paints the host's fill"
    );
    assert!(
        fixture.class_of(DATE).contains("row-hover-trail"),
        "the trailing text rests as the trail"
    );
}

/// Contract (design D2): a row the selection does not mark still names a fill
/// for the hover controls' cover — the column it rests on — so the cover is
/// never transparent and the title it overlays never shows through.
#[test]
fn an_unselected_rows_cover_paints_the_column_it_rests_on() {
    let fixture = flat_built(FlatSpec::default());

    assert!(
        fixture
            .style_of(".row-hover-host")
            .contains("--row-fill: var(--canvas-library-column)"),
        "the host names the column's fill"
    );
}

/// Contract (design D3): the nested row's level picks its look and its depth
/// picks its inset — a parent paints the album look (the node height, the
/// nested inset, the sans face, and its open state), and a leaf paints the
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
    true
)]
#[case(
    TreeLevel::Leaf,
    TreeDepth::Two,
    TRACK_ROW_HEIGHT,
    LEAF_INSET_X,
    LEAF_ROW_FONT_PX,
    true,
    false
)]
fn a_nested_rows_level_and_depth_pick_its_look(
    #[case] level: TreeLevel,
    #[case] depth: TreeDepth,
    #[case] height: f32,
    #[case] inset: f32,
    #[case] font_px: f32,
    #[case] mono: bool,
    #[case] parent: bool,
) {
    let fixture = tree_built(TreeSpec {
        level,
        depth,
        play: None,
        open: parent.then_some(false),
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
}

/// The title weight assertion of the play-tone table: a row that carries a
/// play state paints the weight its case names on the `Truncate` box, and a
/// row that carries none keeps the leaf label's weight and writes no
/// `font-weight` of its own.
fn assert_title_weight(fixture: &Fixture, weight: Option<&str>) {
    let style = fixture.style_of(TITLE);
    match weight {
        Some(weight) => assert!(style.contains(weight), "the title paints {weight}: {style}"),
        None => assert!(
            !style.contains("font-weight"),
            "the title writes no weight of its own: {style}"
        ),
    }
}

/// Contract (design D4; gui-played-row-marks design D1, D3, D4): a nested
/// row's title tone and weight follow its play state, and its play mark names
/// the state — no play state paints the level's own tone and weight and no
/// mark, an unplayed row the `ef` text tone at 600 with the `unplayed` mark,
/// and a played row the `ef` grey tone at 400 with the `played` mark.
#[rstest]
#[case(None, "var(--canvas-muted)", None, None)]
#[case(
    Some(GuiHomeRowStateFrame::Ordinary),
    "var(--ef-fg)",
    Some("font-weight: 600"),
    Some("unplayed")
)]
#[case(
    Some(GuiHomeRowStateFrame::Played),
    "var(--ef-grey1)",
    Some("font-weight: 400"),
    Some("played")
)]
fn a_nested_rows_title_paints_its_play_tone(
    #[case] play: Option<GuiHomeRowStateFrame>,
    #[case] tone: &str,
    #[case] weight: Option<&str>,
    #[case] mark: Option<&str>,
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
    assert_title_weight(&fixture, weight);
    assert_eq!(
        fixture
            .attr_of("[data-play-mark]", "data-play-mark")
            .as_deref(),
        mark,
        "the slot carries the mark value"
    );
}

/// Contract (gui-played-row-marks design D1–D3, `gui-client` spec "A played
/// row" and "The hover controls cover the trailing end"): a flat row that
/// carries a play state paints its mark in the one trailing cluster the trail
/// text shares — no state paints no mark, an unplayed row the `unplayed` mark,
/// a played row the `played` mark with its thumb dimmed to the played opacity.
#[rstest]
#[case(None, None, false)]
#[case(Some(GuiHomeRowStateFrame::Ordinary), Some("unplayed"), false)]
#[case(Some(GuiHomeRowStateFrame::Played), Some("played"), true)]
fn a_flat_rows_mark_shares_the_trailing_cluster_with_the_trail_text(
    #[case] play: Option<GuiHomeRowStateFrame>,
    #[case] mark: Option<&str>,
    #[case] dimmed: bool,
) {
    let fixture = flat_built(FlatSpec {
        play,
        trail: Some("2026-11-24".to_owned()),
        hover_play: true,
        ..FlatSpec::default()
    });

    assert_eq!(
        fixture
            .attr_of("[data-play-mark]", "data-play-mark")
            .as_deref(),
        mark,
        "the slot carries the mark value"
    );
    let thumb_style = fixture.style_of(THUMB);
    assert_eq!(
        thumb_style.contains("opacity: 0.35"),
        dimmed,
        "the thumb box dims only on the played row: {thumb_style}"
    );
    let shared = fixture.presents(".row-hover-trail [data-play-mark]")
        && fixture.presents(".row-hover-trail [data-library-test-date]");
    assert_eq!(
        shared,
        mark.is_some(),
        "the mark and the trail text share the one row-hover-trail cluster"
    );
}
