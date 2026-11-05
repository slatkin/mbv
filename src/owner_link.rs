//! The Client-side Owner link (design D2, D7, and D9 of `add-gui-front-end`):
//! every local Client — the TUI Client and the GUI Client — finds this
//! machine's Owner process, starts it when none runs, and attaches to it as a
//! Client.
//!
//! The entry points stay step-wise because each Client owns its own follow-up
//! policy after a failed attach: the TUI Client prompts on the terminal
//! (`startup::run_local_instance`), and the GUI Client reports in its window.
//! Only the Owner's side of the link lives here, so both Clients run the same
//! find, start, and attach code.
//!
//! A Client never holds the Owner lock and never becomes a Player owner. The
//! Owner the Client starts is always `mbv`: the TUI Client's own executable,
//! or the `mbv` program the GUI Client resolved ([`OwnerStartOptions`]).

use std::io;
use std::path::Path;

use crate::app::App;
use crate::config::Config;
use crate::local_daemon;
use crate::single_instance;
use mbv_emby::EmbyClient;
use mbv_remote_player::{DaemonEndpoint, RemotePlayer, RemotePlayerError};

/// What the local Owner process is doing, as a Client sees it: the step the
/// Client runs next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerState {
    /// No live Owner holds the lock. The Client starts one.
    Start,
    /// A live Owner accepts connections. The Client attaches alongside any
    /// Client already attached.
    Attach,
    /// A live Owner holds the lock but does not accept connections. The
    /// Client refuses, with [`refusal_text`].
    Refuse,
}

/// How [`start_owner`] launches the Owner process.
#[derive(Debug, Clone, Copy, Default)]
pub struct OwnerStartOptions<'a> {
    /// The `mbv` program that becomes the Owner. `None` starts this process's
    /// own executable — the TUI Client, which is `mbv` itself. The GUI Client
    /// names the `mbv` program it resolved, so the Owner is never the GUI
    /// program.
    pub owner_exe: Option<&'a Path>,
    /// The log level to forward to the Owner process.
    pub log_level: Option<&'a mbv_core::applog::LogSpec>,
}

/// Find this machine's Owner process. The probe's freshly acquired lock guard
/// is released here: a Client never holds the Owner lock, so the Owner that
/// [`start_owner`] launches re-acquires it as part of its own startup.
pub fn find_owner() -> io::Result<OwnerState> {
    let socket = single_instance::socket_path();
    let lock = mbv_config::owner_lock_path();
    Ok(match single_instance::resolve(&socket, &lock)? {
        single_instance::Resolution::Fresh(guard) => {
            drop(guard);
            OwnerState::Start
        }
        single_instance::Resolution::Attach => OwnerState::Attach,
        single_instance::Resolution::Refuse => OwnerState::Refuse,
    })
}

/// Start the Owner process from the `mbv` program and wait until its control
/// socket accepts connections.
pub fn start_owner(options: &OwnerStartOptions<'_>) -> Result<(), RemotePlayerError> {
    let exe = match options.owner_exe {
        Some(exe) => exe.to_path_buf(),
        None => std::env::current_exe()
            .map_err(|e| io::Error::new(e.kind(), format!("cannot locate binary: {e}")))?,
    };
    let socket = single_instance::socket_path();
    local_daemon::spawn_detached(&exe, &socket.to_string_lossy(), options.log_level.cloned())
}

/// Attach to this machine's Owner as a Client and return the `App` the Client
/// drives through [`crate::ClientCore`].
///
/// `owner_exe` is the `mbv` program this Client starts an Owner from when
/// none runs, and again after an Owner restart: `None` for the TUI Client,
/// which is `mbv` itself; the `mbv` program the GUI Client resolved, so the
/// Owner is never the GUI program (design D9). The Pin-swap advertisement
/// every connection sends follows the Client kind this process declared at
/// start-up ([`declare_gui_client`]): the TUI Client advertises `pin-swap`,
/// the GUI Client sends a plain Hello.
///
/// The Owner link starts MPRIS from `App` construction (#160, moved there from
/// `startup` in #175 so `App` owns the resulting handle and can `rebind` it
/// later — see `App::switch_to_direct_remote` / `App::restore_local_mode`).
/// That is safe against a same-machine bus-name collision because `mbvd` has
/// no D-Bus/zbus dependency and never claims `org.mpris.MediaPlayer2.mbv`
/// itself (`on_player_ready` is wired to a no-op in `crates/mbvd/src/main.rs`),
/// so this Client is the only thing that will ever own the name for a
/// daemon-connected session.
pub fn attach_owner(config: &Config, owner_exe: Option<&Path>) -> Result<App, RemotePlayerError> {
    let client = cached_emby_client(config);
    let (remote, player_rx) = RemotePlayer::connect_endpoint(
        &DaemonEndpoint::Local,
        crate::pin::pin_swap_hello().as_ref(),
    )?;
    let mut app = App::new_remote_optional_with_config(
        client,
        remote,
        player_rx,
        &DaemonEndpoint::Local,
        config.clone(),
    );
    app.set_owner_exe(owner_exe.map(Path::to_path_buf));
    Ok(app)
}

/// The notice a Client shows when a live Owner holds the lock but does not
/// accept connections. It names the Owner's PID when the lock file carries
/// one. One text for every Client: the TUI Client exits with it, and the GUI
/// Client shows it in its window.
#[must_use]
pub fn refusal_text() -> String {
    let lock_path = mbv_config::owner_lock_path();
    let pid = match mbv_daemon::locked_owner_pid(&lock_path) {
        Some(pid) => format!(
            "mbv: owner process PID is {pid} (per {}).",
            lock_path.display()
        ),
        None => format!(
            "mbv: could not determine the owner process PID from {}.",
            lock_path.display()
        ),
    };
    format!(
        "mbv: the local owner process is not accepting connections.\n{pid}\n\
         mbv: use `mbv -q` to stop the owner, then try again."
    )
}

/// The notice a Client shows when a live Owner already serves an exclusive
/// Client and stay-alive is off. One text for every Client, byte for byte:
/// the TUI Client exits with it (`startup::run_local_instance`), and the GUI
/// Client shows it in its window — the spec's "the same explanation a second
/// TUI Client gets".
#[must_use]
pub fn exclusive_owner_refusal_text(error: &RemotePlayerError) -> String {
    format!(
        "mbv: refusing a second terminal: {error}.\n\
         mbv: only one terminal may use playback while stay-alive is off.\n\
         mbv: close or stop that instance with `mbv -q`, or enable stay-alive to run several terminals at once."
    )
}

/// Declare this process the GUI Client (design D9 of `add-gui-front-end`).
/// Its Owner-link connections — the initial attach and every later App-side
/// reconnect — send a plain Client Hello, so the Owner never picks this
/// Client as a Pin-swap target: the GUI program is neither a terminal Client
/// nor a pinned panel, and the swap's launch snapshot is a TUI shell query
/// it cannot produce. Call once at start-up, before the first Owner
/// connection. The TUI Client is the default and always takes part.
pub fn declare_gui_client() {
    crate::pin::claim_pin_swap_participant(false);
}

/// The cached Emby client the shell starts from: the saved credential and the
/// configured setup, or `None` when either is absent.
fn cached_emby_client(config: &Config) -> Option<EmbyClient> {
    let token = mbv_config::load_service_secret(mbv_queue::ServiceKind::Emby)?;
    let setup = config.emby_setup.as_ref()?;
    let mut client = EmbyClient::new(config.clone());
    client.config.server_url.clone_from(&setup.server_url);
    client.user_id.clone_from(&setup.user_id);
    client.token = token;
    Some(client)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A client missing `user_id` builds `/Users//Views`-shaped paths --
    /// Emby throws on the empty GUID segment rather than rejecting cleanly.
    /// `construct.rs` marks a cached client `Ready` immediately without
    /// re-authenticating, so a blank `user_id` here reaches real requests.
    #[test]
    fn cached_emby_client_carries_user_id_from_setup() {
        let _state_dir = mbv_config::TestStateDirGuard::new();
        mbv_config::save_service_secret(mbv_queue::ServiceKind::Emby, "tok").unwrap();
        let config = Config {
            emby_setup: Some(mbv_config::EmbySetup::new(
                "http://emby.example:8096",
                "the-user-id",
            )),
            ..Config::default()
        };
        let client = cached_emby_client(&config).expect("cached client");
        assert_eq!(client.user_id, "the-user-id");
    }
}
