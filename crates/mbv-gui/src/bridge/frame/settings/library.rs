//! The Library section's content (`add-gui-settings-screen` design D9): its F2
//! choice rows, the routable libraries with their assigned devices, the
//! selected library, and that library's candidate devices. The routes live
//! inline in this section, so the `LibraryRoutes` F2 row is absent from the
//! rows (design D9).

use serde::Serialize;

/// One routable library (design D9): `library` is the lowercased name the
/// saved route table is keyed by — the `routeDevices` and `assignRoute`
/// commands' identity — and `endpoint` the assigned device route, `null` while
/// the library uses the automatic route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiSettingsRoute {
    pub library: String,
    /// The library's display name, as the TUI paints it.
    pub name: String,
    pub endpoint: Option<String>,
}

impl From<&mbv::GuiSettingsRouteProjection> for GuiSettingsRoute {
    fn from(row: &mbv::GuiSettingsRouteProjection) -> Self {
        Self {
            library: row.library.clone(),
            name: row.name.clone(),
            endpoint: row.endpoint.clone(),
        }
    }
}

/// One candidate device of the selected library (design D9): `name` is the
/// device the `assignRoute` command names, and `endpoint` the direct-connect
/// endpoint that command saves, `null` when the device is not currently
/// routable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiSettingsRouteDevice {
    pub name: String,
    pub endpoint: Option<String>,
}

impl From<&mbv::GuiSettingsRouteDeviceProjection> for GuiSettingsRouteDevice {
    fn from(device: &mbv::GuiSettingsRouteDeviceProjection) -> Self {
        Self {
            name: device.name.clone(),
            endpoint: device.endpoint.clone(),
        }
    }
}
