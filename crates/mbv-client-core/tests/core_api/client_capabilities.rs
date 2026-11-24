//! The capability flag-set contract (design D6a).

use mbv_client_core::{ClientCapabilities, ClientCapability};

/// The flag-set contract: the default holds nothing, `with` adds exactly
/// one capability, and `without` removes exactly one.
#[test]
fn with_adds_one_capability_and_without_removes_it() {
    let both = ClientCapabilities::default()
        .with(ClientCapability::TerminalPanelRules)
        .with(ClientCapability::VisualSlotControls);
    assert!(both.has(ClientCapability::TerminalPanelRules));
    assert!(both.has(ClientCapability::VisualSlotControls));
    assert!(!both.has(ClientCapability::ShownQueueEntries));
    assert_eq!(
        both.without(ClientCapability::TerminalPanelRules),
        ClientCapabilities::default().with(ClientCapability::VisualSlotControls)
    );
    assert_eq!(ClientCapabilities::default(), ClientCapabilities::empty());
    assert!(!ClientCapabilities::default().has(ClientCapability::TransportChipKeys));
}
