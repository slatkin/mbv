use super::super::super::palette;
use super::super::super::types_feeds_manage::{FeedFormField, FeedsManageStage};
use super::super::super::ui_util::trunc_str;
use crate::app::render::components::modal_frame::render_modal_frame;
use mbv_core::config::{FeedKind, FeedSubscription};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub(in crate::app) struct FeedsManageRenderModel<'a> {
    pub feeds: &'a [FeedSubscription],
    pub stage: &'a FeedsManageStage,
    pub cursor: usize,
    pub pending_add: Option<u64>,
}

/// Geometry painted by the feeds-manage popup, reused by its mouse
/// hit-testing (task 5.1, design.md D6).
pub(in crate::app) struct FeedsManageRenderGeometry {
    /// The painted modal rect — the outside-click boundary.
    pub frame: Rect,
    /// List stage: painted feed-row rect -> feed index.
    pub rows: Vec<(Rect, usize)>,
    /// Form stage: painted field-row rect -> field.
    pub fields: Vec<(Rect, FeedFormField)>,
}

pub(in crate::app) fn render_feeds_manage_content(
    f: &mut Frame,
    dim_backdrop_active: &mut bool,
    model: FeedsManageRenderModel<'_>,
) -> FeedsManageRenderGeometry {
    match model.stage {
        FeedsManageStage::List => render_feeds_manage_list(f, dim_backdrop_active, &model),
        FeedsManageStage::Form(form) => {
            render_feeds_manage_form(f, dim_backdrop_active, form, model.pending_add)
        }
    }
}

fn render_feeds_manage_list(
    f: &mut Frame,
    dim_backdrop_active: &mut bool,
    model: &FeedsManageRenderModel<'_>,
) -> FeedsManageRenderGeometry {
    let title = " Manage Feeds ";
    let hint = "[a]add  [↵/e]edit  [d]remove  [Esc]close";
    let width: u16 = 58;
    let content_h = (model.feeds.len().max(1) as u16) + 1;
    let height = content_h + 2;

    let inner = render_modal_frame(
        f,
        dim_backdrop_active,
        title,
        width,
        height,
        palette::SURFACE_FOCUSED,
    );

    f.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(palette::TEXT_MUTED))),
        Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        },
    );

    let list_area = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: inner.height.saturating_sub(1),
    };

    if model.feeds.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled(
                "No feed subscriptions yet -- press a to add",
                Style::default().fg(palette::TEXT_SECONDARY),
            )),
            list_area,
        );
        return FeedsManageRenderGeometry {
            frame: inner,
            rows: Vec::new(),
            fields: Vec::new(),
        };
    }
    let rows = model
        .feeds
        .iter()
        .enumerate()
        .take(list_area.height as usize)
        .map(|(i, _)| {
            (
                Rect {
                    x: list_area.x,
                    y: list_area.y + i as u16,
                    width: list_area.width,
                    height: 1,
                },
                i,
            )
        })
        .collect();

    let lines: Vec<Line> = model
        .feeds
        .iter()
        .enumerate()
        .map(|(i, sub)| {
            let focused = i == model.cursor;
            let arrow = if focused { "▸ " } else { "  " };
            let name_style = if focused {
                Style::default()
                    .fg(palette::TEXT_PRIMARY)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::TEXT_SECONDARY)
            };
            let kind_label = match sub.kind {
                FeedKind::Audio => "[audio]",
                FeedKind::Video => "[video]",
            };
            let url_max = (list_area.width as usize)
                .saturating_sub(arrow.len() + sub.name.len() + kind_label.len() + 3);
            Line::from(vec![
                Span::raw(arrow),
                Span::styled(sub.name.clone(), name_style),
                Span::raw(" "),
                Span::styled(kind_label, Style::default().fg(palette::ACCENT)),
                Span::raw(" "),
                Span::styled(
                    trunc_str(&sub.url, url_max),
                    Style::default().fg(palette::TEXT_MUTED),
                ),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(lines), list_area);
    FeedsManageRenderGeometry {
        frame: inner,
        rows,
        fields: Vec::new(),
    }
}

fn render_feeds_manage_form(
    f: &mut Frame,
    dim_backdrop_active: &mut bool,
    form: &super::super::super::types_feeds_manage::FeedForm,
    pending_add: Option<u64>,
) -> FeedsManageRenderGeometry {
    let editing = form.editing_index.is_some();
    let title = if editing { " Edit Feed " } else { " Add Feed " };
    let submitting = pending_add.is_some();

    let width = 58;
    let height = 9;
    let inner = render_modal_frame(
        f,
        dim_backdrop_active,
        title,
        width,
        height,
        palette::SURFACE_FOCUSED,
    );

    let field_style = |focused: bool| {
        if focused {
            Style::default().fg(palette::TEXT_STRONG)
        } else {
            Style::default().fg(palette::TEXT_SECONDARY)
        }
    };
    let cursor_glyph = "▏";

    let name_focused = form.focus == FeedFormField::Name;
    let name_line = format!(
        "Name: {}{}",
        form.name,
        if name_focused { cursor_glyph } else { "" }
    );
    let url_focused = !editing && form.focus == FeedFormField::Url;
    let url_line = if editing {
        format!("URL:  {} (read-only)", form.url)
    } else {
        format!(
            "URL:  {}{}",
            form.url,
            if url_focused { cursor_glyph } else { "" }
        )
    };
    let kind_focused = form.focus == FeedFormField::Kind;
    let kind_label = match form.kind {
        FeedKind::Audio => "Audio",
        FeedKind::Video => "Video",
    };
    let kind_line = format!("Kind: {kind_label}   [←/→ toggle]");

    let rows = [
        (name_line, field_style(name_focused)),
        (url_line, field_style(url_focused)),
        (kind_line, field_style(kind_focused)),
    ];
    let fields: Vec<(Rect, FeedFormField)> =
        [FeedFormField::Name, FeedFormField::Url, FeedFormField::Kind]
            .into_iter()
            .enumerate()
            .map(|(i, field)| {
                (
                    Rect {
                        x: inner.x + 1,
                        y: inner.y + 1 + i as u16,
                        width: inner.width.saturating_sub(2),
                        height: 1,
                    },
                    field,
                )
            })
            .collect();
    for (i, (text, style)) in rows.iter().enumerate() {
        f.render_widget(
            Paragraph::new(Span::styled(text.clone(), *style)),
            Rect {
                x: inner.x + 1,
                y: inner.y + 1 + i as u16,
                width: inner.width.saturating_sub(2),
                height: 1,
            },
        );
    }

    let status = if submitting {
        "Fetching feed…"
    } else {
        "Tab next field · Enter save · Esc cancel"
    };
    f.render_widget(
        Paragraph::new(Span::styled(
            status,
            Style::default().fg(palette::TEXT_MUTED),
        )),
        Rect {
            x: inner.x + 1,
            y: inner.y + 5,
            width: inner.width.saturating_sub(2),
            height: 1,
        },
    );
    FeedsManageRenderGeometry {
        frame: inner,
        rows: Vec::new(),
        fields,
    }
}
