use super::super::super::palette;
use super::super::super::App;
use super::super::super::{LibraryRoutePopup, LibraryRouteStage};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

const LOCAL_NO_ROUTE: &str = "Local (no route)";

impl App {
    pub(crate) fn open_library_routes_popup(&mut self) {
        let client = self.client.lock().unwrap();
        let all = client.get_views().unwrap_or_default();
        let routes = client.config.library_routes.clone();
        let items: Vec<(String, String, Option<String>)> = all
            .iter()
            .filter(|v| v.collection_type != "playlists")
            .map(|v| {
                let lower = v.name.to_lowercase();
                let assigned = routes.get(&lower).cloned();
                (lower, v.name.clone(), assigned)
            })
            .collect();
        drop(client);
        self.library_routes_popup = Some(LibraryRoutePopup {
            stage: LibraryRouteStage::PickLibrary { items },
            cursor: 0,
        });
    }

    fn enter_device_stage(&mut self, library_lower: String, library_display: String) {
        let sessions = self.fetch_sessions_blocking().unwrap_or_default();
        let local_device_name = self.client.lock().unwrap().device_name.clone();
        let mut devices: Vec<String> = sessions
            .iter()
            .filter(|s| s.client.eq_ignore_ascii_case("mbv"))
            .filter(|s| !s.device_name.eq_ignore_ascii_case(&local_device_name))
            .map(|s| s.device_name.clone())
            .collect();
        devices.sort();
        devices.dedup_by(|a, b| a.eq_ignore_ascii_case(b));

        let current = self
            .client
            .lock()
            .unwrap()
            .config
            .library_routes
            .get(&library_lower)
            .cloned();
        let cursor = current
            .as_ref()
            .and_then(|dev| devices.iter().position(|d| d.eq_ignore_ascii_case(dev)))
            .map(|idx| idx + 1) // +1 for the synthetic "Local (no route)" row at index 0
            .unwrap_or(0);

        if let Some(popup) = &mut self.library_routes_popup {
            popup.stage = LibraryRouteStage::PickDevice {
                library_lower,
                library_display,
                devices,
            };
            popup.cursor = cursor;
        }
    }

    fn commit_device_selection(&mut self) {
        let Some(popup) = &self.library_routes_popup else {
            return;
        };
        let LibraryRouteStage::PickDevice {
            library_lower,
            library_display,
            devices,
        } = popup.stage.clone()
        else {
            return;
        };
        let cursor = popup.cursor;

        {
            let mut c = self.client.lock().unwrap();
            if cursor == 0 {
                c.config.library_routes.remove(&library_lower);
            } else if let Some(device) = devices.get(cursor - 1) {
                c.config
                    .library_routes
                    .insert(library_lower.clone(), device.clone());
            }
        }
        let cfg = self.client.lock().unwrap().config.clone();
        // Keep the App's own resolution-time copy (`self.library_routes`,
        // read by `resolve_route_for_library` in library_route.rs) in sync
        // with the just-edited config -- otherwise the change wouldn't take
        // effect until the next app restart, exactly like `MultiSelectKind`'s
        // hidden_libraries/hidden_latest mirrors in multiselect.rs.
        self.library_routes = cfg.library_routes.clone();
        crate::config::save_config_settings(&cfg);

        // Return to the library list, refreshed with the new assignment.
        let all = self.client.lock().unwrap().get_views().unwrap_or_default();
        let routes = cfg.library_routes.clone();
        let items: Vec<(String, String, Option<String>)> = all
            .iter()
            .filter(|v| v.collection_type != "playlists")
            .map(|v| {
                let lower = v.name.to_lowercase();
                let assigned = routes.get(&lower).cloned();
                (lower, v.name.clone(), assigned)
            })
            .collect();
        let restored_cursor = items
            .iter()
            .position(|(lower, _, _)| *lower == library_lower)
            .unwrap_or(0);
        if let Some(popup) = &mut self.library_routes_popup {
            popup.stage = LibraryRouteStage::PickLibrary { items };
            popup.cursor = restored_cursor;
        }
        let _ = library_display; // display name not needed after commit; kept for stage symmetry
    }

    pub(crate) fn handle_library_routes_enter(&mut self) {
        let Some(popup) = &self.library_routes_popup else {
            return;
        };
        match popup.stage.clone() {
            LibraryRouteStage::PickLibrary { items } => {
                if let Some((lower, display, _)) = items.get(popup.cursor) {
                    let lower = lower.clone();
                    let display = display.clone();
                    self.enter_device_stage(lower, display);
                }
            }
            LibraryRouteStage::PickDevice { .. } => {
                self.commit_device_selection();
            }
        }
    }

    pub(crate) fn handle_library_routes_esc(&mut self) {
        let Some(popup) = &self.library_routes_popup else {
            return;
        };
        match &popup.stage {
            LibraryRouteStage::PickLibrary { .. } => {
                self.library_routes_popup = None;
            }
            LibraryRouteStage::PickDevice { .. } => {
                self.open_library_routes_popup();
            }
        }
    }

    pub(crate) fn move_library_routes_cursor(&mut self, delta: i64) {
        let Some(popup) = &mut self.library_routes_popup else {
            return;
        };
        let len = match &popup.stage {
            LibraryRouteStage::PickLibrary { items } => items.len(),
            LibraryRouteStage::PickDevice { devices, .. } => devices.len() + 1,
        };
        if len == 0 {
            return;
        }
        let mut idx = popup.cursor as i64 + delta;
        if idx < 0 {
            idx = 0;
        }
        if idx as usize >= len {
            idx = len as i64 - 1;
        }
        popup.cursor = idx as usize;
    }

    pub(in crate::app::render) fn render_library_routes_popup(&mut self, f: &mut Frame) {
        let Some(ref popup) = self.library_routes_popup else {
            return;
        };
        let (title, lines): (&str, Vec<Line>) = match &popup.stage {
            LibraryRouteStage::PickLibrary { items } => {
                let lines = items
                    .iter()
                    .enumerate()
                    .map(|(i, (_, name, assigned))| {
                        let focused = i == popup.cursor;
                        let arrow = if focused { "▸ " } else { "  " };
                        let name_style = if focused {
                            Style::default().fg(palette::TEXT)
                        } else {
                            Style::default().fg(palette::SUBTLE)
                        };
                        let value = assigned.clone().unwrap_or_else(|| "none".to_string());
                        Line::from(vec![
                            Span::raw(arrow),
                            Span::styled(name.clone(), name_style),
                            Span::raw(" -> "),
                            Span::styled(value, Style::default().fg(palette::FOAM)),
                        ])
                    })
                    .collect();
                (" Library Routes ", lines)
            }
            LibraryRouteStage::PickDevice {
                library_display,
                devices,
                ..
            } => {
                let mut lines = vec![];
                if devices.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "No other mbv devices found right now -- make sure the",
                        Style::default().fg(palette::MUTED),
                    )));
                    lines.push(Line::from(Span::styled(
                        "target is running and connected.",
                        Style::default().fg(palette::MUTED),
                    )));
                }
                let mut rows: Vec<String> = vec![LOCAL_NO_ROUTE.to_string()];
                rows.extend(devices.iter().cloned());
                for (i, name) in rows.iter().enumerate() {
                    let focused = i == popup.cursor;
                    let arrow = if focused { "▸ " } else { "  " };
                    let name_style = if focused {
                        Style::default().fg(palette::TEXT)
                    } else {
                        Style::default().fg(palette::SUBTLE)
                    };
                    lines.push(Line::from(vec![
                        Span::raw(arrow),
                        Span::styled(name.clone(), name_style),
                    ]));
                }
                let _ = library_display;
                (" Pick Device ", lines)
            }
        };

        let max_w = lines.iter().map(|l| l.width()).max().unwrap_or(0);
        let inner_w = ((max_w + 6) as u16).clamp(36, 60);
        let width = inner_w + 2;
        let content_h = lines.len() as u16 + 1;
        let area = f.area();
        let height = (content_h + 2).min(area.height.saturating_sub(2));
        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(height) / 2;
        let rect = Rect {
            x,
            y,
            width,
            height,
        };

        f.render_widget(Clear, rect);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::IRIS))
            .title(Span::styled(
                title,
                Style::default()
                    .fg(palette::WHITE)
                    .add_modifier(Modifier::BOLD),
            ));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let hint = "Enter select  ·  Esc back/close";
        f.render_widget(
            Paragraph::new(Span::styled(hint, Style::default().fg(palette::MUTED))),
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
        f.render_widget(Paragraph::new(lines), list_area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{make_app_stub, make_session};
    use crate::app::{SESSIONS_LOAD_OVERRIDE, SESSIONS_LOAD_TEST_LOCK};

    #[test]
    fn open_library_routes_popup_starts_on_pick_library_stage() {
        let mut app = make_app_stub();
        app.open_library_routes_popup();
        let popup = app.library_routes_popup.as_ref().unwrap();
        assert!(matches!(popup.stage, LibraryRouteStage::PickLibrary { .. }));
    }

    #[test]
    fn commit_device_selection_assigns_library_route() {
        let _guard = crate::config::TestStateDirGuard::new();
        let _sessions_guard = SESSIONS_LOAD_TEST_LOCK.lock().unwrap();
        fn fake_sessions(
            _client: &mbv_core::api::EmbyClient,
        ) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
            Ok(vec![make_session("living-room-pc", "mbv")])
        }
        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = Some(fake_sessions);

        let mut app = make_app_stub();
        app.library_routes_popup = Some(LibraryRoutePopup {
            stage: LibraryRouteStage::PickDevice {
                library_lower: "music".to_string(),
                library_display: "Music".to_string(),
                devices: vec!["living-room-pc".to_string()],
            },
            cursor: 1, // index 0 is "Local (no route)"; 1 is the device
        });

        app.handle_library_routes_enter();

        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = None;
        assert_eq!(
            app.client
                .lock()
                .unwrap()
                .config
                .library_routes
                .get("music"),
            Some(&"living-room-pc".to_string())
        );
        assert_eq!(
            app.library_routes.get("music"),
            Some(&"living-room-pc".to_string())
        );
    }

    #[test]
    fn commit_device_selection_clears_route_on_local_no_route() {
        let mut app = make_app_stub();
        app.client
            .lock()
            .unwrap()
            .config
            .library_routes
            .insert("music".to_string(), "living-room-pc".to_string());
        app.library_routes
            .insert("music".to_string(), "living-room-pc".to_string());
        app.library_routes_popup = Some(LibraryRoutePopup {
            stage: LibraryRouteStage::PickDevice {
                library_lower: "music".to_string(),
                library_display: "Music".to_string(),
                devices: vec!["living-room-pc".to_string()],
            },
            cursor: 0, // "Local (no route)"
        });

        app.handle_library_routes_enter();

        assert_eq!(
            app.client
                .lock()
                .unwrap()
                .config
                .library_routes
                .get("music"),
            None
        );
        assert_eq!(app.library_routes.get("music"), None);
    }
}
