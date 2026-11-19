//! The Library Status bar (`replace-tauri-with-blitz` task 7.1, the port of
//! `LibraryStatusBar`; add-gui-library-panel design D10; design D9 of
//! `move-playback-header-into-queue-footer`): one 40 px band under the Library
//! column, in the canvas's status-bar form — the canvas's mono type, 16 px
//! side padding, its top border, and its list-column fill.
//!
//! The Search field leads the bar's left side, with the chord's `Ctrl K` hint.
//! The typing schedules the debounced `searchQuery` command through
//! [`crate::window::library::search`], the core carries the results back on
//! the frame, and the dropdown paints them in the list-row look — or its
//! landing or failed state — above the field: the bar sits at the window's
//! bottom edge, so the results open upward. The keys that act on the results
//! route through the Library key router while the field holds the keyboard, so
//! this component never interprets a key: it only follows the focus fact, and
//! a highlight id that a new results frame no longer carries falls to the
//! first row. The field mounts only while the Library panel shows, so `Ctrl+K`
//! keeps its core effect wherever no bar paints it.
//!
//! The right side is the location: the destination, the selected collection
//! names, and the picked item's artist or title
//! ([`crate::window::library::status`]), joined by the canvas's separator. The
//! bar paints no key hints: the canvas's fixed hints are gone, and the field
//! takes their place.
//!
//! The frame's `GuiStatusBar` carries the armed prefix and the live toast,
//! the same facts the Queue footer's line paints. While either is active it
//! paints after the field — the toast wins over the prefix badge, as the
//! footer line drops the badge while a toast is live — and the field and the
//! location stay. The bar paints no interactive region of its own: a point on
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
