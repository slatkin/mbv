//! The GUI Client's Client-side Owner link: find, start, and attach to this
//! machine's Owner process through the library entry (design D9 of
//! `add-gui-front-end`). The Owner is started from the `mbv` program
//! (`owner_exe`): resolved from this executable's directory, then from
//! `PATH`, and always recorded on the `App` so an Owner restart never starts
//! this GUI program. The refusal texts are the TUI Client's, byte for byte:
//! the shared Owner-link ones from the library, shown here in the window
//! instead of on a terminal.

use mbv::{App, OwnerStartOptions, OwnerState, load_config};

use crate::owner_exe;

/// Find, start, and attach to the Owner process, and hand back the `App` the
/// Client core drives. `Err` is the notice the window shows while no session
/// exists — the same texts the TUI Client exits with.
pub(crate) fn attach() -> Result<App, String> {
    let owner_exe = owner_exe::resolve()?;
    let config = load_config().map_err(|error| format!("mbv: {error}"))?;
    match mbv::find_owner()
        .map_err(|error| format!("mbv: single-instance check failed: {error}"))?
    {
        OwnerState::Start => {
            mbv::start_owner(&OwnerStartOptions {
                owner_exe: Some(&owner_exe),
                log_level: None,
            })
            .map_err(|error| format!("mbv: failed to start Owner process: {error}"))?;
        }
        OwnerState::Attach => {}
        OwnerState::Refuse => return Err(mbv::refusal_text()),
    }
    mbv::attach_owner(&config, Some(&owner_exe)).map_err(|error| {
        if error.is_exclusive_owner() {
            mbv::exclusive_owner_refusal_text(&error)
        } else {
            format!("mbv: {error}")
        }
    })
}
