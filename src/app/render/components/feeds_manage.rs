use super::super::super::palette;
use super::super::super::types_feeds_manage::{FeedFormField, FeedsManageStage};
use super::super::super::ui_util::trunc_str;
use super::super::super::App;
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

pub(in crate::app) fn render_feeds_manage_content(
    f: &mut Frame,
    dim_backdrop_active: &mut bool,
    model: FeedsManageRenderModel<'_>,
) {
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
) {
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
        return;
    }

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
}

fn render_feeds_manage_form(
    f: &mut Frame,
    dim_backdrop_active: &mut bool,
    form: &super::super::super::types_feeds_manage::FeedForm,
    pending_add: Option<u64>,
) {
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
}

impl App {
    pub(in crate::app::render) fn render_feeds_manage_popup(&mut self, f: &mut Frame) {
        let Some(popup) = self.feeds_manage_popup.as_ref() else {
            return;
        };
        let feeds = self.config.lock().unwrap().feeds.clone();
        render_feeds_manage_content(
            f,
            &mut self.dim_backdrop_active,
            FeedsManageRenderModel {
                feeds: &feeds,
                stage: &popup.stage,
                cursor: popup.cursor,
                pending_add: popup.pending_add,
            },
        );
    }
}
