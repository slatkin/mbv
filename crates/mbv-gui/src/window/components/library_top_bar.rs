//! The window's global header (`replace-tauri-with-blitz` task 7.1, the port of
//! `LibraryTopBar`; add-gui-library-panel design D2, D12; design D7 of
//! `move-playback-header-into-queue-footer`): the root mounts it above the
//! panel row, across the full window width, in every arrangement, and the
//! Settings screen mounts none. It paints the canvas's Home icon, then one tab
//! per presented destination in the frame's catalog order, with the active
//! destination's tab marked current. The icon is current while the Home
//! content is present (the destinations are exclusive), and a press on the
//! icon or on a tab sends the open-destination command with that destination's
//! own position.
//!
//! The bar carries the Library pointer zone (design D7): the pointer over the
//! header is over the Library, so a key pressed there routes as it does over
//! the Library column. A destination press also reports to the root, which
//! shows the Library in place of the Queue while the Mini arrangement holds
//! (design D8).
//!
//! The Search field is not here: it leads the Library Status bar's left side
//! (design D9). The Settings control stays at the bar's right end
//! (add-gui-settings-screen design D11 *Mouse-centric*): a press opens the
//! Settings screen at Services, so the pointer has its own way into the
//! screen.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiLibraryTabFrame;
use crate::bridge::frame::theme::GuiRole;
use crate::bridge::settings::GuiSettingsCommand;
use crate::window::icons::{ChevronLeft, ChevronRight, Settings};
use crate::window::input::f32_from_f64;
use crate::window::layout::LayoutEpoch;
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerZone;
use crate::window::theme::{canvas_var, role_var};

/// The props of the Library top bar (the `LibraryTopBar` props, in
/// order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryTopBarProps {
    /// The presented destination tabs in catalog order (design D2).
    pub tabs: Vec<GuiLibraryTabFrame>,
    /// The active tab's position, or `None` while the active destination is
    /// not one of the tabs (design D2): Home, and every destination the GUI
    /// does not present yet.
    pub current_tab: Option<usize>,
    /// Whether the Home content is present (design D12): the Home icon
    /// marks itself current on it alone.
    #[props(default)]
    pub home_active: bool,
    /// A destination press reported to the root (design D8), carrying the
    /// pressed position: the root shows the Library in place of the Queue
    /// while the Mini arrangement holds, and the active destination's own
    /// control puts the Queue back.
    #[props(default)]
    pub on_destination_opened: Option<Callback<usize>>,
}

/// Home's own position in the destination order (design D2): the Home icon
/// opens it, and the window reads the position to tell a Home press from a tab
/// press.
pub(crate) const HOME_POSITION: usize = 0;

/// Open the destination tab at `position` (design D2): the Home icon opens
/// [`HOME_POSITION`]. The window's own report follows the command and carries
/// the pressed position (design D8).
fn open_destination(
    library: &LibraryState,
    position: usize,
    on_destination_opened: Option<Callback<usize>>,
) {
    library.open_destination(position);
    if let Some(on_destination_opened) = on_destination_opened {
        on_destination_opened.call(position);
    }
}

/// Open the Settings screen at Services (design D11 *Mouse-centric*), as the
/// `settings_open` action does: the pointer's way in, so a user can enter
/// the screen without a key.
fn open_settings(library: &LibraryState) {
    library.link.settings(GuiSettingsCommand::Open);
}

/// The window's global header.
#[component]
pub fn LibraryTopBar(props: LibraryTopBarProps) -> Element {
    let library = consume_context::<LibraryState>();

    // The tab strip's overflow window: the strip's measured width, and the
    // first painted tab the chevrons step. The strip's basis is zero, so its
    // width never depends on the tabs it holds.
    let mut strip_mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    let mut strip_width = use_signal(|| 0.0_f32);
    let mut first_tab = use_signal(|| 0_usize);
    let epoch = try_consume_context::<LayoutEpoch>().map(|epoch| epoch.0);
    use_effect(move || {
        // Reading the epoch re-measures the strip on every layout change.
        let _epoch = epoch.as_ref().map(Signal::cloned);
        let mounted = strip_mounted.cloned();
        spawn(async move {
            let Some(event) = mounted else {
                return;
            };
            let Ok(rect) = event.get_client_rect().await else {
                return;
            };
            let width = f32_from_f64(rect.width());
            if (*strip_width.peek() - width).abs() > 0.5 {
                strip_width.set(width);
            }
        });
    });
    // A new current tab, a new tab list, or a new strip width scrolls the
    // current tab into view. The held first tab reads through `peek`, so a
    // chevron press is never undone by this effect.
    use_effect(use_reactive(
        (&props.tabs, &props.current_tab),
        move |(tabs, current)| {
            let width = strip_width.cloned();
            let Some(current) =
                current.and_then(|position| tabs.iter().position(|tab| tab.position == position))
            else {
                return;
            };
            let start =
                tabs::start_showing(&tabs::tab_widths(&tabs), current, *first_tab.peek(), width);
            if start != *first_tab.peek() && width > 0.0 {
                first_tab.set(start);
            }
        },
    ));
    let tab_window = tabs::tab_window(
        &tabs::tab_widths(&props.tabs),
        first_tab.cloned(),
        strip_width.cloned(),
    );

    // The current tab's run: the tab bar's underline role in both strokes.
    let underline = role_var(GuiRole::TabSelectedUnderline);

    // The Home icon's and the tabs' painted colours and the current tab's
    // weight, resolved before the markup.
    let home_color = if props.home_active {
        canvas_var("heading")
    } else {
        canvas_var("muted")
    };

    // The per-press handlers' own state handles (D4): every handler holds
    // its own clone.
    let home_library = library.clone();
    let home_opened = props.on_destination_opened;
    let settings_library = library.clone();

    rsx! {
        nav {
            class: "flex flex-none items-center",
            "aria-label": "Libraries",
            "data-library-region": "topBar",
            // The Library zone (design D7): the header is the Library's, so
            // the keys pressed over it route as they do over the column.
            "data-pointer-zone": PointerZone::Library.token(),
            style: "height: 56px; gap: 2px; padding: 0 12px 0 8px; border-bottom: 1px solid var(--canvas-border); background: var(--canvas-dim);",
            // The canvas's Home icon, before the tabs (design D12): the
            // current destination marker follows the tabs' `aria-current`
            // rule while the Home content is present.
            button {
                "type": "button",
                class: "flex flex-none items-center justify-center",
                style: "width: 44px; height: 44px; color: {home_color}; cursor: pointer;",
                "aria-label": "Home",
                "aria-current": props.home_active.then_some("page"),
                "data-library-home": "",
                onclick: move |_| open_destination(&home_library, HOME_POSITION, home_opened),
                // The canvas's Home glyph, verbatim.
                svg {
                    width: "18",
                    height: "18",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    "stroke-width": "1.8",
                    "stroke-linejoin": "round",
                    "aria-hidden": "true",
                    path { d: "M3 11 12 4l9 7v9a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z" }
                }
            }
            // The tab strip: the tabs that fit, with a chevron at each edge
            // that hides some. The chevrons step the first painted tab by
            // one, as the TUI's tab bar scrolls its anchor.
            div {
                class: "flex items-center",
                style: "flex: 1 1 0; min-width: 120px; height: 56px; gap: 2px; overflow: hidden;",
                "data-library-tabs": "",
                onmounted: move |event: Event<MountedData>| strip_mounted.set(Some(event)),
                if tab_window.left {
                    button {
                        "type": "button",
                        class: "flex flex-none items-center justify-center",
                        style: "width: 24px; height: 44px; border: 0; background: none; color: var(--canvas-muted); cursor: pointer;",
                        "aria-label": "Previous tabs",
                        "data-library-tabs-left": "",
                        onclick: move |_| first_tab.set(tab_window.start.saturating_sub(1)),
                        ChevronLeft { size: 16 }
                    }
                }
                for tab in &props.tabs[tab_window.start..tab_window.end] {
                    {
                        let current = Some(tab.position) == props.current_tab;
                        let position = tab.position;
                        let label = tab.label.clone();
                        let weight = if current { "700" } else { "500" };
                        let color = if current {
                            canvas_var("heading")
                        } else {
                            canvas_var("muted")
                        };
                        let tab_library = library.clone();
                        let tab_opened = props.on_destination_opened;
                        rsx! {
                            button {
                                key: "{position}",
                                "type": "button",
                                class: "flex items-center uppercase",
                                style: "height: 56px; padding: 0 12px; font-size: 14px; font-weight: {weight}; letter-spacing: 0.06em; color: {color};",
                                "aria-current": current.then_some("page"),
                                "data-library-tab": "{position}",
                                onclick: move |_| open_destination(&tab_library, position, tab_opened),
                                if current {
                                    // The current tab's run: the canvas's two
                                    // underline strokes around the label.
                                    span {
                                        class: "block",
                                        style: "border-top: 2px solid {underline}; border-bottom: 2px solid {underline}; padding: 3px 1px 2px; line-height: 1;",
                                        "{label}"
                                    }
                                } else {
                                    "{label}"
                                }
                            }
                        }
                    }
                }
                if tab_window.right {
                    button {
                        "type": "button",
                        class: "flex flex-none items-center justify-center",
                        style: "width: 24px; height: 44px; border: 0; background: none; color: var(--canvas-muted); cursor: pointer;",
                        "aria-label": "Next tabs",
                        "data-library-tabs-right": "",
                        onclick: move |_| first_tab.set(tab_window.start + 1),
                        ChevronRight { size: 16 }
                    }
                }
            }
            // The Settings control at the bar's right end (design D11
            // *Mouse-centric*, the spec's *A Settings control in the top bar
            // opens the screen*): a Lucide cog whose press opens the screen at
            // Services. The global hover veil gives it its pointer feedback.
            button {
                "type": "button",
                class: "flex flex-none items-center justify-center",
                style: "width: 34px; height: 34px; border: 0; border-radius: 6px; background: none; color: var(--canvas-muted); cursor: pointer;",
                "aria-label": "Settings",
                "data-library-settings": "",
                onclick: move |_| open_settings(&settings_library),
                Settings { size: 18 }
            }
        }
    }
}

mod tabs;

#[cfg(test)]
mod tests;
