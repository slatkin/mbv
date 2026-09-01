//! Shared mouse delivery and gesture primitives.

pub mod gesture;
pub mod hit;

use tuirealm::event::{KeyModifiers, MouseButton, MouseEventKind};
use tuirealm::subscription::{EventClause, MouseEventClause, Sub, SubClause};

use super::{ComponentId, UserEvent};

/// Subscribe a mounted component to every terminal mouse position.
pub fn mouse_sub() -> Sub<ComponentId, UserEvent> {
    Sub::new(
        EventClause::Mouse(MouseEventClause {
            // TuiRealm 4.1's range matcher intentionally ignores kind; this
            // representative value therefore subscribes to every mouse kind.
            kind: MouseEventKind::Down(MouseButton::Left),
            modifiers: KeyModifiers::NONE,
            column: 0..u16::MAX,
            row: 0..u16::MAX,
        }),
        SubClause::Always,
    )
}
