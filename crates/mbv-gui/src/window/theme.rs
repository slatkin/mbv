//! The theme layer (D6 of `replace-tauri-with-blitz`): the port of
//! `lib/bridge/theme.ts`. The theme section's role colours go into CSS
//! custom properties on `:root` through the [`ThemeStyle`] element, and
//! `app.css` points the shadcn variables at them, so shadcn
//! components paint in the mbv palette with no second palette.

use std::fmt::Write as _;

use dioxus_native::prelude::*;

use crate::bridge::frame::theme::{GuiChrome, GuiRole, GuiTheme};
use crate::window::sections::Sections;

/// The CSS custom property a role colour writes to (D6): the `mbv` prefix
/// plus the role's camelCase wire name, such as `--mbv-textPrimary`. The
/// prefix keeps the role properties clear of the shadcn variables
/// that point at them (`accent` would otherwise collide with `--accent`).
#[must_use]
pub fn role_property(role: GuiRole) -> String {
    format!("--mbv-{}", role.name())
}

/// The `var(--mbv-<role>)` value a part paints in: the custom property a
/// role colour reads from, as every component's inline styles name it.
#[must_use]
pub fn role_var(role: GuiRole) -> String {
    format!("var({})", role_property(role))
}

/// The `var(--canvas-<name>)` value a part paints in: the Library and queue
/// canvas hexes, which name no theme role slot. Every painter reads them
/// from here, so the palette's canvas names live in one place.
#[must_use]
pub fn canvas_var(name: &str) -> String {
    format!("var(--canvas-{name})")
}

/// The Queue column's `--ef-*` palette names (design D1 of
/// `gui-queue-panel-redesign`): the Everforest Dark Hard values the redesign's
/// canvas names, which `assets/app.css` defines. A painter reads one through
/// [`ef_var`], so a name the stylesheet does not define fails
/// `every_ef_token_names_a_stylesheet_property` instead of painting nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EfToken {
    Player,
    BgDim,
    Bg0,
    Bg1,
    Bg2,
    Bg3,
    Bg4,
    Bg5,
    BgGreen,
    BgRed,
    Fg,
    Grey0,
    Grey1,
    Grey2,
    Green,
    Aqua,
    Orange,
    Red,
}

impl EfToken {
    /// Every token, in the stylesheet's declaration order.
    pub const ALL: [Self; 18] = [
        Self::Player,
        Self::BgDim,
        Self::Bg0,
        Self::Bg1,
        Self::Bg2,
        Self::Bg3,
        Self::Bg4,
        Self::Bg5,
        Self::BgGreen,
        Self::BgRed,
        Self::Fg,
        Self::Grey0,
        Self::Grey1,
        Self::Grey2,
        Self::Green,
        Self::Aqua,
        Self::Orange,
        Self::Red,
    ];

    /// The token's name after `--ef-`, such as `bg-green`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Player => "player",
            Self::BgDim => "bg-dim",
            Self::Bg0 => "bg0",
            Self::Bg1 => "bg1",
            Self::Bg2 => "bg2",
            Self::Bg3 => "bg3",
            Self::Bg4 => "bg4",
            Self::Bg5 => "bg5",
            Self::BgGreen => "bg-green",
            Self::BgRed => "bg-red",
            Self::Fg => "fg",
            Self::Grey0 => "grey0",
            Self::Grey1 => "grey1",
            Self::Grey2 => "grey2",
            Self::Green => "green",
            Self::Aqua => "aqua",
            Self::Orange => "orange",
            Self::Red => "red",
        }
    }

    /// The `var(--ef-<name>)` value a part paints in.
    #[must_use]
    pub const fn var(self) -> &'static str {
        match self {
            Self::Player => "var(--ef-player)",
            Self::BgDim => "var(--ef-bg-dim)",
            Self::Bg0 => "var(--ef-bg0)",
            Self::Bg1 => "var(--ef-bg1)",
            Self::Bg2 => "var(--ef-bg2)",
            Self::Bg3 => "var(--ef-bg3)",
            Self::Bg4 => "var(--ef-bg4)",
            Self::Bg5 => "var(--ef-bg5)",
            Self::BgGreen => "var(--ef-bg-green)",
            Self::BgRed => "var(--ef-bg-red)",
            Self::Fg => "var(--ef-fg)",
            Self::Grey0 => "var(--ef-grey0)",
            Self::Grey1 => "var(--ef-grey1)",
            Self::Grey2 => "var(--ef-grey2)",
            Self::Green => "var(--ef-green)",
            Self::Aqua => "var(--ef-aqua)",
            Self::Orange => "var(--ef-orange)",
            Self::Red => "var(--ef-red)",
        }
    }
}

/// The `var(--ef-<name>)` value a Queue column part paints in. The const
/// return lets a `const` and a plain style field hold it alike.
#[must_use]
pub const fn ef_var(token: EfToken) -> &'static str {
    token.var()
}

/// A transport glyph's colour: its role while actionable, the muted role
/// when not — the availability gate the TUI's glyph colours carry.
#[must_use]
pub fn glyph_color(available: bool, role: GuiRole) -> String {
    if available {
        role_var(role)
    } else {
        role_var(GuiRole::TextMuted)
    }
}

/// Writes a theme into the `:root` rule the [`ThemeStyle`] element renders
/// (D6). Only the roles and chrome fills the theme carries are set; a role
/// the theme omits keeps its last value, so a theme application is a
/// replace, not a reset. An absent theme writes nothing, so `app.css`'s
/// fallbacks stay and the chrome paints before the first theme.
///
/// The rule's `--mbv-*` set is the reference build's set: the roles and the
/// eight single-word chrome fills ([`write_chrome`] says why the other six
/// are absent).
#[must_use]
pub fn theme_style_rule(theme: Option<&GuiTheme>) -> String {
    let Some(theme) = theme else {
        return String::new();
    };
    let mut rule = String::from(":root { ");
    for (role, rgb) in &theme.roles {
        let _ = writeln!(
            rule,
            "{}: rgb({}, {}, {});",
            role_property(*role),
            rgb.r,
            rgb.g,
            rgb.b
        );
    }
    write_chrome(&mut rule, &theme.chrome);
    rule.push_str(" }");
    rule
}

/// Writes the chrome fills into the rule. The destructure names every field,
/// so a chrome addition breaks this until the writer answers for the fill —
/// the same compiler check that the role names of the DTO give the chrome
/// writer.
///
/// The writer sets the eight fills whose concept has a one-word name and
/// leaves the six two-word fills unwritten, because that is the property set
/// the reference front end defined. Its `GuiChrome` derived
/// `Serialize` with no `rename_all`, so `applyTheme` wrote
/// `--mbv-queue_column`, `--mbv-queue_panel`, and the rest, while every
/// consumer read `var(--mbv-queueColumn)`, `var(--mbv-queuePanel)`, and the
/// rest. Those six properties were therefore never defined in the webview:
/// each consumer's `var()` was invalid at computed-value time, which left a
/// `background` transparent (the queue column's backdrop showed through the
/// recessed box, the footer band, and the compact strip) and the scrubber
/// thumb's `box-shadow` ring unpainted. Writing the
/// camelCase names here would paint four fills the reference build never
/// painted, so the port reproduces its token set instead. The six fields
/// stay on the DTO — they still name the surface mapping — and stay bound so
/// the destructure keeps its exhaustiveness check.
fn write_chrome(rule: &mut String, chrome: &GuiChrome) {
    let GuiChrome {
        background,
        card,
        popover,
        muted,
        secondary,
        sidebar,
        border,
        input,
        queue_column: _queue_column,
        queue_panel: _queue_panel,
        queue_playback_band: _queue_playback_band,
        queue_footer_band: _queue_footer_band,
    } = chrome;
    let _ = writeln!(
        rule,
        "--mbv-background: rgb({}, {}, {});",
        background.r, background.g, background.b
    );
    let _ = writeln!(rule, "--mbv-card: rgb({}, {}, {});", card.r, card.g, card.b);
    let _ = writeln!(
        rule,
        "--mbv-popover: rgb({}, {}, {});",
        popover.r, popover.g, popover.b
    );
    let _ = writeln!(
        rule,
        "--mbv-muted: rgb({}, {}, {});",
        muted.r, muted.g, muted.b
    );
    let _ = writeln!(
        rule,
        "--mbv-secondary: rgb({}, {}, {});",
        secondary.r, secondary.g, secondary.b
    );
    let _ = writeln!(
        rule,
        "--mbv-sidebar: rgb({}, {}, {});",
        sidebar.r, sidebar.g, sidebar.b
    );
    let _ = writeln!(
        rule,
        "--mbv-border: rgb({}, {}, {});",
        border.r, border.g, border.b
    );
    let _ = writeln!(
        rule,
        "--mbv-input: rgb({}, {}, {});",
        input.r, input.g, input.b
    );
}

/// The theme's `style` element (D6): the `:root` rule the last carried theme
/// section built. The root renders it with the attach, overlay, Library,
/// Settings, and player trees, as `App` composes them.
#[component]
pub fn ThemeStyle() -> Element {
    let sections = use_context::<Sections>();
    let theme = sections.theme.read().clone();
    let rule = theme_style_rule(theme.as_ref());
    rsx! {
        style { {rule} }
    }
}

#[cfg(test)]
mod tests;
