use super::super::ui_util::*;
use crate::app::layout::LayoutMain;
use crate::app::{palette, App, QueueScope, RemoteSlotState};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

const QUEUE_TITLE_QUIET_COLUMNS: usize = 8;

impl App {
    /// Renders the "Queue" title pill (and optional Local/Remote scope pills)
    /// at the top of the queue column on a single row.
    pub(super) fn render_power_queue_title(
        &mut self,
        f: &mut Frame,
        area: Rect,
        layout: &mut LayoutMain,
    ) {
        if area.height < 1 {
            return;
        }

        f.render_widget(
            Block::default().style(Style::default().bg(palette::DARK_BG)),
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            },
        );

        layout.queue_scope_local_area = Rect::default();
        layout.queue_scope_remote_area = Rect::default();

        let remote_state = self.remote_slot_state();
        let daemon_endpoint = self
            .client
            .lock()
            .unwrap()
            .config
            .daemon_client_endpoint
            .clone();
        let local_selected = self.visible_queue_scope() == QueueScope::Local;
        let has_remote = matches!(remote_state, RemoteSlotState::DirectRemote);
        let has_attached = matches!(remote_state, RemoteSlotState::AttachedSession);
        let show_split = has_remote || has_attached;
        // DirectRemote only ever exists after a session was confirmed to be an
        // mbv/mbvd client (see `session_direct_endpoint`'s "mbv" check), so it's
        // always an mbv session; AttachedSession covers both mbv/mbvd sessions
        // not yet upgraded to a direct connection and plain Emby clients.
        let is_mbv_session = has_remote
            || (has_attached
                && self
                    .connected_session_state
                    .as_ref()
                    .is_some_and(|session| session.client.eq_ignore_ascii_case("mbv")));

        // --- Left "local" pill: display-only, no longer a scope toggle. ---
        let mut local_spans = self.remote_status_spans(crate::app::RemoteSlotState::Off, "");
        if show_split {
            if let Some(trailing) = local_spans.get_mut(3) {
                trailing.content = "".into();
            }
        }
        if self.use_nerd_fonts {
            if let Some(icon) = local_spans.get_mut(1) {
                icon.content = "\u{F0AFE}".into();
            }
        }
        let local_bg = palette::QUEUE_BUTTON_FOCUSED_BG;
        let local_fg = palette::YELLOW;
        if show_split {
            if let Some(label) = local_spans.get_mut(2) {
                label.content = " Connected: ".into();
            }
        }
        Self::set_status_pill_style(&mut local_spans, local_fg, local_bg);
        if let Some(icon) = local_spans.get_mut(1) {
            icon.style = icon.style.fg(local_fg);
        }
        Self::uppercase_status_label(&mut local_spans);

        let local_content_w: usize = local_spans.iter().map(|s| s.content.width()).sum();
        let local_w = (local_content_w as u16).min(area.width);
        let local_area = Rect {
            x: area.x,
            y: area.y,
            width: local_w,
            height: 1,
        };
        f.render_widget(
            Block::default().style(Style::default().bg(local_bg)),
            local_area,
        );
        f.render_widget(Paragraph::new(Line::from(local_spans)), local_area);

        if !show_split {
            return;
        }

        // --- Button pill: local/remote queue-scope toggle, right-aligned.
        // Only rendered for mbv/mbvd sessions -- plain Emby sessions have no
        // remote queue to switch to.
        let remote_icon = if self.use_nerd_fonts {
            "\u{f1616}"
        } else {
            "\u{1F5A7}"
        };
        let local_btn_text = " \u{2302} ";
        let divider_text = "\u{29F8}";
        let remote_btn_text = format!(" {remote_icon} ");
        let remote_btn_text_w = remote_btn_text.width() as u16;
        let divider_w = divider_text.width() as u16;
        let local_btn_w = local_btn_text.width() as u16;
        let button_pill_w = if is_mbv_session {
            (local_btn_w + divider_w + remote_btn_text_w).min(area.width.saturating_sub(local_w))
        } else {
            0
        };

        // --- Right "target" pill: display-only connected hostname/route. ---
        let target_x = area.x + local_w;
        let target_total_w = area.width.saturating_sub(local_w);
        let target_w = target_total_w.saturating_sub(button_pill_w);
        let target_area = Rect {
            x: target_x,
            y: area.y,
            width: target_w,
            height: 1,
        };
        let (_icon, label) = self.remote_icon_and_label(remote_state, &daemon_endpoint);
        let target_bg = palette::FOAM;
        let target_fg = palette::QUEUE_BUTTON_FOCUSED_BG;
        let target_label_style = Style::default()
            .fg(target_fg)
            .bg(target_bg)
            .add_modifier(Modifier::BOLD);
        let tracking = if has_attached {
            self.remote_tracker
                .as_ref()
                .map(|tracker| {
                    let state = match tracker.state() {
                        mbv_core::remote_reconciliation::TrackingState::Starting => "STARTING",
                        mbv_core::remote_reconciliation::TrackingState::Tracking => "TRACKING",
                        mbv_core::remote_reconciliation::TrackingState::Ambiguous => "AMBIGUOUS",
                        mbv_core::remote_reconciliation::TrackingState::Invalid => "INVALID",
                        mbv_core::remote_reconciliation::TrackingState::Suspended => "SUSPENDED",
                    };
                    if matches!(
                        tracker.state(),
                        mbv_core::remote_reconciliation::TrackingState::Ambiguous
                            | mbv_core::remote_reconciliation::TrackingState::Invalid
                            | mbv_core::remote_reconciliation::TrackingState::Suspended
                    ) {
                        let reason = match tracker.reason() {
                            mbv_core::remote_reconciliation::TrackingReason::DuplicateCandidates => "duplicate",
                            mbv_core::remote_reconciliation::TrackingReason::SessionUnavailable => "session unavailable",
                            mbv_core::remote_reconciliation::TrackingReason::ReturningStateRequiresReanchor => "re-anchor required",
                            _ => "sequence mismatch",
                        };
                        format!(" · {state} ({reason})")
                    } else {
                        format!(" · {state}")
                    }
                })
                .unwrap_or_default()
        } else {
            String::new()
        };
        let target_label_text = format!(" {}{}", label.trim_start(), tracking);
        f.render_widget(
            Block::default().style(Style::default().bg(target_bg)),
            target_area,
        );
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                target_label_text,
                target_label_style,
            )])),
            target_area,
        );

        if !is_mbv_session {
            return;
        }

        let button_area = Rect {
            x: target_x + target_w,
            y: area.y,
            width: button_pill_w,
            height: 1,
        };
        let base_bg = palette::QUEUE_BUTTON_FOCUSED_BG;
        let local_btn_bg = if local_selected {
            palette::QUEUE_SCOPE_BUTTON_ACTIVE_BG
        } else {
            palette::QUEUE_BUTTON_FOCUSED_BG
        };
        let remote_btn_bg = if local_selected {
            palette::QUEUE_BUTTON_FOCUSED_BG
        } else {
            palette::QUEUE_SCOPE_BUTTON_ACTIVE_BG
        };
        let spans = vec![
            Span::styled(
                local_btn_text,
                Style::default().fg(palette::YELLOW).bg(local_btn_bg),
            ),
            Span::styled(divider_text, Style::default().fg(palette::WHITE).bg(base_bg)),
            Span::styled(
                remote_btn_text,
                Style::default().fg(palette::AQUA).bg(remote_btn_bg),
            ),
        ];
        f.render_widget(
            Block::default().style(Style::default().bg(base_bg)),
            button_area,
        );
        f.render_widget(Paragraph::new(Line::from(spans)), button_area);

        let local_btn_w = local_btn_w.min(button_area.width);
        layout.queue_scope_local_area = Rect {
            x: button_area.x,
            y: button_area.y,
            width: local_btn_w,
            height: 1,
        };
        let remote_btn_x = button_area.x + local_btn_w + divider_w;
        let remote_btn_w = button_area.width.saturating_sub(local_btn_w + divider_w);
        layout.queue_scope_remote_area = Rect {
            x: remote_btn_x,
            y: button_area.y,
            width: remote_btn_w,
            height: 1,
        };
    }

    /// Renders the queue list (track items, scrollbar). The title/scope pill
    /// row is rendered separately by `render_power_queue_title`.
    pub(super) fn render_power_queue(
        &mut self,
        f: &mut Frame,
        area: Rect,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        layout.queue_cursor_screen_y = None;
        layout.queue_area = area;

        if area.height < 1 {
            return;
        }

        let (items, cursor) = {
            let queue = self.displayed_queue();
            (queue.items.clone(), queue.queue_cursor)
        };
        let n = items.len();
        if n == 0 {
            self.queue_scroll = 0;
            f.render_widget(
                Paragraph::new(if self.visible_queue_scope() == QueueScope::Local {
                    "  Add items with p from Home or library tabs"
                } else {
                    "  Remote queue is empty"
                })
                .style(Style::default().fg(palette::MUTED)),
                area,
            );
            return;
        }

        let playback = self.displayed_queue_playback_state();

        // Flat display rows, one per item — the queue list has no grouping/headers.
        let display = build_queue_rows(&items);
        let total = display.len();
        let visible = area.height as usize;

        // Visual row of the cursor item.
        let cursor_row = display
            .iter()
            .position(|r| {
                let QueueRow::Track { idx } = r;
                *idx == cursor
            })
            .unwrap_or(0);
        let max_offset = total.saturating_sub(visible);
        self.queue_scroll = self.queue_scroll.min(max_offset);
        if cursor_row < self.queue_scroll {
            self.queue_scroll = cursor_row;
        } else if cursor_row >= self.queue_scroll + visible {
            self.queue_scroll = cursor_row.saturating_sub(visible.saturating_sub(1));
        }
        let offset = self.queue_scroll;
        layout.queue_cursor_screen_y =
            Some(area.y + (cursor_row.saturating_sub(self.queue_scroll)) as u16);

        let has_sb = total > visible; // column always reserved when scrollbar would appear
        let need_sb = has_sb && focused; // scrollbar only drawn when focused
        let render_w = area.width.saturating_sub(if has_sb { 1 } else { 0 }) as usize;
        let show_length = render_w > 30;

        // Build visible ListItems and the row map simultaneously.
        let mut list_items: Vec<ListItem> = Vec::new();

        let mut line_offset: u16 = 0;

        for entry in display.iter().skip(offset) {
            if line_offset as usize >= visible {
                break;
            }
            match entry {
                QueueRow::Track { idx } => {
                    let i = *idx;
                    let indent: usize = 2;
                    let track_content_w = render_w.saturating_sub(2);
                    let item = &items[i];
                    let is_active = i == playback.active_idx && playback.active;
                    let is_cursor = i == cursor && focused;

                    let fg = if is_cursor || focused {
                        palette::WHITE
                    } else {
                        palette::QUEUE_UNFOCUSED_FG
                    };
                    let row_style = Style::default().fg(fg);

                    let (pt, rt) = if is_active {
                        let pos = if playback.position_ticks > 0 {
                            playback.position_ticks
                        } else {
                            item.playback_position_ticks
                        };
                        (pos, playback.runtime_ticks)
                    } else {
                        (item.playback_position_ticks, item.runtime_ticks)
                    };
                    let pct_str = if pt > 0 && rt > 0 && !item.is_audio() {
                        format!("{}%", pt * 100 / rt.max(1))
                    } else {
                        String::new()
                    };

                    // Show queue position (1-based) for all items, right-aligned
                    // so single-digit numbers line up with double-digit ones.
                    let queue_pos = idx + 1;
                    let num_w = items.len().to_string().len();
                    let label = format!("{:>num_w$}. {}", queue_pos, item.name);

                    let len_secs = item.runtime_ticks / TICKS_PER_SECOND;
                    let dur = if len_secs > 0 {
                        if item.is_audio() {
                            fmt_duration(len_secs)
                        } else {
                            fmt_duration_approx(len_secs)
                        }
                    } else {
                        String::new()
                    };
                    let dim_color = if focused {
                        palette::SUBTLE
                    } else {
                        palette::MUTED
                    };

                    // Title truncated to leave room for indent + right-aligned metadata.
                    let dur_visible = show_length && !dur.is_empty();
                    let pct_visible = !pct_str.is_empty();
                    let show_throbber = is_active && focused;
                    let now_playing_icon_w = if show_throbber && !pct_visible {
                        4
                    } else if show_throbber {
                        1
                    } else {
                        0
                    };
                    // When the throbber pairs with the percentage pill, it moves out of
                    // the title line and into the trailing metadata, so its width has
                    // to be reserved there instead. It sits flush against the
                    // percentage, with no gap between them.
                    let throbber_pill_w = if show_throbber && pct_visible {
                        now_playing_icon_w
                    } else {
                        0
                    };
                    // Pill indent: a space to its left (unless the throbber already
                    // fills that slot) and a space to its right, always.
                    let pill_left_indent = if pct_visible && !show_throbber { 1 } else { 0 };
                    // Two columns to the pill's right: one inside the pill, one
                    // in the row background separating it from the metadata.
                    let pill_right_indent = if pct_visible { 2 } else { 0 };
                    let metadata_w = (if dur_visible { dur.width() } else { 0 })
                        + (if pct_visible { pct_str.width() } else { 0 })
                        + pill_left_indent
                        + pill_right_indent
                        + throbber_pill_w;
                    let extra = metadata_w;
                    let title_w = track_content_w.saturating_sub(
                        indent + now_playing_icon_w + extra + QUEUE_TITLE_QUIET_COLUMNS,
                    );
                    let title = trunc_str(&label, title_w);

                    if is_cursor {
                        f.render_widget(
                            Block::default().style(Style::default().bg(palette::BG_GREEN)),
                            Rect {
                                x: area.x,
                                y: area.y + line_offset,
                                width: area.width,
                                height: 1,
                            },
                        );
                    }

                    // Inactive rows (not the now-playing item) match the
                    // dimmed index-number/duration color when the queue
                    // panel is unfocused, instead of standing out in the
                    // brighter unfocused row color.
                    let title_color = if is_active && !focused {
                        palette::AQUA
                    } else if !focused {
                        dim_color
                    } else {
                        fg
                    };

                    let mut spans: Vec<Span> = Vec::new();
                    if indent > 0 {
                        if is_cursor {
                            spans.push(Span::styled("▍", Style::default().fg(palette::AQUA)));
                            spans.push(Span::raw(" "));
                        } else {
                            spans.push(Span::raw("  "));
                        }
                    }
                    // Prefix is "{n:>w}. " — render it dim.
                    let prefix_chars = format!("{:>num_w$}. ", queue_pos).chars().count();
                    let tc = title.chars().count();
                    if tc > prefix_chars {
                        let split = title
                            .char_indices()
                            .nth(prefix_chars)
                            .map(|(i, _)| i)
                            .unwrap_or(title.len());
                        spans.push(Span::styled(
                            title[..split].to_string(),
                            Style::default().fg(dim_color),
                        ));
                        spans.push(Span::styled(
                            title[split..].to_string(),
                            Style::default().fg(title_color),
                        ));
                    } else {
                        spans.push(Span::styled(title, Style::default().fg(title_color)));
                    }
                    if show_throbber && !pct_visible {
                        spans.push(Span::raw(" "));
                        spans.push(self.now_playing_throbber_span());
                        spans.push(Span::raw(" "));
                    }
                    if pct_visible || dur_visible {
                        let used: usize = spans.iter().map(|s| s.content.as_ref().width()).sum();
                        let pad = track_content_w.saturating_sub(used + metadata_w);
                        spans.push(Span::raw(" ".repeat(pad)));
                    }
                    if pct_visible {
                        // Only the now-playing row gets the pill treatment
                        // (dark background, throbber). Other rows just show
                        // plain foam/grey text on the normal row background.
                        let pill_bg = if show_throbber {
                            Some(palette::DARK_BG)
                        } else {
                            None
                        };
                        let with_bg = |mut s: Style| {
                            if let Some(bg) = pill_bg {
                                s = s.bg(bg);
                            }
                            s
                        };
                        if show_throbber {
                            // to_symbol_span appends a trailing space; strip it so
                            // the throbber sits flush against the percentage.
                            let throbber = self.now_playing_throbber_span();
                            spans.push(Span::styled(
                                throbber.content.trim_end().to_string(),
                                with_bg(throbber.style),
                            ));
                        } else {
                            spans.push(Span::raw(" "));
                        }
                        spans.push(Span::styled(
                            pct_str,
                            with_bg(Style::default().fg(palette::FOAM)),
                        ));
                        spans.push(Span::styled(" ", with_bg(Style::default())));
                        spans.push(Span::raw(" "));
                    }
                    if dur_visible {
                        if let Some(split) = dur.rfind('′') {
                            spans.push(Span::styled(
                                dur[..split].to_string(),
                                Style::default().fg(palette::GREEN),
                            ));
                            spans.push(Span::styled(
                                "′".to_string(),
                                Style::default().fg(palette::SOFT_WHITE),
                            ));
                        } else {
                            spans.push(Span::styled(dur, Style::default().fg(palette::GREEN)));
                        }
                    }

                    list_items.push(ListItem::new(Line::from(spans)).style(row_style));
                    layout.queue_row_map.push(Some(i));
                    line_offset += 1;
                }
            }
        }

        let mut state = ListState::default();
        state.select(Some(cursor_row.saturating_sub(offset)));
        let render_area = Rect {
            width: render_w as u16,
            ..area
        };
        f.render_stateful_widget(
            List::new(list_items).highlight_style(Style::default()),
            render_area,
            &mut state,
        );

        if need_sb {
            let max_off = total.saturating_sub(visible);
            super::render_power_scrollbar(f, area, max_off, offset);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{make_app_stub, make_item};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn render_power_queue_scroll_up_reaches_top_without_regressing() {
        // Scrolling up one row at a time from the bottom must monotonically
        // approach the top of the list and land exactly on it.
        let mut app = make_app_stub();
        app.panel_focus = crate::app::PanelFocus::Queue;

        let mut items = Vec::new();
        for i in 0..60 {
            let mut item = make_item(&format!("A{i}"), "Audio");
            item.id = format!("a-{i}");
            items.push(item);
        }
        let n = items.len();
        app.player_tab.set_items(items, n - 1);

        let backend = TestBackend::new(40, 15);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutMain::default();

        let mut prev_scroll = usize::MAX;
        for cursor in (0..n).rev() {
            app.player_tab.queue_cursor = cursor;
            term.draw(|f| {
                app.render_power_queue(f, Rect::new(0, 0, 40, 15), true, &mut layout);
            })
            .unwrap();
            assert!(
                app.queue_scroll <= prev_scroll,
                "scroll regressed from {prev_scroll} to {} at cursor {cursor}",
                app.queue_scroll
            );
            prev_scroll = app.queue_scroll;
        }
        assert_eq!(app.queue_scroll, 0);
    }

    #[test]
    fn render_power_queue_page_up_from_bottom_reaches_top() {
        let mut app = make_app_stub();
        app.panel_focus = crate::app::PanelFocus::Queue;

        let mut items = Vec::new();
        for i in 0..60 {
            let mut item = make_item(&format!("A{i}"), "Audio");
            item.id = format!("a-{i}");
            items.push(item);
        }
        let n = items.len();
        app.player_tab.set_items(items, n - 1);

        let backend = TestBackend::new(40, 15);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutMain::default();
        term.draw(|f| {
            app.render_power_queue(f, Rect::new(0, 0, 40, 15), true, &mut layout);
        })
        .unwrap();

        let page = 14usize; // area.height - 1
        let mut prev_scroll = app.queue_scroll;
        loop {
            let cur = app.player_tab.queue_cursor;
            let next = cur.saturating_sub(page);
            app.player_tab.queue_cursor = next;
            term.draw(|f| {
                app.render_power_queue(f, Rect::new(0, 0, 40, 15), true, &mut layout);
            })
            .unwrap();
            assert!(
                app.queue_scroll <= prev_scroll,
                "scroll regressed from {prev_scroll} to {} at cursor {next}",
                app.queue_scroll
            );
            prev_scroll = app.queue_scroll;
            if next == 0 {
                break;
            }
        }
        assert_eq!(app.queue_scroll, 0);
    }
}
