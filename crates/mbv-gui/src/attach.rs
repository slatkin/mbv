//! The GUI Client's Client-side Owner link, up to the attach (design D9 of
//! `add-gui-front-end`).
//!
//! Row 6.1 only needs a real `App` for the Client core: it attaches to an
//! Owner process that already accepts Clients and reports why it could not
//! when none does. Row 6.2 owns the rest of the link — resolving the `mbv`
//! program from the GUI executable's directory and then `PATH`, starting the
//! Owner from it, and presenting the same refusal text the TUI Client shows
//! for an exclusive Owner. It also moves this notice out of the window and
//! into the window's own refusal presentation.

use mbv::{App, Config, OwnerState, load_config};

/// Attach to the live Owner process and hand back the `App` the Client core
/// drives. The error is the notice the window shows while no session exists.
pub(crate) fn attach() -> Result<App, String> {
    let config = load_config().map_err(|error| format!("mbv: {error}"))?;
    let state = mbv::find_owner().map_err(|error| format!("mbv: {error}"))?;
    match state {
        // The Owner is never this program (design D9), so a Client that
        // starts one must name the `mbv` program: row 6.2 resolves it.
        OwnerState::Start => Err("mbv: no owner process is running.".to_owned()),
        OwnerState::Attach => attach_to(&config),
        OwnerState::Refuse => {
            Err("mbv: the local owner process is not accepting connections.".to_owned())
        }
    }
}

/// Attach alongside whatever Client the live Owner already serves.
///
/// `owner_exe` stays `None` this row because no path here starts an Owner:
/// the start and restart steps that read it belong to row 6.2, which also
/// resolves the `mbv` program they launch (design D9).
fn attach_to(config: &Config) -> Result<App, String> {
    mbv::attach_owner(config, None).map_err(|error| format!("mbv: {error}"))
}
