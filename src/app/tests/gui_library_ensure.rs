//! The GUI Library destination driver's contract (gui-one-destination-ensure
//! design D1, D3, tasks 1.2 and 2.2): `App::run_gui_destination_loads` runs
//! one site's three load steps in the shared order — root, then selection,
//! then dependents — and stops at the first step that calls for a load. The
//! site here is a recording one, so the tests prove the order and the stop
//! with no Service, no client, and no fetch.

use std::cell::RefCell;

use super::*;
use crate::app::gui_library::ensure::{GuiDestinationLoads, GuiStepOutcome};

/// One step of a destination load pass, as the recording site saw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoadStep {
    Root,
    Selection,
    Dependents,
}

/// A site that records each step it runs, in call order, and reports that
/// `halt_at` called for a load.
#[derive(Debug)]
struct RecordingLoads<'a> {
    steps: &'a RefCell<Vec<LoadStep>>,
    halt_at: Option<LoadStep>,
}

impl RecordingLoads<'_> {
    /// Record `step`, and report the pass ended when it is the halt step.
    fn record(&self, step: LoadStep) -> GuiStepOutcome {
        self.steps.borrow_mut().push(step);
        if self.halt_at == Some(step) {
            GuiStepOutcome::LoadStarted
        } else {
            GuiStepOutcome::Continue
        }
    }
}

impl GuiDestinationLoads for RecordingLoads<'_> {
    fn ensure_root(&self, _app: &mut App, _gui: &mut GuiClientState) -> GuiStepOutcome {
        self.record(LoadStep::Root)
    }

    fn ensure_selection(&self, _app: &mut App, _gui: &mut GuiClientState) -> GuiStepOutcome {
        self.record(LoadStep::Selection)
    }

    fn ensure_dependents(&self, _app: &mut App, _gui: &mut GuiClientState) {
        self.record(LoadStep::Dependents);
    }
}

#[test]
fn driver_runs_root_then_selection_then_dependents() {
    let mut app = make_app_stub();
    let mut gui = GuiClientState::new(false);
    let steps = RefCell::new(Vec::new());
    crate::app::gui_library::ensure::run_gui_destination_loads(
        &mut app,
        &mut gui,
        &RecordingLoads {
            steps: &steps,
            halt_at: None,
        },
    );
    assert_eq!(
        steps.into_inner(),
        vec![LoadStep::Root, LoadStep::Selection, LoadStep::Dependents]
    );
}

/// A step that calls for a load ends the pass (design D3), so the driver never
/// starts a dependent load in the same pass as the load it depends on.
#[test]
fn driver_stops_at_the_step_that_calls_for_a_load() {
    let mut app = make_app_stub();
    let mut gui = GuiClientState::new(false);
    let steps = RefCell::new(Vec::new());
    crate::app::gui_library::ensure::run_gui_destination_loads(
        &mut app,
        &mut gui,
        &RecordingLoads {
            steps: &steps,
            halt_at: Some(LoadStep::Selection),
        },
    );
    assert_eq!(
        steps.into_inner(),
        vec![LoadStep::Root, LoadStep::Selection]
    );
}
