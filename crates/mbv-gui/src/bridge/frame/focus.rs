//! The focus section of a frame: the surface that holds focus and the Panel that
//! holds it (design D4). The surface mirrors `ComponentId` variant for variant,
//! and the nested overlay, modal, and popup ids mirror their own enums.

use mbv_surfaces::FocusStack;
use mbv_ui_model::settings::PanelFocus;
use mbv_ui_msg::{ComponentId, ModalId, OverlayId, PopupId};
use serde::Serialize;

/// The focused surface and the Panel that holds focus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiFocus {
    /// The surface that holds focus, or `None` when no surface does.
    pub surface: Option<GuiSurface>,
    /// The Panel that holds focus.
    pub panel: GuiPanel,
}

impl From<&FocusStack> for GuiFocus {
    fn from(focus: &FocusStack) -> Self {
        Self {
            surface: focus.focused().map(GuiSurface::from),
            panel: GuiPanel::from(focus.panel_focus()),
        }
    }
}

/// A surface that can hold focus, mirroring `ComponentId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiSurface {
    UiRoot,
    TabPanel,
    StatusBarPanel,
    LibraryPlaybackPanel,
    QueuePlaybackPanel,
    Queue,
    QueueBoundary,
    Library,
    Overlay(GuiOverlay),
    Modal(GuiModal),
    Popup(GuiPopup),
}

impl From<ComponentId> for GuiSurface {
    fn from(id: ComponentId) -> Self {
        match id {
            ComponentId::UiRoot => Self::UiRoot,
            ComponentId::TabPanel => Self::TabPanel,
            ComponentId::StatusBarPanel => Self::StatusBarPanel,
            ComponentId::LibraryPlaybackPanel => Self::LibraryPlaybackPanel,
            ComponentId::QueuePlaybackPanel => Self::QueuePlaybackPanel,
            ComponentId::Queue => Self::Queue,
            ComponentId::QueueBoundary => Self::QueueBoundary,
            ComponentId::Library => Self::Library,
            ComponentId::Overlay(overlay) => Self::Overlay(GuiOverlay::from(overlay)),
            ComponentId::Modal(modal) => Self::Modal(GuiModal::from(modal)),
            ComponentId::Popup(popup) => Self::Popup(GuiPopup::from(popup)),
        }
    }
}

/// A top-level overlay, mirroring `OverlayId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiOverlay {
    Search,
    Settings,
    Sessions,
    Playlists,
    Help,
    ContextMenu,
}

impl From<OverlayId> for GuiOverlay {
    fn from(id: OverlayId) -> Self {
        match id {
            OverlayId::Search => Self::Search,
            OverlayId::Settings => Self::Settings,
            OverlayId::Sessions => Self::Sessions,
            OverlayId::Playlists => Self::Playlists,
            OverlayId::Help => Self::Help,
            OverlayId::ContextMenu => Self::ContextMenu,
        }
    }
}

/// A modal, mirroring `ModalId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiModal {
    Confirm,
    DaemonLost,
    SavePlaylist,
    TwitchUrl,
}

impl From<ModalId> for GuiModal {
    fn from(id: ModalId) -> Self {
        match id {
            ModalId::Confirm => Self::Confirm,
            ModalId::DaemonLost => Self::DaemonLost,
            ModalId::SavePlaylist => Self::SavePlaylist,
            ModalId::TwitchUrl => Self::TwitchUrl,
        }
    }
}

/// A popup, mirroring `PopupId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiPopup {
    Multiselect,
    LibraryRoutes,
    FeedManage,
}

impl From<PopupId> for GuiPopup {
    fn from(id: PopupId) -> Self {
        match id {
            PopupId::Multiselect => Self::Multiselect,
            PopupId::LibraryRoutes => Self::LibraryRoutes,
            PopupId::FeedManage => Self::FeedManage,
        }
    }
}

/// The Panel that holds focus, mirroring `PanelFocus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GuiPanel {
    Queue,
    Library,
}

impl From<PanelFocus> for GuiPanel {
    fn from(focus: PanelFocus) -> Self {
        match focus {
            PanelFocus::Queue => Self::Queue,
            PanelFocus::Library => Self::Library,
        }
    }
}
