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
        let mut devices: Vec<(String, Option<mbv_core::remote_player::DaemonEndpoint>)> = sessions
            .iter()
            .filter(|s| s.client.eq_ignore_ascii_case("mbv"))
            .filter(|s| !s.device_name.eq_ignore_ascii_case(&local_device_name))
            .map(|s| {
                // A live mbv session that doesn't yield a resolvable
                // endpoint (e.g. no advertised direct-connect port, or
                // an unparseable host) is kept in the list, paired
                // with None, rather than dropped (#256): omitting it
                // entirely would leave a device the user can see live
                // in F3's Sessions panel silently missing here with no
                // way to tell why. `render_library_routes_popup`
                // renders a `None` entry greyed out with a reason, and
                // `commit_device_selection` refuses to commit it.
                (s.device_name.clone(), self.session_direct_endpoint(s))
            })
            .collect();
        devices.sort_by(|a, b| a.0.cmp(&b.0));
        devices.dedup_by(|a, b| a.0.eq_ignore_ascii_case(&b.0));

        // Preselect by resolved endpoint, not by name (#256): a hostname
        // is more likely to change than the address it currently resolves
        // to, and this comparison is free -- `devices` above already paid
        // for the live session fetch this stage needs regardless, to let
        // the user pick a *new* device.
        let current_endpoint = self
            .client
            .lock()
            .unwrap()
            .config
            .library_routes
            .get(&library_lower)
            .and_then(|raw| mbv_core::remote_player::DaemonEndpoint::parse(raw).ok());
        let cursor = current_endpoint
            .and_then(|current| {
                devices
                    .iter()
                    .position(|(_, ep)| ep.as_ref() == Some(&current))
            })
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

        if cursor > 0 {
            if let Some((name, None)) = devices.get(cursor - 1) {
                // #256: a device shown in this picker without a
                // resolvable endpoint (greyed out, see enter_device_stage)
                // can't be committed -- there is nothing meaningful to
                // write to config for it. Flash the reason and stay on
                // this stage rather than silently doing nothing.
                self.flash_status(format!(
                    "{name} is not currently routable (no resolvable direct-connect endpoint)"
                ));
                return;
            }
        }

        {
            let mut c = self.client.lock().unwrap();
            if cursor == 0 {
                c.config.library_routes.remove(&library_lower);
            } else if let Some((_, Some(endpoint))) = devices.get(cursor - 1) {
                // #256: persist the resolved endpoint, never the device
                // name -- the name was only ever needed to let the user
                // pick a device in this dialog.
                c.config
                    .library_routes
                    .insert(library_lower.clone(), endpoint.to_string());
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
                // (label, routable) -- a device without a resolvable
                // endpoint (#256) is shown greyed out with its reason
                // appended, rather than omitted, so a device visible in
                // F3 but not currently pickable here isn't a silent
                // mystery. It stays visible via arrow-key navigation but
                // `commit_device_selection` refuses to commit it.
                let mut rows: Vec<(String, bool)> = vec![(LOCAL_NO_ROUTE.to_string(), true)];
                rows.extend(devices.iter().map(|(name, endpoint)| {
                    if endpoint.is_some() {
                        (name.clone(), true)
                    } else {
                        (format!("{name} (not currently routable)"), false)
                    }
                }));
                for (i, (label, routable)) in rows.iter().enumerate() {
                    let focused = i == popup.cursor;
                    let arrow = if focused { "▸ " } else { "  " };
                    let name_style = if !routable {
                        Style::default().fg(palette::MUTED)
                    } else if focused {
                        Style::default().fg(palette::TEXT)
                    } else {
                        Style::default().fg(palette::SUBTLE)
                    };
                    lines.push(Line::from(vec![
                        Span::raw(arrow),
                        Span::styled(label.clone(), name_style),
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
    fn commit_device_selection_assigns_library_route_as_an_endpoint() {
        // #256: the config value committed here must be the device's
        // resolved endpoint, never its name.
        let mut app = make_app_stub();
        let endpoint =
            mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:9000".parse().unwrap());
        app.library_routes_popup = Some(LibraryRoutePopup {
            stage: LibraryRouteStage::PickDevice {
                library_lower: "music".to_string(),
                library_display: "Music".to_string(),
                devices: vec![("living-room-pc".to_string(), Some(endpoint.clone()))],
            },
            cursor: 1, // index 0 is "Local (no route)"; 1 is the device
        });

        app.handle_library_routes_enter();

        assert_eq!(
            app.client
                .lock()
                .unwrap()
                .config
                .library_routes
                .get("music"),
            Some(&endpoint.to_string())
        );
        assert_eq!(app.library_routes.get("music"), Some(&endpoint.to_string()));
    }

    #[test]
    fn commit_device_selection_clears_route_on_local_no_route() {
        let mut app = make_app_stub();
        app.client
            .lock()
            .unwrap()
            .config
            .library_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
        app.library_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
        app.library_routes_popup = Some(LibraryRoutePopup {
            stage: LibraryRouteStage::PickDevice {
                library_lower: "music".to_string(),
                library_display: "Music".to_string(),
                devices: vec![(
                    "living-room-pc".to_string(),
                    Some(mbv_core::remote_player::DaemonEndpoint::Tcp(
                        "127.0.0.1:9000".parse().unwrap(),
                    )),
                )],
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

    #[test]
    fn enter_device_stage_preselects_by_resolved_endpoint_not_name() {
        // #256: preselecting which picker row matches the current
        // assignment must compare resolved endpoints, not device names --
        // endpoints are the stable identifier here (a hostname is more
        // likely to change than the address it currently resolves to).
        let _guard = crate::config::TestStateDirGuard::new();
        let _sessions_guard = SESSIONS_LOAD_TEST_LOCK.lock().unwrap();
        fn fake_sessions(
            _client: &mbv_core::api::EmbyClient,
        ) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
            let mut sess = make_session("living-room-pc", "mbv");
            sess.host = "127.0.0.1".into();
            sess.supported_commands = vec![mbv_core::api::mbv_direct_tcp_port_command(9000)];
            Ok(vec![sess])
        }
        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = Some(fake_sessions);

        let mut app = make_app_stub();
        app.client
            .lock()
            .unwrap()
            .config
            .library_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
        // enter_device_stage directly, rather than through
        // open_library_routes_popup -> handle_library_routes_enter: the
        // latter round-trips through client.get_views(), a live network
        // call that has nothing to do with what this test verifies
        // (enter_device_stage's endpoint-based preselection).
        app.library_routes_popup = Some(LibraryRoutePopup {
            stage: LibraryRouteStage::PickLibrary { items: vec![] },
            cursor: 0,
        });
        app.enter_device_stage("music".to_string(), "Music".to_string());

        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = None;
        let popup = app.library_routes_popup.as_ref().unwrap();
        assert_eq!(popup.cursor, 1); // 0 = "Local (no route)", 1 = the matched device
    }

    #[test]
    fn enter_device_stage_lists_an_unresolvable_device_instead_of_omitting_it() {
        // #256: a live "mbv" session that session_direct_endpoint can't
        // resolve to an endpoint (here: no advertised direct-connect port)
        // must still show up in the picker, paired with `None` -- silently
        // omitting it would leave a device visible in F3's Sessions panel
        // with no explanation for why it doesn't appear here.
        let _guard = crate::config::TestStateDirGuard::new();
        let _sessions_guard = SESSIONS_LOAD_TEST_LOCK.lock().unwrap();
        fn fake_sessions(
            _client: &mbv_core::api::EmbyClient,
        ) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
            // No supported_commands entry -> parse_mbv_direct_tcp_port
            // finds nothing -> session_direct_endpoint returns None.
            Ok(vec![make_session("no-port-device", "mbv")])
        }
        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = Some(fake_sessions);

        let mut app = make_app_stub();
        // enter_device_stage directly -- see the comment in
        // enter_device_stage_preselects_by_resolved_endpoint_not_name for
        // why this bypasses open_library_routes_popup.
        app.library_routes_popup = Some(LibraryRoutePopup {
            stage: LibraryRouteStage::PickLibrary { items: vec![] },
            cursor: 0,
        });
        app.enter_device_stage("music".to_string(), "Music".to_string());

        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = None;
        let popup = app.library_routes_popup.as_ref().unwrap();
        let LibraryRouteStage::PickDevice { devices, .. } = &popup.stage else {
            panic!("expected PickDevice stage");
        };
        assert_eq!(devices, &vec![("no-port-device".to_string(), None)]);
    }

    #[test]
    fn commit_device_selection_flashes_and_does_not_commit_for_an_unroutable_device() {
        // #256: selecting a greyed-out (None-endpoint) row must not write
        // anything to config -- there is nothing meaningful to write --
        // and must tell the user why, rather than silently doing nothing.
        let mut app = make_app_stub();
        app.library_routes_popup = Some(LibraryRoutePopup {
            stage: LibraryRouteStage::PickDevice {
                library_lower: "music".to_string(),
                library_display: "Music".to_string(),
                devices: vec![("no-port-device".to_string(), None)],
            },
            cursor: 1, // index 0 is "Local (no route)"; 1 is the device
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
        assert!(app.status.contains("no-port-device"));
        assert!(app.status.contains("not currently routable"));
        // Still on the PickDevice stage -- a no-op, not silently
        // reverting to the library list either.
        assert!(matches!(
            app.library_routes_popup.as_ref().unwrap().stage,
            LibraryRouteStage::PickDevice { .. }
        ));
    }
}
