//! Headless tests of the window root's effects (D16): the contract the
//! review of task 4.1 pinned — the `uiResetEpoch` effect settles. Every
//! signal write marks its subscribers (an equal value included), so an
//! effect that reads a signal in its body and then writes it re-queues
//! itself forever, and the scheduler's task/effect drain never empties: the
//! live window's render loop would spin at 100 % CPU after the first frame.
//! No scheduler entry can bound that drain, so the host's effect body
//! carries a run-count tripwire: a self-queued run past the bound panics
//! out of the scheduler, and the test fails fast without hanging. The idle
//! assertion itself drives only `now_or_never(wait_for_work)` passes, which
//! never enter the render loop. The host runs the exact effect body the
//! root runs ([`note_panel_ui_reset`]). No test opens a window, sleeps, or
//! reaches the core.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use dioxus_native::prelude::*;
use futures_util::FutureExt;
use mbv_config::GuiPanelWidths;

use super::{LibraryPaint, batch_changes_painted_rows, note_panel_ui_reset};
use crate::bridge::frame::GuiPanelToggles;
use crate::bridge::frame::library::{
    GuiDestinationList, GuiEntryRowFrame, GuiFeedsDetail, GuiFeedsList, GuiHomeRowStateFrame,
    GuiLibraryDetail, GuiLibraryList,
};
use crate::window::resize::PanelWidthsStore;
use crate::window::sections::Sections;

/// The pass bound for the idle assertion: a settling effect goes `Pending`
/// within a few passes; a self-triggering one never does.
const PASS_BOUND: usize = 16;

/// The run bound the effect's tripwire enforces: a settling effect runs once
/// per fold; the ninth run in one phase means the effect re-queued itself.
const RUN_BOUND: usize = 8;

/// What the root hands the test: the section stores.
type FixtureHandles = Sections;

#[derive(Clone)]
struct FixtureProps {
    runs: Rc<Cell<usize>>,
    handle: Rc<std::cell::RefCell<Option<FixtureHandles>>>,
}

/// The effect's host: the same `uiResetEpoch` fold the root's effect runs,
/// over a width store whose saver records nothing.
fn reset_host(props: FixtureProps) -> Element {
    let sections = use_hook(Sections::new);
    provide_context(sections.clone());
    let widths = use_hook(|| PanelWidthsStore::new(GuiPanelWidths::default(), Arc::new(|_| {})));
    let mut store = widths;
    let host_sections = sections.clone();
    let runs = props.runs;
    use_effect(move || {
        let count = runs.get() + 1;
        runs.set(count);
        assert!(
            count <= RUN_BOUND,
            "the uiResetEpoch effect re-queued itself: run {count} of at most {RUN_BOUND}"
        );
        let _reset = note_panel_ui_reset(&mut store, &host_sections);
    });
    props.handle.borrow_mut().replace(sections);
    rsx! {
        div { "the reset effect's host" }
    }
}

struct Fixture {
    vdom: VirtualDom,
    sections: Sections,
    runs: Rc<Cell<usize>>,
}

impl Fixture {
    /// Whether the effect settled in this phase: the scheduler reached a
    /// pass with no dirty scope and no ready task, within the pass bound. A
    /// `Ready` pass runs at most one queued task or effect — never the
    /// render loop — so a self-triggering effect is caught by the tripwire
    /// in the host's effect body, not by a spin here.
    fn settles(&mut self) -> bool {
        self.runs.set(0);
        for _ in 0..PASS_BOUND {
            let work = self.vdom.wait_for_work();
            if std::pin::pin!(work).now_or_never().is_none() {
                return true;
            }
        }
        false
    }

    /// Write the `panel` section's toggles, as the drain task would.
    fn set_panel(&mut self, toggles: GuiPanelToggles) {
        let mut panel = self.sections.panel;
        self.vdom.in_runtime(move || panel.set(toggles));
    }
}

fn built() -> Fixture {
    let runs = Rc::new(Cell::new(0));
    let handle = Rc::new(std::cell::RefCell::new(None));
    let mut vdom = VirtualDom::new_with_props(
        reset_host,
        FixtureProps {
            runs: Rc::clone(&runs),
            handle: Rc::clone(&handle),
        },
    );
    vdom.rebuild_in_place();
    let sections = handle
        .borrow_mut()
        .take()
        .expect("the root handed the stores over");
    Fixture {
        vdom,
        sections,
        runs,
    }
}

/// Contract (review of task 4.1): the `uiResetEpoch` effect settles. The
/// first fold only seeds the store, and a new epoch — the core's Reset UI
/// State — folds again; neither the seed's write nor a reset's write may
/// re-queue the effect that read it, at mount or after a section change.
#[test]
fn the_ui_reset_effect_settles() {
    let mut fixture = built();
    assert!(fixture.settles(), "the effect did not settle at mount");
    fixture.set_panel(GuiPanelToggles {
        collapsed: false,
        queue_folded: false,
        ui_reset_epoch: 1,
    });
    assert!(fixture.settles(), "the effect did not settle after a reset");
}

/// One entry row titled `title`, as the Feeds list paints it.
fn entry_row(title: &str) -> GuiEntryRowFrame {
    GuiEntryRowFrame {
        id: title.to_owned(),
        title: title.to_owned(),
        subtitle: None,
        date: None,
        state: GuiHomeRowStateFrame::Ordinary,
    }
}

/// The Feeds list frame painting one row per title, in order.
fn feeds_list(titles: &[&str]) -> GuiLibraryList {
    GuiLibraryList {
        tabs: Vec::new(),
        current_tab: None,
        destination: GuiDestinationList::Feeds(GuiFeedsList {
            pills: Vec::new(),
            loading: false,
            failed: false,
            rows: titles.iter().map(|title| entry_row(title)).collect(),
        }),
    }
}

/// The Feeds detail frame selecting `selected`, with no Hero.
fn feeds_detail(selected: &str) -> GuiLibraryDetail {
    GuiLibraryDetail::Feeds(GuiFeedsDetail {
        selected: Some(selected.to_owned()),
        hero: None,
    })
}

/// Contract (D12 of `replace-tauri-with-blitz`): a batch that repaints a
/// Library row bumps the layout epoch, so the `Truncate` box the row mounts
/// re-measures the layout that holds it.
#[test]
fn a_batch_that_repaints_a_library_row_bumps_the_epoch() {
    let before_list = feeds_list(&["one"]);
    let after_list = feeds_list(&["one", "two"]);
    let detail = feeds_detail("one");

    assert!(batch_changes_painted_rows(
        LibraryPaint {
            list: Some(&before_list),
            detail: Some(&detail),
            search: None,
        },
        LibraryPaint {
            list: Some(&after_list),
            detail: Some(&detail),
            search: None,
        },
    ));
}

/// Contract (D12 of `replace-tauri-with-blitz`): a cursor move over rows
/// whose Hero it does not change repaints no row, so it bumps nothing.
#[test]
fn a_cursor_move_over_the_painted_rows_bumps_nothing() {
    let list = feeds_list(&["one", "two"]);
    let before_detail = feeds_detail("one");
    let after_detail = feeds_detail("two");

    assert!(!batch_changes_painted_rows(
        LibraryPaint {
            list: Some(&list),
            detail: Some(&before_detail),
            search: None,
        },
        LibraryPaint {
            list: Some(&list),
            detail: Some(&after_detail),
            search: None,
        },
    ));
}
