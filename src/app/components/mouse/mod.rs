//! Mouse-event delivery plumbing (ADR 0024).
//!
//! Phase 1 of `restore-mouse-support`: the subscription helper that every
//! mouse-eligible component is subscribed with by `sync_mouse_subscriptions`
//! (`src/app/shell_library.rs`). Phase 2 adds [`hit`] (`HitRegions<Tag>`) and
//! [`gesture`] (`MouseGestureState`) alongside this; their consumers land in
//! tasks 3.4-3.6.

pub mod gesture;
pub mod hit;

#[allow(unused_imports)] // consumers land in tasks 3.4-3.6
pub use gesture::{MouseGesture, MouseGestureState};
#[allow(unused_imports)] // consumers land in tasks 3.4-3.6
pub use hit::HitRegions;

use tuirealm::event::{KeyModifiers, MouseEventKind};
use tuirealm::subscription::{EventClause, MouseEventClause, Sub, SubClause};

use super::{ComponentId, UserEvent};

/// The any-position mouse subscription clause.
///
/// PINNED to `tuirealm` 4.1 (ADR 0024): `MouseEventClause::is_in_range`
/// compares only `column` and `row` and ignores `kind`/`modifiers`,
/// contradicting the crate's own `EventClause::forward` doc comment. This
/// single clause therefore delivers every `MouseEventKind` at every
/// coordinate; kind filtering happens inside each component's `on()`, never
/// here. Any `tuirealm` bump must re-verify that behaviour before merge.
///
/// The bounds are the half-open `Range<u16>` `0..u16::MAX` —
/// `0..=u16::MAX` does not type-check (`MouseEventClause` fields are `Range`,
/// not `RangeInclusive`).
pub fn mouse_event_clause() -> EventClause<UserEvent> {
    EventClause::Mouse(MouseEventClause {
        kind: MouseEventKind::Moved,
        modifiers: KeyModifiers::NONE,
        column: 0..u16::MAX,
        row: 0..u16::MAX,
    })
}

/// A `Sub` for [`mouse_event_clause`] with `SubClause::Always`. Mouse
/// eligibility is decided by `sync_mouse_subscriptions` adding and removing
/// this subscription, never by a `SubClause` predicate (ADR 0024 D2).
pub fn mouse_sub() -> Sub<ComponentId, UserEvent> {
    Sub::new(mouse_event_clause(), SubClause::Always)
}

#[cfg(test)]
mod tests;
