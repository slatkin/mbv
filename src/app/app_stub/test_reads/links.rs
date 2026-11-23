//! The two-Player-link fixture (`split-tui-gui-clients` row 2.3, correction
//! round 1): an `App` whose active Player connection and whose suspended home
//! link are separate stub Players, each with its own command receiver.
//!
//! The fixture exists for one contract: a reply that names a link must reach
//! that link's Player and no other. A single stub Player cannot tell the two
//! links apart, which is exactly how an inverted link mapping went unnoticed.

use super::super::App;
use mbv_ctrl::CtrlCmd;
use std::sync::mpsc;

impl App {
    /// Put a stub Player on the active connection, and keep what it receives.
    #[cfg(any(test, feature = "test"))]
    pub fn set_player_for_test(&mut self) {
        let (remote, player_rx, cmd_rx) =
            mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
        remote.update_status(|status| status.volume_max = 100);
        self.player = mbv_player::PlayerProxy::from_remote(remote, false);
        self.player_rx = player_rx;
        ACTIVE_COMMANDS.with(|slot| *slot.borrow_mut() = Some(cmd_rx));
    }

    /// Suspend a stub home link, and keep what it receives. The active
    /// connection keeps answering for the peer link beside it.
    #[cfg(any(test, feature = "test"))]
    pub fn set_suspended_local_for_test(&mut self) {
        let (remote, player_rx, cmd_rx) =
            mbv_remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
        self.suspended_local = Some(crate::app::state::playback::SuspendedLocalSession {
            player: mbv_player::PlayerProxy::from_remote(remote, false),
            player_rx,
        });
        HOME_COMMANDS.with(|slot| *slot.borrow_mut() = Some(cmd_rx));
    }

    /// Whether the active connection has received a `SwapPrepared` reply
    /// since the last take, draining what it received.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn took_swap_prepared_on_active_for_test(&mut self) -> bool {
        ACTIVE_COMMANDS.with(took_swap_prepared)
    }

    /// Whether the suspended home link has received a `SwapPrepared` reply
    /// since the last take, draining what it received.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn took_swap_prepared_on_home_for_test(&mut self) -> bool {
        HOME_COMMANDS.with(took_swap_prepared)
    }
}

/// Drain `slot`'s receiver and answer whether a `SwapPrepared` reply was among
/// what it held.
#[cfg(any(test, feature = "test"))]
fn took_swap_prepared(slot: &std::cell::RefCell<Option<mpsc::Receiver<CtrlCmd>>>) -> bool {
    let mut slot = slot.borrow_mut();
    let Some(receiver) = slot.as_mut() else {
        return false;
    };
    receiver
        .try_iter()
        .any(|command| matches!(command, CtrlCmd::SwapPrepared))
}

thread_local! {
    /// What the fixture's active stub Player received. Per thread, so two
    /// tests running side by side never read each other's commands.
    static ACTIVE_COMMANDS: std::cell::RefCell<Option<mpsc::Receiver<CtrlCmd>>> =
        const { std::cell::RefCell::new(None) };
    /// What the fixture's suspended home stub Player received.
    static HOME_COMMANDS: std::cell::RefCell<Option<mpsc::Receiver<CtrlCmd>>> =
        const { std::cell::RefCell::new(None) };
}
