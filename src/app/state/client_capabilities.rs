//! The neutral facts the Clients differ on (design D6a). The core names no
//! Client: wherever the TUI and the GUI Client behave differently, the core
//! reads one of these capabilities instead of a Client-kind flag.

/// One thing a Client presents. The core never names a Client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientCapability {
    /// The Client may present one panel only, and hides an empty queue
    /// column in its two-panel layout (TUI: yes, GUI: no).
    TerminalPanelRules,
    /// The Client paints the visualizer and the visual-slot hide control
    /// (TUI: yes, GUI: no).
    VisualSlotControls,
    /// The Client shows one queue entry at a time that the user picks: a
    /// Named queue or the peer's queue (TUI: no, GUI: yes).
    ShownQueueEntries,
    /// The Client paints the transport chips, the seek-10/30 controls, the
    /// playback-target chooser, and moves queue rows by key (TUI: no,
    /// GUI: yes).
    TransportChipKeys,
}

impl ClientCapability {
    /// The bit this capability owns in `ClientCapabilities`.
    const fn bit(self) -> u8 {
        match self {
            Self::TerminalPanelRules => 1 << 0,
            Self::VisualSlotControls => 1 << 1,
            Self::ShownQueueEntries => 1 << 2,
            Self::TransportChipKeys => 1 << 3,
        }
    }
}

/// The capabilities of this Client. Each Client builds its own value: the
/// default holds none of them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClientCapabilities(u8);

impl ClientCapabilities {
    /// No capabilities.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// This set plus `capability`.
    #[must_use]
    pub const fn with(self, capability: ClientCapability) -> Self {
        Self(self.0 | capability.bit())
    }

    /// This set without `capability`.
    #[must_use]
    pub const fn without(self, capability: ClientCapability) -> Self {
        Self(self.0 & !capability.bit())
    }

    /// Whether this set holds `capability`.
    #[must_use]
    pub const fn has(self, capability: ClientCapability) -> bool {
        self.0 & capability.bit() != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
