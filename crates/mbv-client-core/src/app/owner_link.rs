//! The Client-side Owner link (design D2, D7, and D9 of `extract-client-core`):
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
//! Owner the Client starts is the `mbv-owner` program beside the running
//! Client ([`owner_program`]); a Client that must name another program passes
//! it in [`OwnerStartOptions::owner_exe`].
//!
//! The start step itself — the Owner's argv, the graphical session's display
//! environment, and the detached spawn that waits for the control socket —
//! sits in the [`start`] child module, so both Clients start the Owner from
//! one place (design D3 of `split-tui-gui-clients`).

mod start;

use std::io;
use std::path::{Path, PathBuf};

use crate::app::App;
use crate::app::config::Config;
use crate::{ClientCapabilities, ClientCapability};
use mbv_emby::EmbyClient;
use mbv_remote_player::{ClientHelloClaims, DaemonEndpoint, RemotePlayer, RemotePlayerError};
use start::spawn_detached;

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
    /// The program that becomes the Owner. `None` starts [`owner_program`],
    /// the `mbv-owner` program beside this Client, so the Owner is never the
    /// running Client's own program.
    pub owner_exe: Option<&'a Path>,
    /// The log level to forward to the Owner process.
    pub log_level: Option<&'a mbv_core::applog::LogSpec>,
}

/// Find this machine's Owner process. The probe's freshly acquired lock guard
/// is released here: a Client never holds the Owner lock, so the Owner that
/// [`start_owner`] launches re-acquires it as part of its own startup.
pub fn find_owner() -> io::Result<OwnerState> {
    let socket = mbv_daemon::socket_path();
    let lock = mbv_config::owner_lock_path();
    Ok(match mbv_daemon::resolve(&socket, &lock)? {
        mbv_daemon::Resolution::Fresh(guard) => {
            drop(guard);
            OwnerState::Start
        }
        mbv_daemon::Resolution::Attach => OwnerState::Attach,
        mbv_daemon::Resolution::Refuse => OwnerState::Refuse,
    })
}

/// Start the Owner process from [`owner_program`], or from the program
/// `options` names, and wait until its control socket accepts connections.
pub fn start_owner(options: &OwnerStartOptions<'_>) -> Result<(), RemotePlayerError> {
    let exe = options
        .owner_exe
        .map_or_else(owner_program, Path::to_path_buf);
    let socket = mbv_daemon::socket_path();
    spawn_detached(&exe, &socket.to_string_lossy(), options.log_level.cloned())
}

/// The file name of the Owner program (design D8 of `3-add-owner-program`).
fn owner_program_name() -> String {
    format!("mbv-owner{}", std::env::consts::EXE_SUFFIX)
}

/// The Owner program a Client starts (design D8 of `3-add-owner-program`):
/// the `mbv-owner` file in `dir`, the running executable's own directory,
/// when `sibling_present` says it is there, else the bare `mbv-owner` name
/// that `PATH` resolves.
fn owner_program_in(dir: Option<&Path>, sibling_present: bool) -> PathBuf {
    match dir {
        Some(dir) if sibling_present => dir.join(owner_program_name()),
        _ => PathBuf::from(owner_program_name()),
    }
}

/// The `mbv-owner` program beside the running Client's executable, else the
/// `mbv-owner` name on `PATH`: the one rule both Clients start an Owner with
/// (design D8 of `3-add-owner-program`).
#[must_use]
pub fn owner_program() -> PathBuf {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    let sibling_present = dir
        .as_deref()
        .is_some_and(|dir| dir.join(owner_program_name()).is_file());
    owner_program_in(dir.as_deref(), sibling_present)
}

/// Attach to this machine's Owner as a Client and return the `App` the Client
/// drives through [`crate::ClientCore`].
///
/// `owner_exe` is the program this Client starts an Owner from when none
/// runs, and again after an Owner restart: `None` starts [`owner_program`],
/// and a Client that resolves the program itself names it, so the Owner is
/// never the running Client's own program (design D8 and D9 of
/// `3-add-owner-program`). The Hello claims every connection sends follow
/// `capabilities` and this process's Pin-swap participation claim: the TUI
/// Client advertises `pin-swap`, the GUI Client sends a plain Hello and
/// claims `named-queues` on its local link (design D6a).
///
/// The Owner link starts MPRIS from `App` construction (#160, moved there from
/// `startup` in #175 so `App` owns the resulting handle and can `rebind` it
/// later — see `App::switch_to_direct_remote` / `App::restore_local_mode`).
/// The Client publisher claims no name up front (design D1): a Client attached
/// to the local Owner holds no name, because the Owner holds
/// `org.mpris.MediaPlayer2.mbv` for its lifetime, and a Client controlling a
/// remote Player owner claims `org.mpris.MediaPlayer2.mbv.instance<pid>` only
/// while the local Owner is not playing. `mbvd` has no D-Bus/zbus dependency;
/// its Player-ready hook is a no-op (design D5).
///
/// `image_decode` is the calling Client's image setting (design D14): the TUI
/// keeps decoded pixels, the GUI keeps the server's bytes only.
pub fn attach_owner(
    config: &Config,
    owner_exe: Option<&Path>,
    image_decode: mbv_images::ImageDecode,
    capabilities: ClientCapabilities,
) -> Result<App, RemotePlayerError> {
    // The GUI Client never takes part in Pin swap (design D9 of
    // `extract-client-core`): it is neither a terminal Client nor a pinned
    // panel, and the swap's launch snapshot is a TUI shell query it cannot
    // produce. No capability stands for it (design D6a), so the caller that
    // builds those capabilities claims its own non-participation before this
    // call, and this process's first and every later connection advertises
    // alike.
    let client = cached_emby_client(config);
    let claims = hello_claims(capabilities, &DaemonEndpoint::Local);
    let (remote, player_rx) = RemotePlayer::connect_endpoint(&DaemonEndpoint::Local, &claims)?;
    let mut app = App::new_remote_optional_with_config(
        client,
        remote,
        player_rx,
        &DaemonEndpoint::Local,
        config.clone(),
        image_decode,
    );
    app.set_owner_exe(owner_exe.map(Path::to_path_buf));
    app.set_client_capabilities(capabilities);
    Ok(app)
}

/// The Hello claims one connection to `endpoint` makes, built from
/// `capabilities` (design D6a of `decouple-client-core`). A Client that shows
/// one queue entry at a time claims `named-queues` on its local Owner link.
/// The Pin-swap claim follows this process's claimed Pin-swap participation
/// (`pin_swap_hello`, claimed at start-up).
#[must_use]
pub(in crate::app) fn hello_claims(
    capabilities: ClientCapabilities,
    endpoint: &DaemonEndpoint,
) -> ClientHelloClaims {
    ClientHelloClaims {
        pin_swap: crate::app::pin::pin_swap_hello(),
        named_queues: endpoint.is_local() && capabilities.has(ClientCapability::ShownQueueEntries),
    }
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
    use rstest::rstest;

    /// The Owner program is the sibling of the running Client when one is
    /// installed beside it, else the `mbv-owner` name on `PATH` (contract:
    /// `owner-program` "Clients start the Owner program beside them").
    #[rstest]
    #[case::sibling_present("/opt/mbv/bin", true, "/opt/mbv/bin/mbv-owner")]
    #[case::sibling_absent("/opt/mbv/bin", false, "mbv-owner")]
    fn owner_program_is_the_sibling_when_present_else_the_bare_name(
        #[case] dir: &str,
        #[case] sibling_present: bool,
        #[case] expected: &str,
    ) {
        assert_eq!(
            owner_program_in(Some(Path::new(dir)), sibling_present),
            PathBuf::from(expected)
        );
    }

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
