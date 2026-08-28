use super::super::super::palette;
#[cfg(test)]
use super::super::super::LibraryRoutePopup;
use super::super::super::LibraryRouteStage;
use crate::app::render::components::modal_frame::render_modal_frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

const LOCAL_NO_ROUTE: &str = "Local (no route)";

#[cfg(test)]
thread_local! {
    static ROUTE_CONFIG_SAVE_CAPTURE: std::cell::RefCell<Vec<crate::config::Config>> = const { std::cell::RefCell::new(Vec::new()) };
    static ROUTE_CONFIG_SAVE_RESULT: std::cell::RefCell<Result<(), String>> = const { std::cell::RefCell::new(Ok(())) };
}

#[cfg(test)]
struct RouteConfigSaveResultGuard(Result<(), String>);

#[cfg(test)]
impl RouteConfigSaveResultGuard {
    fn set(result: Result<(), String>) -> Self {
        let previous = ROUTE_CONFIG_SAVE_RESULT.with(|current| current.replace(result));
        Self(previous)
    }
}

#[cfg(test)]
impl Drop for RouteConfigSaveResultGuard {
    fn drop(&mut self) {
        ROUTE_CONFIG_SAVE_RESULT.with(|current| {
            let _ = current.replace(self.0.clone());
        });
    }
}

#[cfg(not(test))]
pub(in crate::app) fn save_route_config(cfg: &crate::config::Config) -> Result<(), String> {
    crate::config::save_config_settings(cfg)
}

#[cfg(test)]
pub(in crate::app) fn save_route_config(cfg: &crate::config::Config) -> Result<(), String> {
    ROUTE_CONFIG_SAVE_CAPTURE.with(|captured| captured.borrow_mut().push(cfg.clone()));
    ROUTE_CONFIG_SAVE_RESULT.with(|result| result.borrow().clone())
}

pub(in crate::app) struct LibraryRoutesRenderModel<'a> {
    pub stage: &'a LibraryRouteStage,
    pub cursor: usize,
}

pub(in crate::app) fn render_library_routes_content(
    f: &mut Frame,
    dim_backdrop_active: &mut bool,
    model: LibraryRoutesRenderModel<'_>,
) {
    let (title, lines): (&str, Vec<Line>) = match model.stage {
        LibraryRouteStage::PickLibrary { items } => {
            let lines = items
                .iter()
                .enumerate()
                .map(|(i, (_, name, assigned))| {
                    let focused = i == model.cursor;
                    let arrow = if focused { "▸ " } else { "  " };
                    let name_style = if focused {
                        Style::default().fg(palette::TEXT_PRIMARY)
                    } else {
                        Style::default().fg(palette::TEXT_SECONDARY)
                    };
                    let value = assigned.clone().unwrap_or_else(|| "none".to_string());
                    Line::from(vec![
                        Span::raw(arrow),
                        Span::styled(name.clone(), name_style),
                        Span::raw(" -> "),
                        Span::styled(value, Style::default().fg(palette::TEXT_ACCENT_MUTED)),
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
                    Style::default().fg(palette::TEXT_MUTED),
                )));
                lines.push(Line::from(Span::styled(
                    "target is running and connected.",
                    Style::default().fg(palette::TEXT_MUTED),
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
                let focused = i == model.cursor;
                let arrow = if focused { "▸ " } else { "  " };
                let name_style = if !routable {
                    Style::default().fg(palette::TEXT_MUTED)
                } else if focused {
                    Style::default().fg(palette::TEXT_PRIMARY)
                } else {
                    Style::default().fg(palette::TEXT_SECONDARY)
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
    let height = content_h + 2;

    let inner = render_modal_frame(
        f,
        dim_backdrop_active,
        title,
        width,
        height,
        palette::SURFACE_FOCUSED,
    );

    let hint = "Enter select  ·  Esc back/close";
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
    f.render_widget(Paragraph::new(lines), list_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::{ComponentId, LibraryRoutesComponent, PopupId};
    use crate::app::tests::make_app_stub;
    use crate::app::{Model, SESSIONS_LOAD_OVERRIDE, SESSIONS_LOAD_TEST_LOCK};
    use mbv_core::remote_player::DaemonEndpoint;
    use tuirealm::component::AppComponent;

    fn library_routes_id() -> ComponentId {
        ComponentId::Popup(PopupId::LibraryRoutes)
    }

    fn mount_library_routes(model: &mut Model, popup: LibraryRoutePopup) {
        let id = library_routes_id();
        model
            .application
            .mount(id.clone(), Box::new(LibraryRoutesComponent::new()), vec![])
            .expect("mount LibraryRoutes");
        model
            .application
            .active(&id)
            .expect("activate LibraryRoutes");
        if let Some(comp) = model.application.get_component_mut(&id) {
            if let Some(routes) = comp.as_any_mut().downcast_mut::<LibraryRoutesComponent>() {
                routes.set_content(&popup);
            }
        }
    }

    fn library_routes_stage(model: &Model) -> Option<LibraryRouteStage> {
        model
            .application
            .get_component(&library_routes_id())
            .and_then(|c| c.as_any().downcast_ref::<LibraryRoutesComponent>())
            .and_then(|c| c.stage())
            .cloned()
    }

    #[test]
    fn enter_device_stage_shows_high_priority_status_when_session_fetch_fails() {
        let _guard = SESSIONS_LOAD_TEST_LOCK.lock().unwrap();
        fn failed_sessions(
            _client: &mbv_core::api::EmbyClient,
        ) -> Result<Vec<mbv_core::api::SessionInfo>, String> {
            Err("session service unavailable".to_string())
        }
        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = Some(failed_sessions);
        let mut model = Model::new(make_app_stub());

        model.enter_device_stage("music".to_string(), "Music".to_string());

        *SESSIONS_LOAD_OVERRIDE.lock().unwrap() = None;
        assert!(model.app.status.contains("couldn't load devices"));
        assert!(
            model.app.status_expires.unwrap()
                >= std::time::Instant::now() + std::time::Duration::from_secs(4)
        );
    }

    #[test]
    fn failed_route_config_save_preserves_persistence_warning_and_stops_refresh() {
        let mut model = Model::new(make_app_stub());
        let should_refresh =
            model.finish_route_config_save(Err("write /blocked/config.toml: denied".to_string()));
        assert!(!should_refresh);
        assert!(model.app.status.contains("config save failed"));
        assert!(model
            .app
            .status
            .contains("write /blocked/config.toml: denied"));
    }

    #[test]
    fn route_config_save_is_captured_without_targeting_production_config() {
        ROUTE_CONFIG_SAVE_CAPTURE.with(|captured| captured.borrow_mut().clear());
        let mut cfg = crate::config::Config::default();
        cfg.library_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());

        assert!(save_route_config(&cfg).is_ok());

        ROUTE_CONFIG_SAVE_CAPTURE.with(|captured| {
            let captured = captured.borrow();
            assert_eq!(captured.len(), 1);
            assert_eq!(captured[0].library_routes, cfg.library_routes);
        });
    }

    #[test]
    fn route_config_save_result_guard_restores_previous_result() {
        ROUTE_CONFIG_SAVE_RESULT.with(|result| assert!(result.borrow().is_ok()));
        {
            let _guard = RouteConfigSaveResultGuard::set(Err("simulated".to_string()));
            ROUTE_CONFIG_SAVE_RESULT.with(|result| assert!(result.borrow().is_err()));
        }
        ROUTE_CONFIG_SAVE_RESULT.with(|result| assert!(result.borrow().is_ok()));
    }

    #[test]
    fn commit_device_selection_preserves_save_failure_warning_without_refresh() {
        let _save_result =
            RouteConfigSaveResultGuard::set(Err("write isolated config: denied".to_string()));
        let mut model = Model::new(make_app_stub());
        let endpoint = DaemonEndpoint::Tcp("127.0.0.1:9000".parse().unwrap());
        mount_library_routes(
            &mut model,
            LibraryRoutePopup {
                stage: LibraryRouteStage::PickDevice {
                    library_lower: "music".to_string(),
                    library_display: "Music".to_string(),
                    devices: vec![("living-room-pc".to_string(), Some(endpoint))],
                },
                cursor: 1,
            },
        );

        model.handle_library_routes_enter();

        assert!(model.app.status.contains("config save failed"));
        assert!(model.app.status.contains("write isolated config: denied"));
        assert!(matches!(
            library_routes_stage(&model),
            Some(LibraryRouteStage::PickDevice { .. })
        ));
    }

    #[test]
    fn commit_device_selection_clears_route_on_local_no_route() {
        let mut model = Model::new(make_app_stub());
        model
            .app
            .config
            .lock()
            .unwrap()
            .library_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
        model
            .app
            .library_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
        mount_library_routes(
            &mut model,
            LibraryRoutePopup {
                stage: LibraryRouteStage::PickDevice {
                    library_lower: "music".to_string(),
                    library_display: "Music".to_string(),
                    devices: vec![(
                        "living-room-pc".to_string(),
                        Some(DaemonEndpoint::Tcp("127.0.0.1:9000".parse().unwrap())),
                    )],
                },
                cursor: 0, // "Local (no route)"
            },
        );

        model.handle_library_routes_enter();

        assert_eq!(
            model.app.config.lock().unwrap().library_routes.get("music"),
            None
        );
        assert_eq!(model.app.library_routes.get("music"), None);
    }

    #[test]
    fn commit_device_selection_flashes_and_does_not_commit_for_an_unroutable_device() {
        // #256: selecting a greyed-out (None-endpoint) row must not write
        // anything to config -- there is nothing meaningful to write --
        // and must tell the user why, rather than silently doing nothing.
        let mut model = Model::new(make_app_stub());
        mount_library_routes(
            &mut model,
            LibraryRoutePopup {
                stage: LibraryRouteStage::PickDevice {
                    library_lower: "music".to_string(),
                    library_display: "Music".to_string(),
                    devices: vec![("no-port-device".to_string(), None)],
                },
                cursor: 1, // index 0 is "Local (no route)"; 1 is the device
            },
        );

        model.handle_library_routes_enter();

        assert_eq!(
            model.app.config.lock().unwrap().library_routes.get("music"),
            None
        );
        assert_eq!(model.app.library_routes.get("music"), None);
        assert!(model.app.status.contains("no-port-device"));
        assert!(model.app.status.contains("not currently routable"));
        // Still on the PickDevice stage -- a no-op, not silently
        // reverting to the library list either.
        assert!(matches!(
            library_routes_stage(&model),
            Some(LibraryRouteStage::PickDevice { .. })
        ));
    }
}
