use crate::app::components::settings::{ServiceRow, SettingsRow, SetupDraft};
use crate::app::palette;
use crate::app::types_settings::SettingsDestination;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

#[derive(Default)]
pub(in crate::app) struct SettingsRenderGeometry {
    pub panel_area: Rect,
    pub content_area: Rect,
    pub cursor_lines: Vec<usize>,
}

pub(in crate::app) struct SettingsRenderModel<'a> {
    pub destination: SettingsDestination,
    pub rows: &'a [SettingsRow],
    pub services: &'a [ServiceRow],
    pub setup: Option<&'a SetupDraft>,
    pub cursor: usize,
    pub services_cursor: usize,
    pub scroll: usize,
}

pub(in crate::app) fn render_settings_content(
    frame: &mut Frame,
    area: Rect,
    model: SettingsRenderModel<'_>,
    geometry: &mut SettingsRenderGeometry,
) {
    let panel_area = area;
    geometry.panel_area = panel_area;
    let content = crate::app::render::render_panel_shell_at(
        frame,
        panel_area,
        match model.setup {
            Some(SetupDraft::Emby { .. }) => "EMBY SETUP",
            Some(SetupDraft::Audiobookshelf { .. }) => "AUDIOBOOKSHELF SETUP",
            None if model.destination == SettingsDestination::Services => "SERVICES",
            None if model.destination == SettingsDestination::Keys => "KEYS",
            None => "SETTINGS",
        },
        if model.setup.is_some() {
            "[↵]submit [Esc]back"
        } else if model.destination == SettingsDestination::Services {
            "[↵]select [Esc]back"
        } else if model.destination == SettingsDestination::Keys {
            // Read-only destination (design D7): nothing to activate.
            "[↑↓]browse [Esc]back"
        } else {
            "[Space]toggle [Esc]close"
        },
    );
    geometry.content_area = content;
    geometry.cursor_lines.clear();
    match model.setup {
        Some(setup) => render_setup(frame, content, setup),
        None if model.destination == SettingsDestination::Services => {
            geometry.cursor_lines.extend(0..model.services.len());
            let lines = model
                .services
                .iter()
                .enumerate()
                .map(|(index, row)| {
                    let focused = index == model.services_cursor;
                    Line::from(vec![
                        Span::raw(if focused { "▸ " } else { "  " }),
                        Span::styled(
                            row.name.clone(),
                            if focused {
                                Style::default()
                                    .fg(palette::TEXT_PRIMARY)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(palette::TEXT_SECONDARY)
                            },
                        ),
                        Span::raw("  "),
                        Span::styled(
                            row.detail.clone(),
                            Style::default().fg(if row.muted {
                                palette::TEXT_MUTED
                            } else {
                                palette::ACCENT
                            }),
                        ),
                    ])
                })
                .collect::<Vec<_>>();
            frame.render_widget(Paragraph::new(lines), content);
        }
        None => {
            let mut lines: Vec<Line> = Vec::new();
            // Zebra over the data rows in the playlists-list style, opening
            // on the panel fill; section headers and the blank group
            // spacers keep the fill. Stripes ride on the rows, so they hold
            // still under Paragraph scroll without offset math.
            let mut striped = false;
            for row in model.rows {
                if let Some(cursor) = row.cursor {
                    if geometry.cursor_lines.len() <= cursor {
                        geometry.cursor_lines.resize(cursor + 1, 0);
                    }
                    geometry.cursor_lines[cursor] = lines.len();
                }
                if row.section {
                    if !lines.is_empty() {
                        lines.push(Line::from(""));
                    }
                    lines.push(Line::from(vec![
                        Span::raw(""),
                        Span::styled(
                            row.label.clone(),
                            Style::default()
                                .fg(palette::TEXT_METADATA)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                } else {
                    let focused = row.cursor == Some(model.cursor);
                    let value_width = (content.width as usize).saturating_sub(row.label.len());
                    let mut spans = vec![
                        Span::styled(
                            row.label.clone(),
                            if focused {
                                Style::default().fg(palette::TEXT_PRIMARY)
                            } else {
                                Style::default().fg(palette::PLAYBACK_META_FG)
                            },
                        ),
                        Span::styled(
                            format!("{:>width$}", row.value, width = value_width),
                            Style::default().fg(palette::ACCENT),
                        ),
                    ];
                    // A Paragraph line style leaves trailing cells on the
                    // widget fill, so the stripe rides the spans with a pad
                    // out to the content edge instead.
                    if striped {
                        for span in spans.iter_mut() {
                            span.style = span.style.bg(palette::SETTINGS_STRIPE_BG);
                        }
                        let used: usize = spans.iter().map(|span| span.content.width()).sum();
                        let pad = (content.width as usize).saturating_sub(used);
                        if pad > 0 {
                            spans.push(Span::styled(
                                " ".repeat(pad),
                                Style::default().bg(palette::SETTINGS_STRIPE_BG),
                            ));
                        }
                    }
                    striped = !striped;
                    lines.push(Line::from(spans));
                }
            }
            let document = lines.len();
            frame.render_widget(
                Paragraph::new(lines).scroll((model.scroll as u16, 0)),
                content,
            );
            crate::app::render::render_sidebar_scrollbar(frame, content, document, model.scroll);
        }
    }
}

fn render_setup(frame: &mut Frame, content: Rect, setup: &SetupDraft) {
    let (labels, fields, focus, busy, error) = match setup {
        SetupDraft::Emby {
            fields,
            focus,
            busy,
            error,
        } => (
            &["Server URL", "Username", "Password"][..],
            fields.as_slice(),
            *focus,
            *busy,
            error,
        ),
        SetupDraft::Audiobookshelf {
            fields,
            focus,
            busy,
            error,
        } => (
            &["Server URL", "API key"][..],
            fields.as_slice(),
            *focus,
            *busy,
            error,
        ),
    };
    let mut lines = Vec::with_capacity(labels.len() * 2 + 2);
    for (index, label) in labels.iter().enumerate() {
        let focused = focus == index;
        lines.push(Line::from(Span::styled(
            *label,
            Style::default()
                .fg(if focused {
                    palette::TEXT_METADATA
                } else {
                    palette::TEXT_SECONDARY
                })
                .add_modifier(if focused {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        )));
        let value = if (labels.len() == 3 && index == 2) || (labels.len() == 2 && index == 1) {
            "•".repeat(fields[index].chars().count())
        } else {
            fields[index].clone()
        };
        lines.push(Line::from(Span::styled(
            format!("  {value}{}", if focused && !busy { "▏" } else { "" }),
            Style::default().fg(palette::TEXT_PRIMARY),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        if busy { "Working…" } else { error },
        Style::default().fg(if busy {
            palette::TEXT_MUTED
        } else {
            palette::STATUS_ERROR
        }),
    )));
    frame.render_widget(Paragraph::new(lines), content);
}
