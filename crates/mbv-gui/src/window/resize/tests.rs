//! Tests of the two dividers (D16): the contracts of `resize.test.ts` — the
//! defaults, the clamps, the reset, the one resolution the paint and the
//! thresholds share, the saved widths, and the UI-reset epoch. The width
//! arithmetic is pure; the store contracts run over a headless document whose
//! root built the store, so no test opens a window or touches the real state
//! dir (the recording saver stands in for `mbv-config`'s file, whose own
//! contract `mbv-config` tests).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use mbv_config::GuiPanelWidths;
use rstest::rstest;

use super::{
    DraggedWidths, LIBRARY_LIST_DEFAULT_WIDTH_PX, LIBRARY_LIST_MIN_WIDTH_PX, PanelWidthsStore,
    WidthsSaver, clamp_list_width, clamp_queue_width, panel_widths,
};
use crate::bridge::frame::GuiPanelToggles;
use crate::window::arrangement::{GuiArrangement, library_arrangement};
use crate::window::layout::viewport_layout_size;

/// The core's toggles: the expanded panel with its queue painted.
fn expanded() -> GuiPanelToggles {
    GuiPanelToggles {
        collapsed: false,
        queue_folded: false,
        ui_reset_epoch: 0,
    }
}

/// The collapsed panel: the 72 px rail, whatever the dragged width is.
fn collapsed() -> GuiPanelToggles {
    GuiPanelToggles {
        collapsed: true,
        ..expanded()
    }
}

fn container(width: f32, height: f32) -> crate::bridge::layout::LayoutSize {
    crate::bridge::layout::LayoutSize { width, height }
}

/// The 1600 × 900 container the width contracts measure against.
fn measured() -> crate::bridge::layout::LayoutSize {
    container(1600.0, 900.0)
}

fn dragged(queue: Option<f32>, list: Option<f32>) -> DraggedWidths {
    DraggedWidths { queue, list }
}

/// Contract 1: both dividers start at their defaults — the canvas's 400 px
/// Queue column, the list column 15 % wider than the canvas's 320 px index,
/// and the divider between the columns taking its own width.
#[test]
fn both_dividers_start_at_their_defaults() {
    let widths = panel_widths(measured(), &expanded(), &dragged(None, None));

    assert_eq!(widths.queue, 400.0);
    // The request made the list column 15 % wider than the canvas's index.
    assert_eq!(
        (LIBRARY_LIST_MIN_WIDTH_PX * 1.15).round(),
        LIBRARY_LIST_DEFAULT_WIDTH_PX
    );
    assert_eq!(widths.list, LIBRARY_LIST_DEFAULT_WIDTH_PX);
    // The divider overlays the seam, so the columns meet directly.
    assert_eq!(widths.library_column, 1200.0);
}

/// Contract 2: the Queue column stays between its canvas width and the lesser
/// of its 800 px cap and the width that leaves the Library column its Mini
/// threshold — a drag removes no panel, and a drag inside the bounds holds.
#[test]
fn the_queue_column_stays_between_its_canvas_width_and_the_cap() {
    assert_eq!(clamp_queue_width(100.0, 1600.0), 400.0);
    assert_eq!(clamp_queue_width(1400.0, 1600.0), 800.0);

    let mut session = dragged(None, None);
    session.queue = Some(clamp_queue_width(500.0, 1600.0));
    assert_eq!(panel_widths(measured(), &expanded(), &session).queue, 500.0);
}

/// Contract 3: the list column stays between the canvas index and the room the
/// Hero pane leaves its minimum.
#[test]
fn the_list_column_stays_between_the_canvas_index_and_the_heros_minimum() {
    assert_eq!(clamp_list_width(100.0, 1200.0), LIBRARY_LIST_MIN_WIDTH_PX);
    // 1200 px less the Hero's 320 px minimum.
    assert_eq!(clamp_list_width(900.0, 1200.0), 880.0);

    let mut session = dragged(None, None);
    session.list = Some(clamp_list_width(500.0, 1200.0));
    assert_eq!(panel_widths(measured(), &expanded(), &session).list, 500.0);
}

/// Contract 4: the collapsed rail keeps its width and hands the rest to the
/// Library column.
#[test]
fn the_collapsed_rail_keeps_its_width_and_hands_the_rest_to_the_library_column() {
    let session = dragged(Some(500.0), None);

    let widths = panel_widths(measured(), &collapsed(), &session);

    assert_eq!(widths.queue, 72.0);
    assert_eq!(widths.library_column, 1528.0);
}

/// Contract 5: a stored width is clamped again when the window narrows, so a
/// width dragged on a wider window narrows with the window.
#[test]
fn a_stored_width_is_clamped_again_when_the_window_narrows() {
    let session = dragged(Some(clamp_queue_width(1100.0, 1600.0)), None);

    let widths = panel_widths(container(1200.0, 900.0), &expanded(), &session);

    assert_eq!(widths.queue, 720.0);
    assert_eq!(widths.library_column, 480.0);
}

/// Contract 6 (the no-flapping contract): the arrangement classifies the width
/// a drag paints — a drag that shrinks the Library column crosses the
/// threshold instead of leaving the paint and the classification apart.
#[test]
fn the_arrangement_classifies_the_width_a_drag_paints() {
    let session = dragged(Some(clamp_queue_width(1120.0, 1600.0)), None);
    let widths = panel_widths(measured(), &expanded(), &session);

    assert_eq!(
        library_arrangement(measured(), widths.queue),
        GuiArrangement::Narrow
    );
}

/// The recorded saves of a store's test sink.
type Recorded = Arc<Mutex<Vec<GuiPanelWidths>>>;

/// A saver that records every write, standing in for `mbv-config`'s file.
fn recording_sink(saves: &Recorded) -> WidthsSaver {
    let saves = Arc::clone(saves);
    Arc::new(move |widths: &GuiPanelWidths| {
        saves.lock().expect("the test sink lives").push(*widths);
    })
}

/// The handles a store test drives: the document keeps the store's signals
/// alive, the handle reaches the store, and the sink records every save.
struct Fixture {
    doc: DioxusDocument,
    store: PanelWidthsStore,
    saves: Recorded,
}

#[derive(Clone)]
struct StoreProps {
    saved: GuiPanelWidths,
    handle: Rc<RefCell<Option<PanelWidthsStore>>>,
    saves: Recorded,
}

fn store_app(props: StoreProps) -> Element {
    let StoreProps {
        saved,
        handle,
        saves,
    } = props;
    let store = use_hook(move || PanelWidthsStore::new(saved, recording_sink(&saves)));
    handle.borrow_mut().replace(store);
    rsx! {}
}

/// A document whose root built the store over the recording saver.
fn built_store(saved: GuiPanelWidths) -> Fixture {
    let recorded: Recorded = Arc::new(Mutex::new(Vec::new()));
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            store_app,
            StoreProps {
                saved,
                handle: Rc::clone(&handle),
                saves: Arc::clone(&recorded),
            },
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    let store = handle
        .borrow()
        .clone()
        .expect("the root handed the store over");
    Fixture {
        doc,
        store,
        saves: recorded,
    }
}

impl Fixture {
    /// The saves the store wrote, in order.
    fn saved(&self) -> Vec<GuiPanelWidths> {
        self.saves.lock().expect("the test sink lives").clone()
    }
}

/// Contract 7 (the sticky widths): a drag is saved — the sink records the
/// clamped width — and a relaunch reads it back.
#[test]
fn a_dragged_width_is_saved_and_read_back_on_the_next_start() {
    let mut fixture = built_store(GuiPanelWidths::default());
    let mut store = fixture.store.clone();
    let _doc = &mut fixture.doc;

    store.drag_list_width(420.0, 1400.0);

    assert_eq!(
        fixture.saved(),
        vec![GuiPanelWidths {
            queue: None,
            list: Some(420)
        }]
    );
    // The next start reads the saved widths back (D14).
    let mut next_start = built_store(GuiPanelWidths {
        queue: None,
        list: Some(420),
    });
    let _doc = &mut next_start.doc;
    assert_eq!(next_start.store.dragged().list, Some(420.0));
}

/// Contract 8: a double-click returns each divider to its default, and the
/// reset saves the defaults (D14).
#[test]
fn a_double_click_returns_each_divider_to_its_default() {
    let mut fixture = built_store(GuiPanelWidths::default());
    let mut store = fixture.store.clone();
    let _doc = &mut fixture.doc;

    store.drag_queue_width(500.0, 1600.0);
    store.drag_list_width(500.0, 1200.0);
    store.reset_queue_width();
    store.reset_list_width();

    let widths = panel_widths(measured(), &expanded(), &store.dragged());
    assert_eq!(widths.queue, 400.0);
    assert_eq!(widths.list, LIBRARY_LIST_DEFAULT_WIDTH_PX);
    assert_eq!(fixture.saved().last(), Some(&GuiPanelWidths::default()));
}

/// Contract 9 (the `gui-settings-screen` reset): a new `uiResetEpoch` — the
/// core's Reset UI State — resets both widths and writes the defaults, the
/// first value only seeds the epoch, and a frame carrying the same epoch
/// resets nothing.
#[test]
fn a_new_ui_reset_epoch_resets_both_widths() {
    let mut fixture = built_store(GuiPanelWidths::default());
    let mut store = fixture.store.clone();
    let _doc = &mut fixture.doc;

    assert!(
        !store.note_ui_reset_epoch(11),
        "the first value only seeds it"
    );
    store.drag_queue_width(500.0, 1600.0);
    store.drag_list_width(500.0, 1200.0);

    assert!(store.note_ui_reset_epoch(12));
    assert_eq!(store.dragged(), DraggedWidths::default());
    assert_eq!(fixture.saved().last(), Some(&GuiPanelWidths::default()));

    // A frame carrying the same epoch resets nothing.
    store.drag_queue_width(500.0, 1600.0);
    assert!(!store.note_ui_reset_epoch(12));
    assert_eq!(store.dragged().queue, Some(500.0));
}

/// Contract 10 (the live resize jerk of `e978b31aa`, D9): the size the launch
/// copy routes at a resize is the size the container measures — the physical
/// surface over its scale factor — so the arrangement and the painted widths
/// picked from it are the ones a measured container of that size gives. The
/// cases cross each threshold: the Mini maximum, the Wide minimum width, and
/// the Wide minimum height.
#[rstest]
#[case::mini(1600, 1200, 2.0, 800.0, 600.0, GuiArrangement::Mini)]
#[case::narrow_at_the_mini_maximum(1760, 1200, 2.0, 880.0, 600.0, GuiArrangement::Narrow)]
#[case::wide(2560, 1600, 2.0, 1280.0, 800.0, GuiArrangement::Wide)]
#[case::wide_height_one_under(2560, 1198, 2.0, 1280.0, 599.0, GuiArrangement::Narrow)]
#[case::wide_on_a_fractional_scale(2880, 1800, 1.5, 1920.0, 1200.0, GuiArrangement::Wide)]
fn the_size_a_resize_routes_is_the_size_the_container_measures(
    #[case] physical_width: u32,
    #[case] physical_height: u32,
    #[case] scale: f32,
    #[case] measured_width: f32,
    #[case] measured_height: f32,
    #[case] expected: GuiArrangement,
) {
    let routed = viewport_layout_size(&Viewport::new(
        physical_width,
        physical_height,
        scale,
        ColorScheme::Light,
    ));
    let measured = container(measured_width, measured_height);
    let toggles = expanded();

    // The routed size is the container's, to the pixel.
    assert_eq!(routed, measured);
    // One derivation: either source paints and classifies the same columns.
    assert_eq!(
        panel_widths(routed, &toggles, &dragged(None, None)),
        panel_widths(measured, &toggles, &dragged(None, None))
    );
    let widths = panel_widths(measured, &toggles, &dragged(None, None));
    assert_eq!(library_arrangement(measured, widths.queue), expected);
}
