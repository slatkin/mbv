//! `[panel]` layout settings for the pinned panel (change `pin-mbv-in-pinwin`,
//! design D6).
//!
//! Values are validated here with plain Rust range checks only; whether a
//! layout actually fits the output needs live monitor metrics, so it is
//! `pinwin`'s call (`pinwin_start` / `pinwin_apply_layout`), never
//! `mbv-config`'s.

/// The screen edge the pinned panel docks to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PanelSide {
    /// Docked to the left edge.
    #[default]
    Left,
    /// Docked to the right edge.
    Right,
}

impl PanelSide {
    /// The `[panel] side` value as written to `config.toml`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            PanelSide::Left => "left",
            PanelSide::Right => "right",
        }
    }

    /// Parse a `side` value from config; unknown values yield `None` so
    /// callers fall back to `PanelSide::default()`.
    #[must_use]
    pub fn parse(s: &str) -> Option<PanelSide> {
        match s.trim().to_ascii_lowercase().as_str() {
            "left" => Some(PanelSide::Left),
            "right" => Some(PanelSide::Right),
            _ => None,
        }
    }
}

/// The pinned panel's width in terminal columns: `1..=65535` (design D6).
pub const PANEL_COLS_MIN: u16 = 1;
/// See [`PANEL_COLS_MIN`].
pub const PANEL_COLS_MAX: u16 = u16::MAX;
/// The `[panel] cols` default (design D6).
pub const DEFAULT_PANEL_COLS: u16 = 40;

/// The `[panel]` section: docking side, width in columns and four gutters in
/// logical pixels (negative values are allowed). Every value is already
/// validated; an out-of-range or malformed TOML value was replaced with its
/// default while parsing (design D6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelConfig {
    /// The edge the panel docks to.
    pub side: PanelSide,
    /// Panel width in terminal columns.
    pub cols: u16,
    /// Inset from the output's top edge.
    pub gutter_top: i32,
    /// Inset from the output's bottom edge.
    pub gutter_bottom: i32,
    /// Inset from the output's left edge.
    pub gutter_left: i32,
    /// Inset from the output's right edge.
    pub gutter_right: i32,
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            side: PanelSide::Left,
            cols: DEFAULT_PANEL_COLS,
            gutter_top: 0,
            gutter_bottom: 0,
            gutter_left: 0,
            gutter_right: 0,
        }
    }
}
