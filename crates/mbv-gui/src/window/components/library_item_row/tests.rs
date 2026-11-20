//! Headless tests of the top-level row part (gui-delineated-rows design D1,
//! D2): the thumb's four art states, the image fit its own shape asks for, the
//! media kind icon while the thumb holds no image, the row's progress along the
//! thumb's bottom edge, and a row that carries a title alone. The document
//! paints the part; the harness supplies the art painter's contexts and the
//! utilities the tree sizes its boxes with. No test opens a window, sleeps, or
//! reaches the core.

use blitz_dom::{Document as _, LocalName};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use rstest::rstest;

use super::{
    LIBRARY_ROW_HEIGHT, LibraryItemRow, LibraryRowThumb, LibraryTrailPart, THUMB_SQUARE_PX,
};
use crate::art::GuiArtRef;
use crate::bridge::frame::library::{GuiRowArtFrame, GuiRowMediaKindFrame};
use crate::window::clock::{Clock, FiredTimers, RecordingClock};
use crate::window::components::art_image::{ArtEvents, ScaleFactor};

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

/// The thumb's box hook and its progress hook, under the row's `test` prefix.
const THUMB: &str = "[data-library-test-thumb]";
const THUMB_PROGRESS: &str = "[data-library-test-thumb-progress]";

/// One row the fixture paints.
#[derive(Clone, PartialEq)]
struct RowSpec {
    art: GuiRowArtFrame,
    media: GuiRowMediaKindFrame,
    progress: Option<f32>,
    subtitle: Option<String>,
    trail: Option<String>,
}

impl Default for RowSpec {
    fn default() -> Self {
        Self {
            art: GuiRowArtFrame::NoSource,
            media: GuiRowMediaKindFrame::Video,
            progress: None,
            subtitle: None,
            trail: None,
        }
    }
}

/// The root: provides the art painter's contexts and paints one top-level row.
fn row_app(spec: RowSpec) -> Element {
    let RowSpec {
        art,
        media,
        progress,
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
        LibraryItemRow {
            attr: "test",
            kind: "item".to_owned(),
            id: "row-1".to_owned(),
            height: LIBRARY_ROW_HEIGHT,
            title: "Sabrosito".to_owned(),
            subtitle,
            trail,
            trail_part: LibraryTrailPart::Date,
            title_color: "var(--canvas-text)".to_owned(),
            background: None,
            selected: false,
            progress,
            thumb: Some(LibraryRowThumb { art, media }),
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
}

/// Paint one row and let its first render land.
fn built(spec: RowSpec) -> Fixture {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(row_app, spec),
        DocumentConfig {
            viewport: Some(Viewport::new(1200, 900, 1.0, ColorScheme::Light)),
            ua_stylesheets: vec![
                blitz_dom::UserAgentStylesheet::default_sheet(),
                blitz_dom::UserAgentStylesheet::parse(FIXTURE_CSS),
            ],
            ..DocumentConfig::default()
        },
    );
    doc.initial_build();
    let mut fixture = Fixture { doc };
    fixture.poll_until_idle();
    fixture
}

impl Fixture {
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

/// The cached row image the image-state cases paint.
fn cached_art() -> GuiRowArtFrame {
    GuiRowArtFrame::Image {
        art: GuiArtRef::Image {
            src: "mbv-art://localhost/card/abc".to_owned(),
        },
        square: false,
    }
}

/// Contract (design D1): the thumb names its art state, so a destination's
/// reader tells an image from a fetch in flight, a source with no image, and a
/// kind with no art source.
#[rstest]
#[case(cached_art(), "image")]
#[case(GuiRowArtFrame::Loading, "loading")]
#[case(GuiRowArtFrame::Missing, "missing")]
#[case(GuiRowArtFrame::NoSource, "none")]
fn the_thumb_names_its_art_state(#[case] art: GuiRowArtFrame, #[case] state: &str) {
    let fixture = built(RowSpec {
        art,
        ..RowSpec::default()
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
    let fixture = built(RowSpec {
        art: cached_art(),
        ..RowSpec::default()
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
    let fixture = built(RowSpec {
        art: GuiRowArtFrame::Image {
            art: GuiArtRef::Image {
                src: "mbv-art://localhost/card/abc".to_owned(),
            },
            square: true,
        },
        ..RowSpec::default()
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
    let fixture = built(RowSpec {
        art,
        media,
        ..RowSpec::default()
    });

    assert!(
        fixture.presents(&format!("{THUMB} {icon}")),
        "the {media:?} thumb paints {icon}"
    );
}

/// Contract (design D2, `library-row-artwork` spec "The thumb carries the
/// row's progress"): the row's percent paints along the thumb's bottom edge and
/// nowhere else.
#[test]
fn the_rows_progress_paints_along_the_thumbs_bottom_edge() {
    let fixture = built(RowSpec {
        progress: Some(40.0),
        ..RowSpec::default()
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
        !fixture.presents("[data-library-test-progress]"),
        "the row paints no progress mark outside its thumb"
    );
}

/// Contract (design D2): a row with no progress shows no progress mark.
#[test]
fn a_row_with_no_progress_paints_no_mark() {
    let fixture = built(RowSpec::default());

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
    let fixture = built(RowSpec::default());

    assert_eq!(
        fixture
            .attr_of("[data-library-test-row-id]", "data-library-test-row-id")
            .as_deref(),
        Some("row-1")
    );
    assert_eq!(fixture.text_of("[data-library-test-title]"), "Sabrosito");
    assert!(
        fixture
            .style_of("[data-library-test-row]")
            .contains(&format!("height: {LIBRARY_ROW_HEIGHT}px")),
        "the row keeps the top-level height"
    );
    assert_eq!(
        fixture.attr_of(THUMB, "data-library-test-thumb").as_deref(),
        Some("none"),
        "the thumb paints in the no-source state"
    );
    assert!(!fixture.presents("[data-library-test-subtitle]"));
    assert!(!fixture.presents("[data-library-test-date]"));
}
