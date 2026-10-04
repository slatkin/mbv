//! `[panel]` layout settings for the pinned panel (change `pin-mbv-in-pinwin`,
//! design D6).
//!
//! Values are validated here with plain Rust range checks only; whether a
//! layout actually fits the output needs live monitor metrics, so it is
//! `pinwin`'s call (`Panel::start` / `Panel::apply_layout_animated`), never
//! `mbv-config`'s.

use std::num::NonZeroU16;

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
/// The `[panel] cols` default (design D6).
pub const DEFAULT_PANEL_COLS: u16 = 40;
/// The `[panel] cols_expanded` default (`panel-expand-toggle`, design D2).
pub const DEFAULT_PANEL_COLS_EXPANDED: u16 = 120;

/// The `[panel] accent_color` default (`pinned-panel-focus-accent`, design D2).
pub const DEFAULT_PANEL_ACCENT_COLOR: PanelAccentColor = PanelAccentColor([0xda, 0xbc, 0x7f]);

/// The `[panel] accent_color` value: an RGB triple parsed from `"#RRGGBB"` or
/// `"RRGGBB"` and displayed as `#rrggbb`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelAccentColor(pub [u8; 3]);

impl PanelAccentColor {
    /// Parse an `accent_color` value from config; a malformed value yields
    /// `None` so callers fall back to `DEFAULT_PANEL_ACCENT_COLOR`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let hex = s.strip_prefix('#').unwrap_or(s);
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let mut rgb = [0u8; 3];
        for (byte, chunk) in rgb.iter_mut().zip([&hex[0..2], &hex[2..4], &hex[4..6]]) {
            *byte = u8::from_str_radix(chunk, 16).ok()?;
        }
        Some(Self(rgb))
    }
}

impl std::fmt::Display for PanelAccentColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.0[0], self.0[1], self.0[2])
    }
}

/// The `[panel]` section: docking side, width in columns and four gutters in
/// logical pixels (negative values are allowed). Every value is already
/// validated; an out-of-range or malformed TOML value was replaced with its
/// default while parsing (design D6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelConfig {
    /// The edge the panel docks to.
    pub side: PanelSide,
    /// Panel width in terminal columns (collapsed).
    pub cols: u16,
    /// Panel width in terminal columns when expanded (`panel-expand-toggle`,
    /// design D2).
    pub cols_expanded: u16,
    /// Inset from the output's top edge.
    pub gutter_top: i32,
    /// Inset from the output's bottom edge.
    pub gutter_bottom: i32,
    /// Inset from the output's left edge.
    pub gutter_left: i32,
    /// Inset from the output's right edge.
    pub gutter_right: i32,
    /// Whether the focus accent stroke is drawn (`pinned-panel-focus-accent`).
    pub accent: bool,
    /// The focus accent stroke's colour.
    pub accent_color: PanelAccentColor,
    /// The focus accent stroke's width in pixels.
    pub accent_width: NonZeroU16,
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            side: PanelSide::Left,
            cols: DEFAULT_PANEL_COLS,
            cols_expanded: DEFAULT_PANEL_COLS_EXPANDED,
            gutter_top: 0,
            gutter_bottom: 0,
            gutter_left: 0,
            gutter_right: 0,
            accent: true,
            accent_color: DEFAULT_PANEL_ACCENT_COLOR,
            accent_width: NonZeroU16::MIN,
        }
    }
}
