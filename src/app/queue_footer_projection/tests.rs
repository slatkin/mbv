//! The footer's session marker names the shown queue's Player owner and
//! carries whether that owner is live (`add-tauri-gui-client` design D4a: the
//! dot marks the live Owner). The fact is the shown link's own connection
//! state; it needs no second liveness store.

use crate::app::tests::make_app_stub;

/// The owner's liveness follows the shown link's connection: the stub's
/// Owner link is up, and a lost link reports the Owner gone.
#[test]
fn the_footer_owner_is_live_while_its_link_is_up() {
    let app = make_app_stub();
    assert!(
        app.queue_footer_projection().owner_live,
        "a live Owner link marks the owner live"
    );

    app.player.remote().set_disconnected_for_test(true);
    assert!(
        !app.queue_footer_projection().owner_live,
        "a lost Owner link marks the owner not live"
    );
}
