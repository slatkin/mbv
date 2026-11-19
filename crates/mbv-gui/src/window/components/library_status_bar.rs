//! The Library Status bar (`replace-tauri-with-blitz` task 7.1, the port of
//! `LibraryStatusBar`; add-gui-library-panel design D10; design D9 of
//! `move-playback-header-into-queue-footer`): one 40 px band under the Library
//! column, in the canvas's status-bar form — the canvas's mono type, 16 px
//! side padding, its top border, and its list-column fill.
//!
//! No Search field paints here (user request 2026-11-19): the top bar paints
//! it at its right end, as `LibrarySearchControl` — inline while the bar has
//! room, and otherwise as an icon whose press opens a panel that carries the
//! same field, hint, and results. That control owns the field's mount, its
//! keyboard hold, and its results dropdown; see
//! [`crate::window::components::library_search_control`] and
//! [`crate::window::library::search`]. The keys that act on the results route
//! through the Library key router while the field holds the keyboard, so no
//! component interprets a key. The field mounts wherever the top bar paints
//! the control, so `Ctrl+K` arms it there and keeps its core effect wherever
//! the control paints nothing.
//!
//! The location is the bar's one content: the destination, the selected
//! collection names, and the picked item's artist or title
//! ([`crate::window::library::status`]), flush right and joined by the canvas's
//! separator. The bar paints no key hints.
//!
//! The frame's `GuiStatusBar` carries the armed prefix and the live toast,
//! the same facts the Queue footer's line paints. While either is active it
//! paints at the left edge — the toast wins over the prefix badge, as the
//! footer line drops the badge while a toast is live — and the location
//! stays. The bar paints no interactive region of its own: a point on
//! it resolves to no target, as the canvas's bar carries none, and it reports
//! no layout contribution.

use dioxus_native::prelude::*;

use crate::bridge::frame::chrome::GuiStatusBar;
use crate::bridge::frame::library::{GuiLibraryDetail, GuiLibraryList, GuiLibraryTabFrame};
use crate::window::library::status::library_location;
use crate::window::sections::SectionRef;
use crate::window::status::toast_color;

/// The props of the Library Status bar (the `LibraryStatusBar` props,
/// in order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryStatusBarProps {
    /// The presented destination tabs and the active one (design D2): the
    /// location's first segment. While the Home content is present the
    /// location's segments come from it.
    pub tabs: Vec<GuiLibraryTabFrame>,
    /// The active tab's position, or `None` while the active destination is
    /// not one of the tabs.
    pub current_tab: Option<usize>,
    /// The Library list section (gui-change-driven-sections design D8): the
    /// location's destination, the selected collections, and the selected
    /// row's title. `None` while no destination is active. The handle is the
    /// one the panel paints from (`share-gui-sections-by-rc` design D3), so
    /// a bar repaint clones no list's rows.
    #[props(default)]
    pub list: Option<SectionRef<GuiLibraryList>>,
    /// The active destination's detail part: the picked item's Hero title,
    /// and the selected row the location names. `None` while no destination
    /// is active, as the same one handle.
    #[props(default)]
    pub detail: Option<SectionRef<GuiLibraryDetail>>,
    /// The armed prefix and the live toast (design D10): they paint at the
    /// bar's left edge while active.
    pub status: GuiStatusBar,
}

/// The Library Status bar: the
/// canvas status bar band, the armed prefix or the live toast at the left
/// edge, and the location flush at the right. The Search field it once led
/// its left side with is the top bar's right end now (user request
/// 2026-11-19).
#[component]
pub fn LibraryStatusBar(props: LibraryStatusBarProps) -> Element {
    let location = library_location(
        &props.tabs,
        props.current_tab,
        props.list.as_deref(),
        props.detail.as_deref(),
    );
    // The live toast's severity colour, resolved before the markup (the
    // the former client `toastColor` call in the attribute).
    let toast_style = props.status.toast.as_ref().map_or(String::new(), |toast| {
        format!("color: {};", toast_color(toast.severity))
    });

    rsx! {
        div {
            class: "flex flex-none items-center justify-between font-mono whitespace-nowrap",
            style: "height: 40px; padding-inline: 16px; font-size: 13px; border-top: 1px solid var(--canvas-border); background: var(--canvas-library-panel);",
            "data-library-status": "",
            // The left side: the armed prefix or the live toast while either
            // is active.
            div {
                class: "flex min-w-0 items-center",
                style: "gap: 12px;",
                span {
                    class: "overflow-hidden",
                    if let Some(toast) = props.status.toast.as_ref() {
                        span {
                            style: "{toast_style}",
                            "data-library-status-toast": "",
                            "{toast.text}"
                        }
                    } else if props.status.prefix_armed {
                        span {
                            class: "font-bold",
                            style: "color: var(--mbv-accent);",
                            "data-library-status-prefix": "",
                            "PREFIX"
                        }
                    }
                }
            }
            // The location, flush at the right edge: the destination, the
            // selected collections, and the picked item, joined by the
            // canvas's separator.
            span {
                class: "overflow-hidden",
                style: "color: var(--canvas-muted);",
                "data-library-status-location": "",
                "{location}"
            }
        }
    }
}

#[cfg(test)]
mod tests;
