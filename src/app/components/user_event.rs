//! `UserEvent` (design D5).
//!
//! `TuiRealm`'s `Application` is generic over a user-event type; the shell
//! never wires an event publisher (#609), so the only variant is the
//! clock tick tests inject to drive `handle_clock` paths. Completion
//! results do not ride events: the shell pushes the owned presentation
//! model directly into the mounted target via
//! `get_component_mut`+downcast (design D5).

#[cfg(test)]
use std::time::Instant;

/// `TuiRealm` user-event type (design D5). `Application` requires `UserEvent:
/// Eq + PartialEq + Clone + Send + 'static`; the convenience `Debug` derive
/// aids diagnostics. `Clock` reuses `std::time::Instant` (which is `Eq`).
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum UserEvent {
    #[cfg(test)]
    Clock(Instant),
}
