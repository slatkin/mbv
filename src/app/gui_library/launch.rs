//! The GUI's launch location (add-gui-library-panel design D9): the
//! destination, the selected collections of each destination, and Panel
//! focus, saved at orderly exit and restored after the catalog lands.
//!
//! The GUI keeps its own file ([`mbv_config::save_gui_launch_state`]): the TUI
//! launch state never moves this Client's tab or Panel focus, and this file
//! never holds a TUI launch location. Identities are stable Service ids, so a
//! resolution that finds no match falls back instead of opening a stale
//! position.

use mbv_config::{
    GUI_LAUNCH_STATE_VERSION, GuiDestinationCollections, GuiGroupPick, GuiLaunchState,
};
use mbv_config::{LaunchPanelFocus, TabIdentity};
use mbv_queue::ServiceKind;
use mbv_ui_model::browse::BrowseLevel;
use mbv_ui_model::settings::PanelFocus;
use mbv_ui_model::tab_selection::TabSelection;

use super::tabs::GuiDestinationKind;
use crate::app::App;

impl App {
    /// The GUI's launch location (design D9): the active destination, the
    /// selected collections of each destination, and Panel focus. Read at
    /// orderly exit only; nothing writes it while the window is open.
    #[must_use]
    pub fn gui_launch_state(&self) -> GuiLaunchState {
        GuiLaunchState {
            version: GUI_LAUNCH_STATE_VERSION,
            destination: self.gui_launch_destination(),
            panel_focus: match self.gui_panel_focus() {
                PanelFocus::Library => LaunchPanelFocus::Library,
                PanelFocus::Queue => LaunchPanelFocus::Queue,
            },
            collections: self.gui_launch_collections(),
        }
    }

    /// Open the destination a saved location names (design D9): the presented
    /// destination whose identity matches, or the first presented one while
    /// the GUI presents only some destinations. Returns the opened
    /// destination's identity as the saved file carries it; `None` means the
    /// catalog holds none yet and the caller retries when one lands. The
    /// caller applies the saved Panel focus after the open, because opening a
    /// library takes Library focus.
    pub fn open_gui_launch_destination(&mut self, state: &GuiLaunchState) -> Option<TabIdentity> {
        let position = self.presented_destination_position(&state.destination)?;
        self.set_library_tab(position);
        Some(self.gui_launch_destination())
    }

    /// Apply the saved collection choices of the destination the first phase
    /// opened (design D9, D13): the saved picks of the groups the library
    /// still holds restore, and the first saved group the library still
    /// holds — in saved order — is the one selected; with none, a saved set
    /// the library emptied, and a destination with no saved entry, a fresh
    /// launch's case, the destination keeps its default first group (design
    /// D6). The saved entry matches `destination`, the identity phase 1
    /// recorded, and
    /// the active destination must still be that one: a mid-restore tab
    /// switch abandons the restore, so no other destination's saved set
    /// lands under the user. Returns `false` while the destination's group
    /// list has not landed, so the caller retries.
    pub fn restore_gui_launch_collections(
        &mut self,
        state: &GuiLaunchState,
        destination: &TabIdentity,
    ) -> bool {
        let Some(lib_idx) = self.gui_music_destination() else {
            // The active destination paints no collection pills.
            return true;
        };
        if self.gui_launch_destination() != *destination {
            // A different destination is active: the restore gives up rather
            // than applying a saved set the user did not ask for.
            return true;
        }
        let Some(present) = self.gui_music_present_groups(lib_idx) else {
            return false;
        };
        // The saved entry of the destination the first phase opened: its
        // picks restore first, and its group ids select the group with.
        let saved_entry = state
            .collections
            .iter()
            .find(|entry| entry.destination == *destination);
        // The saved picks of the groups the library still holds (design D13)
        // restore first, and never overwrite a pick the session already made.
        for pick in saved_entry.map_or(&[][..], |entry| entry.picks.as_slice()) {
            if present.contains(&pick.group_id) {
                self.gui_library_picks
                    .entry((lib_idx, pick.group_id.clone()))
                    .or_insert_with(|| pick.album_id.clone());
            }
        }
        if self.gui_library_groups.contains_key(&lib_idx) {
            // The user already selected a group in the pre-restore window: the
            // restore never clobbers a choice the session made.
            return true;
        }
        // The saved group the library still holds, in saved order, is the
        // one selected. Without one, the destination keeps the default first
        // group (design D6), so a fresh launch opens on the first group.
        let saved_group = saved_entry.and_then(|entry| {
            entry
                .group_ids
                .iter()
                .find(|group_id| present.contains(group_id))
        });
        if let Some(group_id) = saved_group.cloned() {
            self.select_gui_library_collection(lib_idx, group_id);
        }
        true
    }

    /// The identity of the active destination (design D9), as the saved file
    /// carries it: a stable Service library id, never a tab position.
    fn gui_launch_destination(&self) -> TabIdentity {
        match self.tab {
            TabSelection::Home => TabIdentity::Home,
            TabSelection::Feeds => TabIdentity::Feeds,
            TabSelection::EmbyLibrary(lib_idx) => {
                self.libs.get(lib_idx).map_or(TabIdentity::Home, |tab| {
                    library_identity(ServiceKind::Emby, &tab.library.id)
                })
            }
            TabSelection::AudiobookshelfLibrary(index) => self
                .audiobookshelf_libraries
                .get(index)
                .map_or(TabIdentity::Home, |library| {
                    library_identity(ServiceKind::Audiobookshelf, &library.id)
                }),
        }
    }

    /// The selected collection and the group picks of the catalog's
    /// destinations (design D9, D13), in catalog order: one entry per
    /// destination whose choice or pick the session holds, so a destination the
    /// session never touched opens on its first group. The file keeps the list
    /// shape, and each entry holds the one selected group and the picks in
    /// group-id order, so the saved file is stable.
    fn gui_launch_collections(&self) -> Vec<GuiDestinationCollections> {
        (0..self.libs.len())
            .filter_map(|lib_idx| {
                let selected = self.gui_library_groups.get(&lib_idx);
                let mut picks: Vec<GuiGroupPick> = self
                    .gui_library_picks
                    .iter()
                    .filter(|((pick_lib, _), _)| *pick_lib == lib_idx)
                    .map(|((_, group_id), album_id)| GuiGroupPick {
                        group_id: group_id.clone(),
                        album_id: album_id.clone(),
                    })
                    .collect();
                if selected.is_none() && picks.is_empty() {
                    return None;
                }
                picks.sort_by(|a, b| a.group_id.cmp(&b.group_id));
                Some(GuiDestinationCollections {
                    destination: library_identity(
                        ServiceKind::Emby,
                        &self.libs[lib_idx].library.id,
                    ),
                    group_ids: selected.cloned().into_iter().collect(),
                    picks,
                })
            })
            .collect()
    }

    /// The destinations the GUI presents, in catalog order (design D9, D2):
    /// the Grouped Music libraries, the Emby TV (Shows) libraries, every
    /// other Emby library (row 8.1's plain Emby-library destination), the
    /// Audiobookshelf libraries — the book libraries' Audiobooks destination
    /// (row 9.1) and the podcast libraries' Podcasts destination (row 10.1) —
    /// and the Feeds tab when subscriptions are configured (row 11.1), as the
    /// destination-kind enum names them. The restore's fallback and the
    /// frame's destination tabs (`gui_library_tabs`) read this one list, so
    /// they cannot disagree about the presented set.
    pub(in crate::app::gui_library) fn presented_destinations(&self) -> Vec<GuiDestinationKind> {
        let mut presented: Vec<GuiDestinationKind> = (0..self.libs.len())
            .map(|lib_idx| {
                if self.is_grouped_music_library(lib_idx) {
                    GuiDestinationKind::Music { lib_idx }
                } else if self.libs[lib_idx].library.collection_type == "tvshows" {
                    GuiDestinationKind::Shows { lib_idx }
                } else {
                    GuiDestinationKind::Library { lib_idx }
                }
            })
            .collect();
        // The Audiobookshelf libraries' tab positions follow the Emby ones,
        // so appending them keeps the catalog order. A library's browse kind
        // picks its destination: a book library presents Audiobooks and a
        // podcast library presents Podcasts.
        presented.extend(
            (0..self.audiobookshelf_libraries.len()).filter_map(|lib_idx| {
                match self.audiobookshelf_kind_at(lib_idx) {
                    Some(mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind::Book) => {
                        Some(GuiDestinationKind::Audiobooks { lib_idx })
                    }
                    Some(
                        mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind::Podcast,
                    ) => Some(GuiDestinationKind::Podcasts { lib_idx }),
                    None => None,
                }
            }),
        );
        // The Feeds tab follows every Service library, and it presents while
        // subscriptions are configured (row 11.1): the TUI's own tab-strip
        // rule, so a subscription set the session emptied presents no tab.
        presented.extend(
            self.has_feeds_subscriptions()
                .then_some(GuiDestinationKind::Feeds),
        );
        presented
    }

    /// Whether an open-destination command's `position` names a destination
    /// the GUI presents (design D2): Home, or one of the presented
    /// destinations' tab positions. The command arm reads this one
    /// definition, so a stale frame's position cannot activate a destination
    /// the top bar does not paint — an unpresented destination opens
    /// nothing, as the absent digit key does.
    pub(in crate::app::gui_library) fn presents_open_position(&self, position: usize) -> bool {
        position == 0
            || self
                .presented_destinations()
                .iter()
                .any(|kind| self.gui_destination_position(*kind) == Some(position))
    }

    /// The tab position of the destination a saved location names, resolved
    /// against the destinations the GUI presents (design D9): the matching
    /// presented library, else the first presented one. A saved
    /// Audiobookshelf destination whose catalog has not settled yet resolves
    /// to nothing, so the restore retries instead of opening a fallback the
    /// catalog's arrival would contradict. A saved Feeds location resolves to
    /// the feed tab while subscriptions are configured, and falls back like
    /// every other unpresented destination when they are not. The GUI
    /// presents the Grouped Music and Shows libraries, so every other saved
    /// destination falls back to the first of them.
    fn presented_destination_position(&self, destination: &TabIdentity) -> Option<usize> {
        let presented = self.presented_destinations();
        // The Feeds destination names no library: the saved identity matches
        // the presented kind or nothing.
        if *destination == TabIdentity::Feeds {
            let feeds = presented
                .iter()
                .copied()
                .find(|kind| matches!(kind, GuiDestinationKind::Feeds));
            return self.gui_destination_position(feeds.or_else(|| presented.first().copied())?);
        }
        let saved = match destination {
            TabIdentity::ServiceLibrary { kind, library_id } => Some((*kind, library_id.as_str())),
            TabIdentity::Home | TabIdentity::Feeds => None,
        };
        if saved.is_some_and(|(kind, _)| kind == ServiceKind::Audiobookshelf)
            && self.gui_abs_catalog_pending()
        {
            return None;
        }
        let kind = saved
            .and_then(|(saved_kind, library_id)| {
                presented.iter().copied().find(|kind| {
                    kind.library_index().is_some_and(|lib_idx| {
                        self.gui_destination_library_id(saved_kind, lib_idx)
                            .is_some_and(|id| id == library_id)
                    })
                })
            })
            .or_else(|| presented.first().copied())?;
        self.gui_destination_position(kind)
    }

    /// The stable Service library id the presented destination `kind` names
    /// at `lib_idx` (design D9): the identity the launch file saves and the
    /// restore resolves against. An Emby kind's index names `App::libs`; the
    /// Audiobooks and Podcasts kinds' index names the Audiobookshelf
    /// libraries.
    fn gui_destination_library_id(&self, kind: ServiceKind, lib_idx: usize) -> Option<String> {
        match kind {
            ServiceKind::Audiobookshelf => {
                Some(self.audiobookshelf_libraries.get(lib_idx)?.id.clone())
            }
            ServiceKind::Emby => Some(self.libs.get(lib_idx)?.library.id.clone()),
        }
    }

    /// Whether the Audiobookshelf catalog can still land (row 9.1): the
    /// startup is still running, or its catalog fetch is still in flight —
    /// either way the libraries list is not final, and a saved
    /// Audiobookshelf destination's resolution must wait for it.
    fn gui_abs_catalog_pending(&self) -> bool {
        self.audiobookshelf_runtime.state == mbv_core::service_runtime::ServiceState::Connecting
            || self.setup.audiobookshelf_catalog_rx.is_some()
    }

    /// The active Grouped Music library's group level: the library's root in
    /// `nav_stack`, whose items are the collection groups the pills paint.
    pub(in crate::app::gui_library) fn gui_music_group_level(
        &self,
        lib_idx: usize,
    ) -> Option<&BrowseLevel> {
        let lib = self.libs.get(lib_idx)?;
        lib.nav_stack
            .iter()
            .find(|level| level.parent_id == lib.library.id)
    }

    /// The groups the library holds, in pill order. `None` while the list has
    /// not landed: the level is absent, or its fetch is still in flight.
    pub(in crate::app::gui_library) fn gui_music_present_groups(
        &self,
        lib_idx: usize,
    ) -> Option<Vec<String>> {
        let level = self.gui_music_group_level(lib_idx)?;
        if level.loading {
            return None;
        }
        Some(level.items.iter().map(|group| group.id.clone()).collect())
    }
}

/// The saved identity of one Service library.
fn library_identity(kind: ServiceKind, library_id: &str) -> TabIdentity {
    TabIdentity::ServiceLibrary {
        kind,
        library_id: library_id.to_string(),
    }
}
