//! The GUI Library destination driver's contract (gui-one-destination-ensure
//! design D1, task 1.2): `App::run_gui_destination_loads` runs one site's
//! three load steps in the shared order — root, then selection, then
//! dependents. The site here is a recording one, so the test proves the order
//! with no Service, no client, and no fetch.

use std::cell::RefCell;

use super::*;
use crate::app::gui_library::ensure::GuiDestinationLoads;

/// One step of a destination load pass, as the recording site saw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoadStep {
    Root,
    Selection,
    Dependents,
}

/// A site that records each step it runs, in call order.
#[derive(Debug)]
struct RecordingLoads<'a> {
    steps: &'a RefCell<Vec<LoadStep>>,
}

impl GuiDestinationLoads for RecordingLoads<'_> {
    fn ensure_root(&self, _app: &mut App) {
        self.steps.borrow_mut().push(LoadStep::Root);
    }

    fn ensure_selection(&self, _app: &mut App) {
        self.steps.borrow_mut().push(LoadStep::Selection);
    }

    fn ensure_dependents(&self, _app: &mut App) {
        self.steps.borrow_mut().push(LoadStep::Dependents);
    }
}

#[test]
fn driver_runs_root_then_selection_then_dependents() {
    let mut app = make_app_stub();
    let steps = RefCell::new(Vec::new());
    app.run_gui_destination_loads(&RecordingLoads { steps: &steps });
    assert_eq!(
        steps.into_inner(),
        vec![LoadStep::Root, LoadStep::Selection, LoadStep::Dependents]
    );
}
