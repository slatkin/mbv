//! The Library panel (`replace-tauri-with-blitz` task 7.1, the port of
//! `LibraryPanel`; add-gui-library-panel design D4, D12): the column
//! beside the Queue panel. It paints the shell and the arrangement's
//! regions — the list, and, in Wide, the Hero pane beside the list; in Narrow
//! the Hero mounts as an overlay over the list once an item is picked. The top
//! bar is the window's global header above both columns, so the panel mounts
//! none (design D7 of `move-playback-header-into-queue-footer`). The active destination decides which painters mount
//! (design D12): Music paints the union list and the album Hero (the Home Hero
//! mounts with row 7.5); Home paints the flat list and its Hero, with no letter
//! rail and no Workspace; Shows paints its series list with the letter rail and
//! the series Hero with its Workspace; a plain Emby library paints its list with
//! the plain mode's rail and its Hero with no Workspace, and a feed-group
//! library paints the date-ordered list; Audiobooks paints the book list with
//! its letter rail and the book Hero with its chapters Workspace (row 9.1). The
//! Status bar closes the column under both.
//!
//! The collection dropdown is not a panel region (gui-collection-dropdown design
//! D6a): it rides the list header's title line, so the panel paints nothing
//! above the list, and the list's own component carries the destination's
//! collections frame to its header.
//!
//! The panel's interaction state lives in [`crate::window::library::state`]
//! and its keys in [`crate::window::library::keys`], so this shell holds no
//! state of its own. The Heroes mount for real (rows 7.5–7.6) and the
//! destination lists mount for real (row 7.7); the destination lists read
//! their carried frames from the section store themselves, so a panel
//! repaint never deep-clones a list's rows for them.
//! The collection dropdowns ride the list headers (gui-collection-dropdown row
//! 8.2).
//!
//! A press inside the panel stores no focus (design D2 of
//! `gui-pointer-target-interaction`): the pointer names the key target, so the
//! press acts on the row it lands on. The press that lands while the
//! right-click item menu or the collection dropdown is up closes it — the
//! outside-press rule — unless it landed on the menu or the dropdown itself,
//! whose handler stops this one.
//!
//! The Narrow Hero overlay covers the list, so it must be dismissible: its
//! backdrop press closes it, and Esc closes it in the key router (design
//! D3). The panel also reports the arrangement it paints, so the router
//! closes an overlay only where one actually paints.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::{GuiLibraryDetail, GuiLibraryList, GuiMusicMenuEntryFrame};
use crate::bridge::frame::overlay::GuiRowMenuEntry;
use crate::window::arrangement::GuiArrangement;
use crate::window::components::library_audiobooks_hero::LibraryAudiobooksHero;
use crate::window::components::library_emby_hero::LibraryEmbyHero;
use crate::window::components::library_favourites_hero::LibraryFavouritesHero;
use crate::window::components::library_favourites_list::LibraryFavouritesList;
use crate::window::components::library_feeds_hero::LibraryFeedsHero;
use crate::window::components::library_hero::LibraryHero;
use crate::window::components::library_hero_frame::HeroPaneWidth;
use crate::window::components::library_home_hero::LibraryHomeHero;
use crate::window::components::library_home_list::LibraryHomeList;
use crate::window::components::library_lettered_list::LibraryLetteredList;
use crate::window::components::library_list::LibraryList;
use crate::window::components::library_podcasts_hero::LibraryPodcastsHero;
use crate::window::components::library_shows_hero::LibraryShowsHero;
use crate::window::components::library_shows_list::LibraryShowsList;
use crate::window::components::library_status_bar::LibraryStatusBar;
use crate::window::components::panel_divider::{DividerName, DividerSide, PanelDivider};
use crate::window::components::queue_row_menu::QueueRowMenu;
use crate::window::library::favourites::favourites_part;
use crate::window::library::lettered::{AudiobooksList, EmbyList, FeedsList, PodcastsList};
use crate::window::library::parts::{LibraryPair, library_pair};
use crate::window::library::state::LibraryState;
use crate::window::pointer_target::PointerZone;
use crate::window::resize::PanelWidthsStore;
use crate::window::sections::SectionRef;

/// The props of the Library panel (the `LibraryPanel` props, in
/// order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryPanelProps {
    /// The arrangement picked from the Library column width (design D4).
    pub arrangement: GuiArrangement,
    /// The list column's painted width (design D4): the session's dragged
    /// width, or its default.
    pub list_width: f32,
    /// The Library column the list column sits in: the drag's clamp.
    pub library_column: f32,
}

/// A press inside the panel (design D8, D15; gui-collection-dropdown design
/// D7): the press closes the right-click item menu — the outside-press rule —
/// the open collection dropdown, whose next open starts with an empty filter,
/// and the view and season dropdowns. The menu's and the dropdowns' own
/// handlers stop this one from running, so a press on either surface acts on
/// it instead of closing it.
fn press_in_panel(library: &LibraryState) {
    if library.ui.peek().row_menu.is_some() {
        library.close_library_row_menu();
    }
    library.close_dropdowns();
}

/// Whether the pair paints a Hero (design D4, D12; emby-favourites design D6):
/// every destination's detail carries a `hero`, `None` while none is picked, and
/// the Favourites view carries its picked favourite's Hero on the list part.
fn pair_has_hero(pair: &LibraryPair) -> bool {
    if pair
        .favourites()
        .is_some_and(|favourites| favourites.hero.is_some())
    {
        return true;
    }
    match pair {
        LibraryPair::Music(_, detail) => detail.hero.is_some(),
        LibraryPair::Home(_, detail) => detail.hero.is_some(),
        LibraryPair::Shows(_, detail) => detail.hero.is_some(),
        LibraryPair::Emby(_, detail) => detail.hero.is_some(),
        LibraryPair::Audiobooks(_, detail) => detail.hero.is_some(),
        LibraryPair::Podcasts(_, detail) => detail.hero.is_some(),
        LibraryPair::Feeds(_, detail) => detail.hero.is_some(),
    }
}

/// The active destination's list painter (row 7.7): the list the former client
/// panel mounts for the pair's destination. Every list reads its carried
/// frames from the section store itself, so a panel repaint never
/// deep-clones a list's rows for it.
fn list_painter(pair: LibraryPair) -> Element {
    // The Favourites view paints the favourites list in place of the
    // destination's own list (emby-favourites design D6): the All list's stores
    // are untouched, so pressing All paints it as it was.
    if pair.favourites().is_some() {
        return rsx! {
            LibraryFavouritesList {}
        };
    }
    match pair {
        LibraryPair::Music(..) => rsx! {
            LibraryList {}
        },
        LibraryPair::Home(..) => rsx! {
            LibraryHomeList {}
        },
        LibraryPair::Shows(..) => rsx! {
            LibraryShowsList {}
        },
        LibraryPair::Emby(..) => rsx! {
            LibraryLetteredList::<EmbyList> {}
        },
        LibraryPair::Audiobooks(..) => rsx! {
            LibraryLetteredList::<AudiobooksList> {}
        },
        LibraryPair::Podcasts(..) => rsx! {
            LibraryLetteredList::<PodcastsList> {}
        },
        LibraryPair::Feeds(..) => rsx! {
            LibraryLetteredList::<FeedsList> {}
        },
    }
}

/// The active destination's Hero painter: the same painter the Wide pane and
/// the Narrow overlay mount, so the two cannot drift (design D4). The Music,
/// Home, Shows, plain Emby, Audiobooks, Podcasts, and Feeds Heroes are rows
/// 7.5 and 7.6's ports; the props carry what each Hero reads — the Shows and
/// Audiobooks chapters' commands name the library index, and each Hero takes
/// the carried handle and narrows its own part in its body, so a panel repaint
/// clones no section's content (`share-gui-sections-by-rc` design D3).
fn hero_painter(
    list: Option<&SectionRef<GuiLibraryList>>,
    detail: Option<&SectionRef<GuiLibraryDetail>>,
) -> Option<Element> {
    let list = list?;
    // The Favourites view's Hero rides the list part (emby-favourites design
    // D6), so it paints in place of the destination's own Hero, which carries
    // none while that view is active.
    if favourites_part(Some(&**list)).is_some_and(|(_, part)| part.hero.is_some()) {
        return Some(rsx! {
            LibraryFavouritesHero { list: list.clone() }
        });
    }
    let detail = detail?;
    let pair = library_pair(Some(&**list), Some(&**detail))?;
    Some(match pair {
        LibraryPair::Music(..) => rsx! {
            LibraryHero { detail: detail.clone() }
        },
        LibraryPair::Home(..) => rsx! {
            LibraryHomeHero { detail: detail.clone() }
        },
        LibraryPair::Shows(list_part, _) => rsx! {
            LibraryShowsHero {
                lib_idx: list_part.lib_idx,
                detail: detail.clone(),
            }
        },
        LibraryPair::Emby(..) => rsx! {
            LibraryEmbyHero {
                detail: detail.clone(),
            }
        },
        LibraryPair::Audiobooks(list_part, _) => rsx! {
            LibraryAudiobooksHero {
                lib_idx: list_part.lib_idx,
                detail: detail.clone(),
            }
        },
        LibraryPair::Podcasts(..) => rsx! {
            LibraryPodcastsHero {
                detail: detail.clone(),
            }
        },
        LibraryPair::Feeds(..) => rsx! {
            LibraryFeedsHero {
                list: list.clone(),
                detail: detail.clone(),
            }
        },
    })
}

/// The index View Detail reports. The core's entries report their own
/// shared-builder index, which is far below it, so a press names View Detail
/// and no core entry.
const VIEW_DETAIL_INDEX: usize = usize::MAX;

/// The View Detail entry (user request 2026-11-20): the window paints it first
/// in the row menu while the Hero would be an overlay, because right-clicking
/// it is the only way the Narrow Hero opens.
fn view_detail_entry() -> GuiRowMenuEntry {
    GuiRowMenuEntry {
        label: "View Detail".to_owned(),
        key_hint: None,
        destructive: false,
        checked: None,
        index: Some(VIEW_DETAIL_INDEX),
    }
}

/// The right-click menu's entries in the row menu's entry shape: the
/// Library's entries carry no destructive flag, and the entry's own
/// `index` — the core's shared-builder order — rides in the frame the
/// conversion keeps, so the menu paints and reports it and a press names the
/// entry it painted.
pub(crate) fn row_menu_entries(entries: &[GuiMusicMenuEntryFrame]) -> Vec<GuiRowMenuEntry> {
    entries
        .iter()
        .map(|entry| GuiRowMenuEntry {
            label: entry.label.clone(),
            key_hint: entry.key_hint.clone(),
            destructive: false,
            checked: None,
            index: Some(entry.index),
        })
        .collect()
}

/// The Library panel: the
/// Heroes mount (rows 7.5–7.6) and the destination lists mount (row 7.7).
#[component]
pub fn LibraryPanel(props: LibraryPanelProps) -> Element {
    let library = consume_context::<LibraryState>();
    let widths_store = consume_context::<PanelWidthsStore>();

    // The sections the panel reads (design D8): the destination tabs and the
    // active destination's list and detail parts, narrowed together by
    // `library_pair`; the Search section's results; the Status bar's facts;
    // and the right-click item menu the window last opened.
    let list_section = library.sections.library_list.cloned();
    let detail_section = library.sections.library_detail.cloned();
    let status = library.sections.status.cloned();
    let menu = library.sections.row_menu.cloned();
    let ui = library.ui.cloned();

    // The destination reset (design D2 of `gui-pointer-target-interaction`):
    // the core cannot reach the window state an open clears, so the panel
    // runs the reset itself when the carried active tab changes — a press on a
    // tab, a window digit, a router tab step, and the launch restore's open
    // all change `current_tab`, so one check covers them all. The first value
    // seen only sets the baseline.
    let watched_list = library.sections.library_list;
    let destination_reset = library.clone();
    use_effect(move || {
        let list = watched_list.cloned();
        let current_tab = list.as_deref().and_then(|list| list.current_tab);
        destination_reset.settle_destination_tab(current_tab);
    });

    // The presented destination tabs and the active one (design D2): the
    // top bar paints them. The tabs are small and bounded by the tab count,
    // so they stay cloned (`share-gui-sections-by-rc` non-goal).
    let tabs = list_section
        .as_ref()
        .map(|list| list.tabs.clone())
        .unwrap_or_default();
    let current_tab = list_section.as_ref().and_then(|list| list.current_tab);
    // The active destination's list and detail parts, narrowed together on
    // the destination's kind (design D8), borrowed from the handles: `None`
    // while no Library panel shows.
    let pair = library_pair(list_section.as_deref(), detail_section.as_deref());

    // Whether the Hero sits beside the list rather than over it.
    let arrangement = props.arrangement;
    let wide = arrangement == GuiArrangement::Wide;
    // The Narrow Hero overlay's facts: the active destination's pair while
    // its detail paints a Hero (design D4, D12), and no dismissal since that
    // pick. The arrangement is the window's own value, the same one the keys
    // read (design D4), so the paint and Esc cannot disagree.
    let overlay_open = arrangement == GuiArrangement::Narrow
        && pair.as_ref().is_some_and(pair_has_hero)
        && !ui.hero_dismissed;

    // The right-click item menu's press surface (design D8, D15): the
    // viewport-anchored card at the pointer position, painted while the
    // content carries the named item's entries. It belongs to the Library,
    // not to one destination, so a row of any destination opens it. A press
    // runs the entry the core built, resolved by that item's stable id.
    let menu_open = match (menu.as_ref(), ui.row_menu.as_ref()) {
        (Some(menu), Some(row_menu)) if menu.row == row_menu.row && !menu.entries.is_empty() => {
            Some((menu, row_menu))
        }
        _ => None,
    };
    // The Wide Hero pane's and the Narrow overlay's Hero fact, resolved once
    // per render.
    let pair_paints_hero = pair.as_ref().is_some_and(pair_has_hero);

    // The per-press handlers' own state handles (D4): every handler holds
    // its own clone.
    let press_library = library.clone();
    let backdrop_library = library.clone();
    // The Hero pane's width, for the backdrop's bake (design D4): the Library
    // column less the list column while Wide paints both.
    let hero_pane_width = if wide {
        (props.library_column - props.list_width).max(1.0)
    } else {
        props.library_column.max(1.0)
    };
    let mut pane_width = use_context_provider(|| HeroPaneWidth(Signal::new(hero_pane_width)));
    if (*pane_width.0.peek() - hero_pane_width).abs() >= 1.0 {
        pane_width.0.set(hero_pane_width);
    }
    // The list column's divider drag (design D4, D14): the store the drag's
    // width clamps and saves through. It needs no layout bump: the drag writes
    // the width the tree paints, so the frame that resolves the new inline
    // width reports the change and the measure follows (D9).
    let mut drag_widths = widths_store.clone();
    let mut reset_widths = widths_store;

    rsx! {
        div {
            class: "relative flex h-full min-h-0 min-w-0 flex-1 flex-col",
            style: "background: var(--canvas-library-column);",
            "data-library-panel": "",
            "data-library-arrangement": arrangement.name(),
            // The Library zone (`gui-pointer-target-interaction` D1): every key
            // whose pointer rests anywhere in this column is the Library's,
            // and the row and list markers under it name what the key acts
            // on.
            "data-pointer-zone": PointerZone::Library.token(),
            onpointerdown: move |_| press_in_panel(&press_library),
            div {
                class: "relative flex min-h-0 flex-1",
                // The list column: the canvas's index in Wide, the whole
                // Library column in Mini and Narrow. Rows 7.2 and 7.7 paint
                // the destination lists here. Wide paints the session's width
                // (design D4) at the left edge; its divider and the Hero pane
                // sit to its right.
                div {
                    "data-library-region": "list",
                    class: if wide {
                        "flex min-h-0 flex-col flex-none border-r"
                    } else {
                        "flex min-h-0 flex-col min-w-0 flex-1"
                    },
                    style: if wide {
                        format!(
                            "width: {}px; background: var(--canvas-library-panel); border-color: var(--canvas-border);",
                            props.list_width
                        )
                    } else {
                        "background: var(--canvas-library-panel); border-color: var(--canvas-border);".to_owned()
                    },
                    if let Some(pair) = pair {
                        { list_painter(pair) }
                    }
                }
                if wide {
                    // The Wide list column's divider (design D4): dragging it
                    // resizes the list column, a double-click returns the
                    // canvas width.
                    PanelDivider {
                        side: DividerSide::Left,
                        name: DividerName::List,
                        width: props.list_width,
                        on_drag: move |width| {
                            drag_widths.drag_list_width(width, props.library_column);
                        },
                        on_reset: move |()| {
                            reset_widths.reset_list_width();
                        },
                    }
                    // The Wide Hero pane, with the active destination's Hero inside.
                    div {
                        "data-library-region": "hero",
                        class: "flex min-h-0 min-w-0 flex-1 flex-col",
                        style: "background: var(--canvas-library-column); overflow-y: auto;",
                        if pair_paints_hero {
                            if let Some(hero) =
                                hero_painter(list_section.as_ref(), detail_section.as_ref())
                            {
                                { hero }
                            }
                        }
                    }
                }
                if overlay_open {
                    // The Narrow Hero overlay (design D4): picking an item
                    // paints the Hero over the list, through the same painter
                    // the Wide pane uses, so the two cannot drift. The
                    // backdrop press and Esc close it (design D3), so a picked
                    // item never leaves an overlay no input dismisses; the
                    // card scrolls its own content and swallows the presses
                    // that land on it. The overlay sits at `z-20`, above
                    // the root's Queue divider (`z-10`), whose band overlaps
                    // the Library column's edge by 3 px: a press there closes
                    // the overlay, it never starts a Queue resize.
                    div {
                        "data-library-region": "heroOverlay",
                        class: "absolute inset-0 z-20 flex",
                        style: "background: rgba(0, 0, 0, 0.45);",
                        button {
                            "type": "button",
                            class: "absolute inset-0",
                            style: "background: none; border: 0;",
                            "aria-label": "Close the Hero",
                            "data-library-hero-backdrop": "",
                            onclick: move |_| backdrop_library.close_library_hero(),
                        }
                        div {
                            class: "relative m-3 flex min-h-0 min-w-0 flex-1 flex-col overflow-y-auto",
                            style: "background: var(--canvas-library-column); border: 1px solid var(--canvas-border); border-radius: 8px;",
                            "data-library-hero-card": "",
                            if pair_paints_hero {
                                if let Some(hero) =
                                    hero_painter(list_section.as_ref(), detail_section.as_ref())
                                {
                                    { hero }
                                }
                            }
                        }
                    }
                }
            }
            // The Library Status bar (design D10, D9 of
            // `move-playback-header-into-queue-footer`): under the list
            // region, closing the column with the Search field and the
            // location while the panel paints.
            LibraryStatusBar {
                tabs,
                current_tab,
                list: list_section,
                detail: detail_section,
                status,
            }
            if let Some((menu, row_menu)) = menu_open {
                {
                    let menu_library = library.clone();
                    // The menu reports the entry's own index (the core's
                    // shared-builder order), so the press names the entry it
                    // painted.
                    let on_select = Callback::new(move |index: usize| {
                        if index == VIEW_DETAIL_INDEX {
                            menu_library.view_library_detail();
                        } else {
                            menu_library.run_library_row_menu_entry(index);
                        }
                    });
                    let mut entries = row_menu_entries(&menu.entries);
                    if arrangement == GuiArrangement::Narrow && !overlay_open {
                        entries.insert(0, view_detail_entry());
                    }
                    let top = format!("{}px", row_menu.y);
                    let left = format!("{}px", row_menu.x);
                    rsx! {
                        div {
                            class: "fixed",
                            style: "left: 0; top: 0; width: 0; height: 0; z-index: 30;",
                            onpointerdown: move |event: PointerEvent| {
                                // A press on the menu does not close it: the
                                // outside-press rule skips the menu, and the
                                // stop keeps the panel's close from running.
                                event.stop_propagation();
                            },
                            QueueRowMenu {
                                cursor: None,
                                entries,
                                top,
                                left: Some(left),
                                on_select: Some(on_select),
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
