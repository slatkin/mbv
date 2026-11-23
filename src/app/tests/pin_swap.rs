//! The pin-swap reply's link (`split-tui-gui-clients` row 2.3, correction
//! round 1): a `SwapPrepared` answer must travel on the connection the
//! `SwapPrepare` request arrived on.
//!
//! The shell's `PinSwapLink::Active` is the Client's active Player connection
//! and `PinSwapLink::Home` is the suspended home link, which is the core's
//! `QueueLink::Peer`/`QueueLink::Local` pairing (`App::queue_owner_player`).
//! Reading the pair the other way round answers on the wrong daemon: a
//! swapped-out Client would tell the home link it swapped in, or leave the
//! swap unanswered and let the Owner abandon it at its deadline.

use crate::app::QueueLink;
use crate::app::tests::make_app_stub;

/// A stub `App` with two stub Players: the active connection on `app.player`,
/// and — when `suspended` is set — a separate home link on
/// `app.suspended_local`. Each keeps what it receives, so a test reads which
/// one the reply went to.
fn app_with_two_links(suspended: bool) -> crate::app::App {
    let mut app = make_app_stub();
    app.set_player_for_test();
    if suspended {
        app.set_suspended_local_for_test();
    }
    app
}

#[test]
fn the_active_link_replies_on_the_active_player() {
    let mut app = app_with_two_links(true);

    assert!(app.reply_pin_swap_prepared(QueueLink::Peer));

    assert!(
        app.took_swap_prepared_on_active_for_test(),
        "the active link's SwapPrepare is answered on the active player"
    );
    assert!(
        !app.took_swap_prepared_on_home_for_test(),
        "the home link received nothing while the active link answered"
    );
}

#[test]
fn the_home_link_replies_on_the_suspended_home_player() {
    let mut app = app_with_two_links(true);

    assert!(app.reply_pin_swap_prepared(QueueLink::Local));

    assert!(
        app.took_swap_prepared_on_home_for_test(),
        "the home link's SwapPrepare is answered on the suspended home player"
    );
    assert!(
        !app.took_swap_prepared_on_active_for_test(),
        "the active link received nothing while the home link answered"
    );
}

#[test]
fn a_home_link_with_no_suspended_session_answers_nothing() {
    let mut app = app_with_two_links(false);

    assert!(!app.reply_pin_swap_prepared(QueueLink::Local));
    assert!(!app.took_swap_prepared_on_active_for_test());
}
