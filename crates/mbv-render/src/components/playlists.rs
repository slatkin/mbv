use super::chrome;
use super::confirm_modal::render_button_row;
use crate::components::modal_frame::render_modal_frame_bounds;
use mbv_emby_model::EmbyItem;
use mbv_theme as palette;
use mbv_ui_model::confirm::ConfirmButton;
use mbv_ui_model::ui_util::trunc_str;
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Paragraph;

/// Geometry painted by the save-playlist modal, reused by its mouse
/// hit-testing (task 5.1, design.md D6). The modal's click targets are the
/// button pills; the frame is the outside-click boundary.
#[derive(Debug)]
pub struct SavePlaylistRenderGeometry {
    pub frame: Rect,
    /// Painted button pill rects in button order (save, cancel).
    pub buttons: Vec<Rect>,
}

pub fn render_save_playlist_content(
    f: &mut Frame,
    dim_backdrop_active: &mut bool,
    input: &str,
    rename: bool,
) -> SavePlaylistRenderGeometry {
    let modal = render_modal_frame_bounds(
        f,
        dim_backdrop_active,
        60,
        7,
        palette::surface_colors(palette::Surface::PopupFrame, false).fill,
    );
    let inner = modal.inner;
    let label = "Name: ";
    let cursor = "▏";
    let max_input = inner.width as usize - label.len() - cursor.len() - 2;
    let visible: String = input
        .chars()
        .rev()
        .take(max_input)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let input_line = format!("{label}{visible}{cursor}");
    let title = if rename {
        "Rename Playlist"
    } else {
        "Save Playlist"
    };
    let input_y = inner.y + (inner.height.saturating_sub(3)) / 2;
    let buttons_y = input_y + 2;
    f.render_widget(
        Paragraph::new(Span::styled(
            title,
            Style::default()
                .fg(palette::TEXT_HERO_TITLE)
                .add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center),
        Rect {
            x: inner.x,
            y: input_y.saturating_sub(1),
            width: inner.width,
            height: 1,
        },
    );
    f.render_widget(
        Paragraph::new(Span::styled(
            input_line,
            Style::default().fg(palette::TEXT_STRONG),
        ))
        .alignment(Alignment::Center),
        Rect {
            x: inner.x,
            y: input_y,
            width: inner.width,
            height: 1,
        },
    );
    let affirmative = if rename {
        ConfirmButton::affirmative("Enter", "Rename")
    } else {
        ConfirmButton::affirmative("Enter", "Save")
    };
    let buttons = vec![affirmative, ConfirmButton::cancel("Esc", "Cancel")];
    let buttons_rect = render_button_row(f, inner, buttons_y, &buttons);
    SavePlaylistRenderGeometry {
        frame: modal.outer,
        buttons: buttons_rect,
    }
}

#[derive(Default, Debug)]
pub struct PlaylistsRenderGeometry {
    pub panel_area: Rect,
    pub content_area: Rect,
    pub playlist_rows: Vec<(Rect, usize)>,
    pub open_rows: Vec<(Rect, usize)>,
}

/// Everything the playlists painter reads and mutates, gathered from the
/// component once per frame. The painter keeps no ownership: cursors and
/// scroll offsets are written back through these borrows.
#[derive(Debug)]
pub struct PlaylistsViewState<'a> {
    pub panel_area: Option<Rect>,
    pub playlists: &'a [EmbyItem],
    pub playlists_cursor: &'a mut usize,
    pub playlists_scroll: &'a mut usize,
    pub playlists_loading: bool,
    pub playlists_open: Option<&'a EmbyItem>,
    pub open_items: &'a [EmbyItem],
    pub open_cursor: &'a mut usize,
    pub open_scroll: &'a mut usize,
    pub open_loading: bool,
    pub loaded_id: Option<&'a str>,
    pub geometry: &'a mut PlaylistsRenderGeometry,
}

pub fn render_playlists_content(frame: &mut Frame, area: Rect, view: PlaylistsViewState<'_>) {
    let PlaylistsViewState {
        panel_area,
        playlists,
        playlists_cursor,
        playlists_scroll,
        playlists_loading,
        playlists_open,
        open_items,
        open_cursor,
        open_scroll,
        open_loading,
        loaded_id,
        geometry,
    } = view;
    *geometry = PlaylistsRenderGeometry::default();
    let (title, hint) = if let Some(playlist) = playlists_open {
        (
            playlist.name.to_uppercase(),
            "[↵]play [s]shuffle [a]add [←]back [Esc]close".to_string(),
        )
    } else {
        (
            "PLAYLISTS".to_string(),
            "[↵]play [s]shuffle [a]add [→]browse [n]rename [d]delete [r]refresh [Esc]close"
                .to_string(),
        )
    };
    let panel = panel_area.unwrap_or(area);
    geometry.panel_area = panel;
    let content = chrome::render_panel_shell_at(frame, panel, &title, &hint);
    geometry.content_area = content;
    if playlists_open.is_some() {
        render_open_playlist_content(
            frame,
            content,
            open_items,
            open_cursor,
            open_scroll,
            open_loading,
            geometry,
        );
        return;
    }
    if playlists_loading && playlists.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " Loading…",
                Style::default().fg(palette::TEXT_SECONDARY),
            )),
            content,
        );
        return;
    }
    if playlists.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " No playlists found",
                Style::default().fg(palette::TEXT_SECONDARY),
            )),
            content,
        );
        return;
    }
    render_playlist_rows(
        frame,
        content,
        playlists,
        playlists_cursor,
        playlists_scroll,
        loaded_id,
        geometry,
    );
}

/// What one playlist-panel row says; `paint_rows` owns every style decision.
struct RowContent {
    lead: String,
    title: String,
    bold_title: bool,
    trail: String,
    /// Title colour when the row is not selected.
    title_fg: Color,
}

/// Paint one-line rows with the shared scroll clamp, bg/fg rule and scrollbar.
/// Returns each painted row's rect with its absolute index.
fn paint_rows(
    frame: &mut Frame,
    content: Rect,
    total: usize,
    cursor: usize,
    scroll: &mut usize,
    row_content: impl Fn(usize) -> RowContent,
) -> Vec<(Rect, usize)> {
    let height = content.height as usize;
    if cursor < *scroll {
        *scroll = cursor;
    } else if cursor >= *scroll + height {
        *scroll = cursor.saturating_add(1).saturating_sub(height);
    }
    let mut painted = Vec::new();
    for visible in 0..height.min(total.saturating_sub(*scroll)) {
        let index = *scroll + visible;
        let selected = index == cursor;
        // Keep zebra parity tied to the absolute row so bands hold still under scroll.
        let bg = if selected {
            Some(palette::SELECTED_ROW_BG)
        } else if index % 2 == 1 {
            Some(palette::PLAYLIST_STRIPE_BG)
        } else {
            None
        };
        let row = row_content(index);
        let (fg, muted_fg) = if selected {
            (palette::SELECTED_ROW_FG, palette::SELECTED_ROW_FG)
        } else {
            (row.title_fg, palette::TEXT_MUTED)
        };
        let title_style = if row.bold_title && !selected {
            Style::default().fg(fg).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(fg)
        };
        let title_width = chrome::panel_row_text_width(content.width)
            .saturating_sub(row.lead.len() + row.trail.len());
        let rect = Rect {
            x: content.x,
            y: content.y
                + u16::try_from(visible).expect("visible row is bounded by content height"),
            width: content.width,
            height: 1,
        };
        chrome::render_panel_row(
            frame,
            content.x,
            rect.y,
            content.width,
            selected,
            vec![
                Span::styled(row.lead, Style::default().fg(muted_fg)),
                Span::styled(trunc_str(&row.title, title_width), title_style),
                Span::styled(row.trail, Style::default().fg(muted_fg)),
            ],
            bg,
        );
        painted.push((rect, index));
    }
    chrome::render_sidebar_scrollbar(frame, content, total, *scroll);
    painted
}

fn render_playlist_rows(
    frame: &mut Frame,
    content: Rect,
    playlists: &[EmbyItem],
    cursor: &mut usize,
    scroll: &mut usize,
    loaded_id: Option<&str>,
    geometry: &mut PlaylistsRenderGeometry,
) {
    geometry.playlist_rows = paint_rows(frame, content, playlists.len(), *cursor, scroll, |i| {
        let playlist = &playlists[i];
        let loaded = loaded_id.is_some_and(|id| id == playlist.id);
        RowContent {
            lead: String::new(),
            title: playlist.name.clone(),
            bold_title: true,
            trail: if playlist.total_count > 0 {
                format!(" ({})", playlist.total_count)
            } else {
                String::new()
            },
            title_fg: if loaded {
                palette::PLAYLIST_LOADED_FG
            } else {
                palette::TEXT_PRIMARY
            },
        }
    });
}

fn render_open_playlist_content(
    frame: &mut Frame,
    content: Rect,
    items: &[EmbyItem],
    cursor: &mut usize,
    scroll: &mut usize,
    loading: bool,
    geometry: &mut PlaylistsRenderGeometry,
) {
    if loading && items.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " Loading…",
                Style::default().fg(palette::TEXT_SECONDARY),
            )),
            content,
        );
        return;
    }
    if items.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " Playlist is empty",
                Style::default().fg(palette::TEXT_SECONDARY),
            )),
            content,
        );
        return;
    }
    *cursor = (*cursor).min(items.len() - 1);
    geometry.open_rows = paint_rows(frame, content, items.len(), *cursor, scroll, |i| {
        RowContent {
            lead: format!("{:>2}. ", i + 1),
            title: items[i].display_name(),
            bold_title: false,
            trail: String::new(),
            title_fg: palette::TEXT_PRIMARY,
        }
    });
}
