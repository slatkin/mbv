//! The menu-card chrome the queue canvas cards share: the chooser's
//! dropdown, the Queue row menu, and the confirm and daemon-lost overlays
//! paint the canvas's fill, border, radius, and shadow, so the four style
//! strings share one definition and cannot drift apart.

/// The menu-card chrome: background, border, radius, and shadow. Every
/// card's own style string embeds it, so a card paints the same chrome with
/// its own padding, width, and position around it.
pub(crate) const MENU_CARD_STYLE: &str = "background: var(--canvas-menu); border: 1px solid var(--canvas-menu-border); border-radius: 8px; box-shadow: var(--canvas-menu-shadow);";
