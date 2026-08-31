use crate::app::layout::LayoutMain;
use crate::app::render::components::chrome::thin_vertical_thumb;
use crate::app::render::components::list_rows::{selection_marker, MarkerEdge};
use crate::app::render::components::widgets::render_scrollbar_with_viewport_at;
use crate::app::ui_util::*;
use crate::app::{palette, App, QueueScope, RemoteSlotState};
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::playback_queue::QueueItem;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

// Extra gap reserved between the title and whatever follows it (inline
// percent and/or the right-aligned duration), on top of their own widths
// which are already accounted for separately via `pct_w`/`right_w`.
const QUEUE_TITLE_QUIET_COLUMNS: usize = 2;

/// Time text for one queue row. The now-playing row shows the moving
/// elapsed time next to its duration (`1:05 / 3:22`), matching the playback
/// panel's time readout; every other row shows just its duration. Empty
/// when the duration is unknown.
fn queue_row_time_text(pos_ticks: i64, dur_ticks: i64, show_elapsed: bool) -> String {
    let dur_s = dur_ticks / TICKS_PER_SECOND;
    if dur_s <= 0 {
        return String::new();
    }
    if show_elapsed {
        format!(
            "{} / {}",
            fmt_duration_short(pos_ticks / TICKS_PER_SECOND),
            fmt_duration_short(dur_s)
        )
    } else {
        fmt_duration_short(dur_s)
    }
}

impl App {
    /// Renders the "Queue" title pill (and optional Local/Remote scope pills)
    /// at the top of the queue column on a single row.
    pub(super) fn render_queue_title(
        &mut self,
        f: &mut Frame,
        area: Rect,
        layout: &mut LayoutMain,
    ) {
        if area.height < 1 {
            return;
        }

        f.render_widget(
            Block::default().style(Style::default().bg(palette::SURFACE_CHROME)),
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
        let daemon_endpoint = self.config.lock().unwrap().daemon_client_endpoint.clone();
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
        let local_bg = palette::SURFACE_CHROME;
        let local_fg = palette::TEXT_FOCUS_ACCENT;
        if show_split {
            if let Some(label) = local_spans.get_mut(2) {
                label.content = " Connected: ".into();
            }
        }
        Self::set_status_pill_style(&mut local_spans, local_fg, local_bg);
        if let Some(icon) = local_spans.get_mut(1) {
            icon.style = icon.style.fg(palette::TEXT_METADATA);
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
        let remote_btn_text = format!(" {remote_icon} ");
        let remote_btn_text_w = remote_btn_text.width() as u16;
        let local_btn_w = local_btn_text.width() as u16;
        let button_pill_w = if is_mbv_session {
            (local_btn_w + remote_btn_text_w).min(area.width.saturating_sub(local_w))
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
        let target_bg = palette::SURFACE_CHROME;
        let target_fg = if is_mbv_session {
            palette::ACCENT
        } else {
            palette::TEXT_FOCUS_ACCENT
        };
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
        let target_label_text = format!("{}{}", label.trim_start(), tracking);
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
        let base_bg = palette::SURFACE_CHROME;
        let local_btn_bg = if local_selected {
            palette::ACCENT
        } else {
            palette::PILL_BG
        };
        let local_btn_fg = if local_selected {
            palette::TEXT_FOCUS_ACCENT
        } else {
            palette::PILL_FG
        };
        let remote_btn_bg = if local_selected {
            palette::PILL_BG
        } else {
            palette::ACCENT
        };
        let remote_btn_fg = if local_selected {
            palette::PILL_FG
        } else {
            palette::TEXT_FOCUS_ACCENT
        };
        let spans = vec![
            Span::styled(
                local_btn_text,
                Style::default().fg(local_btn_fg).bg(local_btn_bg),
            ),
            Span::styled(
                remote_btn_text,
                Style::default().fg(remote_btn_fg).bg(remote_btn_bg),
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
        let remote_btn_x = button_area.x + local_btn_w;
        let remote_btn_w = button_area.width.saturating_sub(local_btn_w);
        layout.queue_scope_remote_area = Rect {
            x: remote_btn_x,
            y: button_area.y,
            width: remote_btn_w,
            height: 1,
        };
    }

    /// Renders the queue list (track items, scrollbar). The title/scope pill
    /// row is rendered separately by `render_queue_title`.
    pub(super) fn render_queue(
        &mut self,
        f: &mut Frame,
        area: Rect,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        if area.height < 1 {
            return;
        }

        let (slots_snapshot, cursor) = {
            let queue = self.displayed_queue();
            (queue.slots().to_vec(), queue.queue_cursor)
        };
        let n = slots_snapshot.len();
        if n == 0 {
            f.render_widget(
                Paragraph::new(if self.visible_queue_scope() == QueueScope::Local {
                    "  Add items with p from Home or library tabs"
                } else {
                    "  Remote queue is empty"
                })
                .style(Style::default().fg(palette::TEXT_MUTED)),
                area,
            );
            return;
        }

        let playback = self.displayed_queue_playback_state();

        // Flat display rows from the canonical queue — one row per slot.
        let display = build_queue_rows(&slots_snapshot);
        let total = display.len();
        let visible = area.height as usize;

        // Visual row of the cursor item.  The cursor is a slot index.
        let cursor_row = display
            .iter()
            .position(|r| matches!(r, QueueRow::Slot { slot_idx } if *slot_idx == cursor))
            .unwrap_or(0);
        let max_offset = total.saturating_sub(visible);
        // Stateless follow-cursor viewport (split-queue-cursor-ownership D3):
        // the legacy painter derives its window fresh from the canonical
        // cursor each frame, so there is no persistent scroll owner — no
        // App-side mirror, and no state to leak across App instances, scopes,
        // or reentrant renders. The cursor is bottom-anchored in the window
        // once the list overflows, exactly as the previous stateful clamp
        // converged for the bottom-exit case, so layout geometry and visual
        // behavior are preserved.
        let offset = cursor_row
            .saturating_sub(visible.saturating_sub(1))
            .min(max_offset);
        layout.queue_selected_item_rect = Some(Rect {
            x: area.x,
            y: area.y + (cursor_row.saturating_sub(offset)) as u16,
            width: area.width,
            height: 1,
        });

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
                QueueRow::Slot { slot_idx } => {
                    let slot_idx = *slot_idx;
                    let indent: usize = 2;
                    let track_content_w = render_w.saturating_sub(2);
                    let slot = &slots_snapshot[slot_idx];
                    let is_active = playback.active && playback.active_idx == slot_idx;
                    let is_cursor = slot_idx == cursor && focused;

                    let fg = if is_cursor || focused {
                        palette::TEXT_STRONG
                    } else {
                        palette::QUEUE_ROW_FG
                    };
                    let row_style = Style::default().fg(fg);

                    match &slot.item {
                        QueueItem::Emby(item) => {
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
                            let pct_str = if item.is_audio() {
                                String::new()
                            } else {
                                fmt_playback_pct(pt, rt)
                            };

                            let dur = queue_row_time_text(pt, item.runtime_ticks, is_active);
                            let dim_color = if focused {
                                palette::TEXT_SECONDARY
                            } else {
                                palette::TEXT_MUTED
                            };

                            let dur_visible = show_length && !dur.is_empty();
                            let pct_visible = !pct_str.is_empty();
                            let pct_w = if pct_visible { 1 + pct_str.width() } else { 0 };
                            let right_w = if dur_visible { dur.width() } else { 0 };
                            let title_w = track_content_w.saturating_sub(
                                indent + pct_w + right_w + QUEUE_TITLE_QUIET_COLUMNS,
                            );
                            let title = trunc_str(&item.name, title_w);

                            if is_cursor {
                                f.render_widget(
                                    Block::default()
                                        .style(Style::default().bg(palette::SURFACE_FOCUSED)),
                                    Rect {
                                        x: area.x,
                                        y: area.y + line_offset,
                                        width: area.width,
                                        height: 1,
                                    },
                                );
                            }

                            let title_color = if is_active {
                                palette::ACCENT
                            } else if !focused {
                                dim_color
                            } else {
                                fg
                            };

                            let mut spans: Vec<Span> = Vec::new();
                            if indent > 0 {
                                spans.push(selection_marker(is_cursor, MarkerEdge::Left));
                                spans.push(Span::raw(" "));
                            }
                            let title_w_actual = title.width();
                            spans.push(Span::styled(title, Style::default().fg(title_color)));
                            if pct_visible {
                                spans.push(Span::raw(" "));
                                spans.push(Span::styled(
                                    pct_str,
                                    Style::default().fg(palette::TEXT_METADATA),
                                ));
                            }

                            if dur_visible {
                                let used = indent + title_w_actual + pct_w;
                                let pad = track_content_w.saturating_sub(used + right_w);
                                spans.push(Span::raw(" ".repeat(pad)));
                                spans.push(Span::styled(
                                    dur,
                                    Style::default().fg(palette::STATUS_AVAILABLE),
                                ));
                            }

                            list_items.push(ListItem::new(Line::from(spans)).style(row_style));
                            layout.queue_row_map.push(Some(slot_idx));
                            line_offset += 1;
                        }
                        other => {
                            let (title_raw, duration_ticks) = match other {
                                QueueItem::Feed(entry) => {
                                    (entry.title.as_str(), entry.duration_ticks.unwrap_or(0))
                                }
                                QueueItem::Audiobookshelf(ep) => {
                                    (ep.title.as_str(), ep.duration_ticks.unwrap_or(0))
                                }
                                QueueItem::AudiobookshelfBook(book) => {
                                    (book.title.as_str(), book.duration_ticks.unwrap_or(0))
                                }
                                QueueItem::Emby(_) => unreachable!(),
                            };
                            let pos_ticks = if is_active {
                                playback.position_ticks
                            } else {
                                0
                            };
                            let dur =
                                queue_row_time_text(pos_ticks, duration_ticks as i64, is_active);
                            let dim_color = if focused {
                                palette::TEXT_SECONDARY
                            } else {
                                palette::TEXT_MUTED
                            };

                            let dur_visible = show_length && !dur.is_empty();
                            let right_w = if dur_visible { dur.width() } else { 0 };
                            let title_w = track_content_w
                                .saturating_sub(indent + right_w + QUEUE_TITLE_QUIET_COLUMNS);
                            let title = trunc_str(title_raw, title_w);

                            if is_cursor {
                                f.render_widget(
                                    Block::default()
                                        .style(Style::default().bg(palette::SURFACE_FOCUSED)),
                                    Rect {
                                        x: area.x,
                                        y: area.y + line_offset,
                                        width: area.width,
                                        height: 1,
                                    },
                                );
                            }

                            let title_color = if is_active {
                                palette::ACCENT
                            } else if !focused {
                                dim_color
                            } else {
                                fg
                            };

                            let mut spans: Vec<Span> = Vec::new();
                            if indent > 0 {
                                spans.push(selection_marker(is_cursor, MarkerEdge::Left));
                                spans.push(Span::raw(" "));
                            }
                            let title_w_actual = title.width();
                            spans.push(Span::styled(title, Style::default().fg(title_color)));

                            if dur_visible {
                                let used = indent + title_w_actual;
                                let pad = track_content_w.saturating_sub(used + right_w);
                                spans.push(Span::raw(" ".repeat(pad)));
                                spans.push(Span::styled(
                                    dur,
                                    Style::default().fg(palette::STATUS_AVAILABLE),
                                ));
                            }

                            list_items.push(ListItem::new(Line::from(spans)).style(row_style));
                            layout.queue_row_map.push(Some(slot_idx));
                            line_offset += 1;
                        }
                    }
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
            render_scrollbar_with_viewport_at(
                f,
                area,
                max_off.saturating_add(visible),
                visible,
                offset,
                area.x + area.width.saturating_sub(1),
                thin_vertical_thumb(tui_scrollbar::GlyphSet::minimal()),
                palette::TEXT_EMPHASIS,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{make_app_stub, make_item};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    /// Renders `app.render_queue` into a string of buffer rows.
    fn render_queue_rows(app: &mut App) -> Vec<String> {
        let backend = TestBackend::new(40, 15);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutMain::default();
        term.draw(|f| {
            app.render_queue(f, Rect::new(0, 0, 40, 15), true, &mut layout);
        })
        .unwrap();
        let buf = term.backend().buffer();
        (0..15)
            .map(|y| (0..40).map(|x| buf[(x, y)].symbol().to_string()).collect())
            .collect()
    }

    /// Renders `app.render_queue` once and returns the painted selected-item
    /// rect's y offset within the 15-row buffer (the stateless derived
    /// viewport's cursor row). Reuses a single `Rect::new` per render so the
    /// test keeps the legacy screen's baseline count unchanged.
    fn render_queue_cursor_row(app: &mut App) -> u16 {
        let backend = TestBackend::new(40, 15);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutMain::default();
        term.draw(|f| {
            app.render_queue(f, Rect::new(0, 0, 40, 15), true, &mut layout);
        })
        .unwrap();
        layout.queue_selected_item_rect.map(|r| r.y).unwrap_or(0)
    }

    #[test]
    fn now_playing_queue_row_shows_elapsed_next_to_duration() {
        let mut app = make_app_stub();
        app.panel_focus = crate::app::PanelFocus::Queue;

        let tick = mbv_core::api::TICKS_PER_SECOND;
        let mut items = Vec::new();
        for i in 0..3 {
            let mut item = make_item(&format!("A{i}"), "Movie");
            item.id = format!("a-{i}");
            item.runtime_ticks = 3 * 60 * tick;
            items.push(item);
        }
        app.player_tab.set_items(items, 0);
        {
            let mut status = app.player.status.lock().unwrap();
            status.active = true;
            status.current_idx = 0;
            status.queue_len = 3;
            status.position_ticks = 45 * tick;
            status.runtime_ticks = 3 * 60 * tick;
        }

        let rows = render_queue_rows(&mut app);
        assert!(
            rows.iter().any(|row| row.contains("0:45 / 3:00")),
            "active row must show elapsed next to duration:\n{}",
            rows.join("\n")
        );
        // Sibling rows keep showing only their own duration.
        assert!(
            rows.iter()
                .any(|row| row.contains("3:00") && !row.contains(" / ")),
            "non-active rows must show duration without elapsed:\n{}",
            rows.join("\n")
        );
    }

    #[test]
    fn render_queue_scroll_up_reaches_top_without_regressing() {
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

        let mut prev_scroll = usize::MAX;
        for cursor in (0..n).rev() {
            app.player_tab.queue_cursor = cursor;
            // The legacy painter derives its viewport from the cursor each
            // frame (no persistent scroll owner), so the offset is exactly
            // the bottom-anchored window formula.
            let expected = cursor.saturating_sub(14).min(n.saturating_sub(15));
            assert!(
                expected <= prev_scroll,
                "scroll regressed from {prev_scroll} to {expected} at cursor {cursor}"
            );
            prev_scroll = expected;
            assert_eq!(
                render_queue_cursor_row(&mut app),
                (cursor.saturating_sub(expected)) as u16,
                "cursor row must render at its windowed offset for cursor {cursor}"
            );
        }
        assert_eq!(prev_scroll, 0);
    }

    #[test]
    fn render_queue_page_up_from_bottom_reaches_top() {
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

        let page = 14usize; // area.height - 1
        let mut prev_scroll = usize::MAX;
        let mut cursor = n - 1;
        loop {
            let expected = cursor.saturating_sub(14).min(n.saturating_sub(15));
            assert!(
                expected <= prev_scroll,
                "scroll regressed from {prev_scroll} to {expected} at cursor {cursor}"
            );
            prev_scroll = expected;
            assert_eq!(
                render_queue_cursor_row(&mut app),
                (cursor.saturating_sub(expected)) as u16,
                "cursor row must render at its windowed offset for cursor {cursor}"
            );
            if cursor == 0 {
                break;
            }
            cursor = cursor.saturating_sub(page);
            app.player_tab.queue_cursor = cursor;
        }
        assert_eq!(prev_scroll, 0);
    }

    #[test]
    fn render_queue_viewport_does_not_leak_between_app_instances() {
        // The legacy painter is stateless (split-queue-cursor-ownership D3):
        // its viewport is derived from the canonical cursor each frame, so a
        // render of one App instance must not influence another. Render a
        // bottom cursor (which would set a high viewport in a stateful
        // renderer), then a *fresh* App with a mid cursor whose derived
        // window differs from any leaked scroll: with a leaked viewport the
        // mid cursor would be clamped to the window bottom (row 0); derived,
        // it renders at its absolute windowed row.
        let mut app_a = make_app_stub();
        app_a.panel_focus = crate::app::PanelFocus::Queue;
        let items_a: Vec<_> = (0..60)
            .map(|i| {
                let mut item = make_item(&format!("A{i}"), "Audio");
                item.id = format!("a-{i}");
                item
            })
            .collect();
        app_a.player_tab.set_items(items_a, 59);
        // Cursor 59 of 60 in a 15-row window: derived offset 45, cursor row 14.
        assert_eq!(
            render_queue_cursor_row(&mut app_a),
            14,
            "bottom cursor must render at the last visible row"
        );

        let mut app_b = make_app_stub();
        app_b.panel_focus = crate::app::PanelFocus::Queue;
        let items_b: Vec<_> = (0..60)
            .map(|i| {
                let mut item = make_item(&format!("B{i}"), "Audio");
                item.id = format!("b-{i}");
                item
            })
            .collect();
        app_b.player_tab.set_items(items_b, 30);
        // Cursor 30 of 60: derived offset 16, cursor row 14. A leaked
        // viewport from app_a (45) would clamp to 30 -> row 0.
        assert_eq!(
            render_queue_cursor_row(&mut app_b),
            14,
            "fresh App must derive its viewport independently of app_a (leak would give row 0)"
        );
    }
}
