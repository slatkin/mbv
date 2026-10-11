use std::sync::atomic::{AtomicU8, Ordering};

/// Which kinds of local Client are attached to the Owner process, computed
/// by the ctrl registry on every connect and disconnect
/// (tray-open-terminal design D1). Owner-action and admin roles, and
/// connections over TCP, never count as attached Clients.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachedClients {
    /// No local Client is attached.
    None,
    /// At least one local Client is attached and none of them runs the
    /// pinned surface.
    Unpinned,
    /// A local Client runs the pinned surface.
    Pinned,
}

impl AttachedClients {
    fn to_u8(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Unpinned => 1,
            Self::Pinned => 2,
        }
    }

    /// Inverse of [`AttachedClients::to_u8`]. Only [`AttachedClientsCell::
    /// store`] writes the shared value, and it writes only the three tags
    /// above, so the catch-all arm is unreachable in practice; mapping it
    /// to [`AttachedClients::None`] keeps `load` total without inventing a
    /// fourth state.
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Unpinned,
            2 => Self::Pinned,
            _ => Self::None,
        }
    }
}

/// Shared [`AttachedClients`] value, read by the Tray and written by the
/// ctrl registry. The `u8` representation never leaks: `load` returns the
/// enum and `store` takes it, so no invalid raw value is observable by
/// callers.
#[derive(Debug)]
pub struct AttachedClientsCell(AtomicU8);

impl AttachedClientsCell {
    #[must_use]
    pub fn new(value: AttachedClients) -> Self {
        Self(AtomicU8::new(value.to_u8()))
    }

    #[must_use]
    pub fn load(&self) -> AttachedClients {
        AttachedClients::from_u8(self.0.load(Ordering::SeqCst))
    }

    pub fn store(&self, value: AttachedClients) {
        self.0.store(value.to_u8(), Ordering::SeqCst);
    }
}

impl Default for AttachedClientsCell {
    fn default() -> Self {
        Self::new(AttachedClients::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Contract: the cell round-trips every state, and `load` on a fresh
    /// cell returns exactly what `new` stored — the enum is the only form
    /// callers ever see.
    #[test]
    fn cell_loads_the_stored_state() {
        let cell = AttachedClientsCell::new(AttachedClients::None);
        assert_eq!(cell.load(), AttachedClients::None);

        cell.store(AttachedClients::Pinned);
        assert_eq!(cell.load(), AttachedClients::Pinned);

        cell.store(AttachedClients::Unpinned);
        assert_eq!(cell.load(), AttachedClients::Unpinned);
    }
}
