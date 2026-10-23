//! `[panel]` layout settings for the pinned panel.
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

/// The pinned panel's width in logical pixels: `1..=65535`.
pub const PANEL_WIDTH_MIN: u16 = 1;
/// The `[panel] width` default in logical pixels.
pub const DEFAULT_PANEL_WIDTH: u16 = 400;
/// The `[panel] width_expanded` default in logical pixels.
pub const DEFAULT_PANEL_WIDTH_EXPANDED: u16 = 1100;

/// The `[panel] accent_color` default.
pub const DEFAULT_PANEL_ACCENT_COLOR: PanelAccentColor = PanelAccentColor([0xda, 0xbc, 0x7f]);
/// The `[panel] accent_width` default in pixels (`pinned-panel-focus-accent`).
pub const DEFAULT_PANEL_ACCENT_WIDTH: NonZeroU16 = NonZeroU16::MIN;

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
        let [_, r, g, b] = u32::from_str_radix(hex, 16).ok()?.to_be_bytes();
        Some(Self([r, g, b]))
    }
}

impl std::fmt::Display for PanelAccentColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.0[0], self.0[1], self.0[2])
    }
}

/// The `[panel]` section: docking side, collapsed and expanded widths in
/// logical pixels, whether the collapsed panel reserves its strip, four
/// gutters in logical pixels (negative values are allowed), the focus accent
/// stroke (flag, colour, width in pixels), and the two terminal argv
/// prefixes. Every value is already validated; an out-of-range or malformed
/// TOML value was replaced with its default while parsing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanelConfig {
    /// The edge the panel docks to.
    pub side: PanelSide,
    /// Panel width in logical pixels (collapsed).
    pub width: u16,
    /// Panel width in logical pixels when expanded.
    pub width_expanded: u16,
    /// Whether the collapsed panel reserves a strip beside it (pushes the
    /// tiled windows); `false` makes it cover them instead. The expanded
    /// width always covers. The `Reserve space` F2 row edits this.
    pub reserve: bool,
    /// Inset from the output's top edge.
    pub gutter_top: i32,
    /// Inset from the output's bottom edge.
    pub gutter_bottom: i32,
    /// Inset from the output's left edge.
    pub gutter_left: i32,
    /// Inset from the output's right edge.
    pub gutter_right: i32,
    /// Whether the focus accent stroke is drawn.
    pub accent: bool,
    /// The focus accent stroke's colour.
    pub accent_color: PanelAccentColor,
    /// The focus accent stroke's width in pixels.
    pub accent_width: NonZeroU16,
    /// Terminal emulator argv prefix used to relaunch the TUI when the Owner
    /// process swaps the pinned panel for a terminal Client: the Owner appends
    /// `mbv` and runs the argv. `None` when the
    /// key is absent, empty, or malformed; callers fall back to `$TERMINAL -e`.
    /// Not a F2 Panel row: it is edited in `config.toml` only.
    pub terminal: Option<Vec<String>>,
    /// Terminal emulator argv prefix that runs mbv inside the panel: `--pin`
    /// appends the mbv exe and runs the argv. `None` when the key
    /// is absent, empty, or malformed; callers fall back to `$TERMINAL -e`.
    /// Kept apart from `terminal` because the panel's terminal must open its
    /// window in the process `pinwin run` starts, which a terminal that hands
    /// the launch to a running copy does not. Not a F2 Panel row: it is edited
    /// in `config.toml` only.
    pub pin_terminal: Option<Vec<String>>,
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            side: PanelSide::Left,
            width: DEFAULT_PANEL_WIDTH,
            width_expanded: DEFAULT_PANEL_WIDTH_EXPANDED,
            reserve: true,
            gutter_top: 0,
            gutter_bottom: 0,
            gutter_left: 0,
            gutter_right: 0,
            accent: true,
            accent_color: DEFAULT_PANEL_ACCENT_COLOR,
            accent_width: DEFAULT_PANEL_ACCENT_WIDTH,
            terminal: None,
            pin_terminal: None,
        }
    }
}
