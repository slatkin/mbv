//! Semantic theme colours for every painted surface.
//!
//! Three closed kinds, one authority per kind:
//!
//! * `Slot` and `Theme` (crate-private): a slot is a bare colour position
//!   named by tier or hue; the `Theme` value holds one colour per slot, and
//!   `active()` is the single resolution point. `Theme::DEFAULT` is the only
//!   place a palette `Color::Rgb` literal lives.
//! * [`Role`]: what a foreground colour means (`Role::TextMetadata`,
//!   `Role::Accent`). Callers name the meaning, never the hue, and resolve
//!   it with [`Role::color`].
//! * [`Surface`]: what a background fill means (`Surface::MainContentBox`).
//!   Fills resolve only through the surface table, via [`surface_colors`],
//!   which reads the surface's (resting, focused) slot pair against the
//!   active theme.
//!
//! Layout and painting live in `mbv-render`, which consumes these roles and
//! surfaces.

#[cfg(test)]
mod palette_json;
mod role;
mod slot;
mod surface;
mod surface_resolve;
mod surface_table;

#[doc(inline)]
pub use role::{HERO_META_ROLES, Role};
#[doc(inline)]
pub use surface::{HINT_CHIPS, Surface};
#[doc(inline)]
pub use surface_resolve::surface_colors;

use ratatui::style::Color;

/// The selected/connected-bar text policy: a row painted over the opaque
/// selection bar (`Surface::SelectedRow`) renders in the bar's own foreground
/// because ordinary text roles do not read on the light fill; elsewhere a row
/// keeps the role it was given. Shared by the media list and the sessions
/// sidebar, the two surfaces that paint this bar.
#[must_use]
pub fn bar_role_fg(role: Color, on_bar: bool) -> Color {
    if on_bar {
        Role::SelectedRowFg.color()
    } else {
        role
    }
}
