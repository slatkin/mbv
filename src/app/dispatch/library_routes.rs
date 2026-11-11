//! The F2 "Library routes" picker's shared effects (add-gui-settings-screen
//! design D7): the library/session fetches, route persistence, and
//! post-save refresh that both Clients run, moved out of the TUI shell. The
//! TUI `LibraryRoutesComponent` keeps its stages and cursor; these methods
//! return the stage content the caller assigns.

use super::super::App;
use super::notify::ToastSeverity;

/// One candidate library row: (lowercased name, display name, assigned
/// direct-connect endpoint, if any).
type LibraryRouteRow = (String, String, Option<String>);

/// One candidate device row: (device name, direct-connect endpoint as the
/// wire form, or `None` when it is not currently routable).
type DeviceRouteRow = (String, Option<String>);

impl App {
    /// Fetch the Emby library views and each one's assigned route
    /// (add-gui-settings-screen design D7): the body of the TUI shell's
    /// `open_library_routes` without the component calls. Returns `None`
    /// when there is no Emby client or the fetch failed (the failure toasts
    /// here).
    pub(in crate::app) fn route_libraries(&mut self) -> Option<Vec<LibraryRouteRow>> {
        tracing::info!(
            name: "library_route.picker.opened",
            target: "library_route",
            "F2 route picker opened"
        );
        let client = self.emby_client()?;
        let client = client.lock().unwrap();
        let all = match client.get_views() {
            Ok(all) => {
                tracing::info!(name: "library_route.library_fetch.succeeded", target: "library_route", count = all.len(), "F2 library fetch succeeded");
                all
            }
            Err(e) => {
                tracing::warn!(name: "library_route.library_fetch.failed", target: "library_route", { error = %e }, "F2 library fetch failed");
                drop(client);
                self.flash(
                    format!("⚠ Library routes couldn't load libraries ({e})"),
                    ToastSeverity::Error,
                );
                return None;
            }
        };
        let routes = self.config.lock().unwrap().library_routes.clone();
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
        Some(items)
    }

    /// Fetch the candidate output devices for `library_lower`
    /// (add-gui-settings-screen design D7): the body of the TUI shell's
    /// `enter_device_stage` without the component calls. Returns the
    /// `(device name, direct-connect endpoint)` candidates plus the cursor
    /// seed that points at the library's current route. Returns `None` when
    /// the session fetch failed (the failure toasts here) or there is no
    /// Emby client.
    pub(in crate::app) fn route_devices(
        &mut self,
        library_lower: &str,
    ) -> Option<(Vec<DeviceRouteRow>, usize)> {
        let sessions = match self.fetch_sessions_blocking() {
            Ok(sessions) => {
                tracing::info!(name: "library_route.session_fetch.succeeded", target: "library_route", count = sessions.len(), "F2 session fetch succeeded");
                sessions
            }
            Err(e) => {
                tracing::warn!(name: "library_route.session_fetch.failed", target: "library_route", { library = %library_lower, error = %e }, "F2 session fetch failed");
                self.flash(
                    format!("⚠ Library routes couldn't load devices ({e})"),
                    ToastSeverity::Error,
                );
                return None;
            }
        };
        let client = self.emby_client()?;
        let local_device_name = client.lock().unwrap().device_name.clone();
        let mut devices: Vec<(String, Option<mbv_remote_player::DaemonEndpoint>)> = sessions
            .iter()
            .filter(|s| s.client.eq_ignore_ascii_case("mbv"))
            .filter(|s| !s.device_name.eq_ignore_ascii_case(&local_device_name))
            .map(|s| {
                let endpoint = self.session_direct_endpoint(s);
                if let Some(endpoint) = &endpoint {
                    tracing::info!(name: "library_route.endpoint.eligible", target: "library_route", device = %s.device_name, endpoint = %endpoint, "F2 endpoint eligible");
                } else {
                    tracing::info!(name: "library_route.endpoint.rejected", target: "library_route", device = %s.device_name, reason = "no resolvable direct-connect endpoint", "F2 endpoint rejected");
                }
                (s.device_name.clone(), endpoint)
            })
            .collect();
        devices.sort_by(|a, b| a.0.cmp(&b.0));
        devices.dedup_by(|a, b| a.0.eq_ignore_ascii_case(&b.0));
        tracing::info!(name: "library_route.candidates.counted", target: "library_route", count = devices.len(), library = %library_lower, "F2 candidate count");

        let current_endpoint = self
            .config
            .lock()
            .unwrap()
            .library_routes
            .get(library_lower)
            .and_then(|raw| mbv_remote_player::DaemonEndpoint::parse(raw).ok());
        let cursor = current_endpoint
            .and_then(|current| {
                devices
                    .iter()
                    .position(|(_, ep)| ep.as_ref() == Some(&current))
            })
            .map_or(0, |idx| idx + 1);
        Some((
            devices
                .into_iter()
                .map(|(name, endpoint)| (name, endpoint.map(|endpoint| endpoint.to_string())))
                .collect(),
            cursor,
        ))
    }

    /// Write the chosen route for `library_lower` and refresh the library
    /// list (add-gui-settings-screen design D7): the body of the TUI
    /// shell's `commit_device_selection` without the component calls. The
    /// choice is the picked device row, where `None` (the list's automatic
    /// entry) removes the override; the refusal for a device with no
    /// resolvable endpoint stays here. On success returns the refreshed
    /// library rows plus the restored cursor for the picked library.
    pub(in crate::app) fn assign_library_route(
        &mut self,
        library_lower: &str,
        device: Option<&DeviceRouteRow>,
    ) -> Option<(Vec<LibraryRouteRow>, usize)> {
        if let Some((name, None)) = device {
            self.flash(
                format!("{name} is not currently routable (no resolvable direct-connect endpoint)"),
                ToastSeverity::Neutral,
            );
            return None;
        }
        // Writes the selected device route into config, or removes the
        // override when the automatic (index 0) entry is chosen.
        {
            let mut c = self.config.lock().unwrap();
            if let Some((_, Some(endpoint))) = device {
                c.library_routes
                    .insert(library_lower.to_string(), endpoint.clone());
                tracing::info!(name: "library_route.endpoint.persisted", target: "library_route", library = %library_lower, endpoint = %endpoint, "F2 endpoint persisted");
            } else {
                c.library_routes.remove(library_lower);
                tracing::info!(name: "library_route.route.removed", target: "library_route", library = %library_lower, "F2 route removed");
            }
        }
        let cfg = self.config.lock().unwrap().clone();
        self.library_routes = cfg.library_routes.clone();
        tracing::info!(name: "library_route.route_table.synchronized", target: "library_route", count = self.library_routes.len(), "runtime route table synchronized");
        match crate::config::save_config_section(&cfg, mbv_config::ConfigSection::LibraryRoutes) {
            Ok(()) => {
                tracing::info!(name: "library_route.config_save.succeeded", target: "library_route", "config save succeeded");
            }
            Err(e) => {
                tracing::warn!(name: "library_route.config_save.failed", target: "library_route", { error = %e }, "config save failed");
                self.flash(
                    format!("⚠ Library route changed but config save failed ({e})"),
                    ToastSeverity::Error,
                );
                return None;
            }
        }
        let client = self.emby_client()?;
        let refresh_result = { client.lock().unwrap().get_views() };
        let all = match refresh_result {
            Ok(all) => all,
            Err(e) => {
                tracing::warn!(name: "library_route.library_refresh.failed", target: "library_route", { error = %e }, "F2 post-save library refresh failed");
                self.flash(
                    format!("⚠ Library route saved but couldn't refresh libraries ({e})"),
                    ToastSeverity::Error,
                );
                return None;
            }
        };
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
            .position(|(lower, _, _)| lower == library_lower)
            .unwrap_or(0);
        Some((items, restored_cursor))
    }
}
