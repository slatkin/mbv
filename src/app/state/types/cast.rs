// The event the TUI's one remaining Cast background worker reports: a
// completed discovery browse for the F3 panel's merged target list. The
// receiver itself is attached by the Owner (its Cast output, design D13),
// so no attachment or transport event crosses this channel any more.

use mbv_cast::discovery::CastReceiver;

/// A background cast discovery browse (`App::spawn_cast_discovery`)
/// completed -- possibly empty, since `browse_cast_receivers` never errors
/// (see `mbv_cast::discovery`). Drives `App::rebuild_panel_targets` (8.1).
pub(in crate::app) enum CastEvent {
    DiscoveryCompleted(Vec<CastReceiver>),
}
