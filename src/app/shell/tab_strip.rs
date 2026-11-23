//! The tab bar's scroll anchor (design D6b).
//!
//! The tab strip is TUI presentation: its scroll anchor, the window it
//! reveals, and the width rule that window is measured against all live on the
//! shell `Model`. The core only sets `App::tab_reveal_requested` where the
//! current tab must be scrolled into view; the shell takes that request after
//! every update step and tick drain, before it paints or resolves a pointer
//! press, so every `tab_scroll` reader sees the revealed value at the same
//! point it always did.

use super::Model;
use crate::app::App;
use mbv_render::arrangements::chrome::tab_strip_text_width;
use ratatui::layout::Rect;

impl Model {
    /// Run the reveal rule when the core requested it, and clear the request.
    /// Idempotent while no request is pending: a request the core raises
    /// during the tick is applied once, here, before the tick's paint and
    /// press resolution.
    pub(in crate::app) fn reveal_requested_tab(&mut self) {
        if !std::mem::take(&mut self.app.tab_reveal_requested) {
            return;
        }
        self.tab_scroll = reveal_scroll(&self.app, self.tab_scroll);
    }
}

/// The smallest scroll anchor that shows the current tab, given the current
/// anchor. A tab left of the anchor pulls the anchor onto it; a tab right of
/// the visible window advances the anchor until the window reaches it.
fn reveal_scroll(app: &App, anchor: usize) -> usize {
    if app.tab_count() == 0 {
        return anchor;
    }
    let pos = app
        .tab
        .to_position_with_counts(app.libs.len(), app.feeds_tab_pos());
    if pos < anchor {
        return pos;
    }
    let avail_w = tab_strip_width(app);
    let widths = tab_title_widths(app);
    let mut scroll = anchor;
    loop {
        let (_, end) =
            mbv_render::components::chrome_tabs::visible_tab_range(&widths, scroll, avail_w);
        if pos < end {
            return scroll;
        }
        scroll += 1;
    }
}

/// Residual A: derive the tab-strip width from the shared arrangement
/// primitive instead of reading the painted `layout.tabs_area`.
fn tab_strip_width(app: &App) -> u16 {
    let area = Rect::new(0, 0, app.terminal_width, app.terminal_height);
    let chrome = super::draw::compute_chrome_geometry(app, area);
    if chrome.right_visible {
        tab_strip_text_width(chrome.tab_bar_area.width)
    } else {
        app.terminal_width
            .saturating_sub(mbv_render::layout::TABBAR_LEFT_RESERVE)
    }
}

/// Tab-bar title widths: Continue + one per library + Feeds when present.
fn tab_title_widths(app: &App) -> Vec<u16> {
    let pad: u16 = 2;
    let title_width = |title: &str| {
        u16::try_from(title.chars().count())
            .unwrap_or(u16::MAX)
            .saturating_add(pad)
    };
    let mut w = vec![title_width(mbv_ui_model::ui_util::continue_tab_title(
        app.use_nerd_fonts,
    ))];
    for l in &app.libs {
        w.push(title_width(&l.library.name));
    }
    for l in &app.audiobookshelf_libraries {
        w.push(title_width(&l.name));
    }
    if app.has_feeds_subscriptions() {
        w.push(title_width("Feeds"));
    }
    w
}
