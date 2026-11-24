use crate::app::App;
use mbv_queue::TvContentMode;
use mbv_render::arrangements::wide_hero;
use mbv_render::components::widgets;
use mbv_ui_model::settings::PanelMode;
use ratatui::layout::Rect;

/// The right panel's content area for the current terminal size and
/// panel state, paint-free — `None` when the right panel is not visible
/// (e.g. Queue-only panel mode). Factored out of `wide_tv_show_area`
/// so every paint-free breakpoint consumer shares one pipeline.
fn right_panel_lib_area(app: &App, terminal_size: (u16, u16)) -> Option<Rect> {
    let chrome = super::draw::compute_chrome_geometry(
        app,
        0,
        terminal_size.0,
        Rect::new(0, 0, terminal_size.0, terminal_size.1),
    );
    if !chrome.right_visible {
        return None;
    }
    Some(widgets::right_panel_content_area(
        chrome.right_area,
        app.effective_panel_mode() != PanelMode::Both,
    ))
}

/// Whether the right panel is in the wide Wide hero breakpoint right
/// now, derived paint-free from the current terminal size. Replaces the
/// four deleted `is_wide_*_active()` paint-inference predicates: the
/// breakpoint (`wide_hero_fits`) is the same for every
/// Wide hero destination, so one predicate serves all of them.
pub(in crate::tui) fn is_right_panel_wide(app: &App, terminal_size: (u16, u16)) -> bool {
    right_panel_lib_area(app, terminal_size).is_some_and(wide_hero::wide_hero_fits)
}

/// The finalized library content rect when the Wide TV series Workspace
/// owns `lib_idx`, computed paint-free from the current terminal size.
/// `None` when the library is not a TV library, the breakpoint is
/// narrow, or the content mode is `Latest` or `Upcoming`. The mode is the
/// level's `tv_content_mode`, else the library's (the Upcoming level
/// stores `None`); `None` for both counts as a show mode. Computed a
/// frame earlier than a painted rect, so mount and focus do not flash
/// the narrow browser on entry.
pub(in crate::tui) fn wide_tv_show_area(
    app: &App,
    lib_idx: usize,
    terminal_size: (u16, u16),
) -> Option<Rect> {
    let collection_type = app.emby_library_collection_type(lib_idx)?;
    if collection_type != "tvshows" {
        return None;
    }
    let mode = app.emby_library_tv_content_mode(lib_idx);
    if matches!(mode, Some(TvContentMode::Latest | TvContentMode::Upcoming)) {
        return None;
    }
    let lib_area = right_panel_lib_area(app, terminal_size)?;
    wide_hero::wide_hero_fits(lib_area).then_some(lib_area)
}
