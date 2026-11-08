//! The startup restoration state `App` carries (launch-resolution's scoped
//! snapshot), split out of `app_struct.rs` along its own seam.

use mbv_config::TuiLaunchState;
use mbv_ui_model::tab_selection::TabSelection;

/// Startup restoration state, scoped to the tab selected by launch resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::app) enum LaunchRestore {
    Pending(TuiLaunchState),
    TabSettled {
        state: TuiLaunchState,
        tab: TabSelection,
    },
    Done,
}
