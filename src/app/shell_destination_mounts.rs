//! Catalog-driven destination mount reconciliation (keep-destination-
//! components-mounted, tasks 1.1-1.3).
//!
//! Destination surface components stay mounted while their Service library
//! is in the catalog (design D1); the `Model` `*_id` fields are
//! active-destination pointers, and this module retires components whose
//! library left the catalog. TuiRealm's `Application` has no public component
//! enumeration, so stale discovery reads a shell-maintained registry
//! (`Model::mounted_destinations`) that mirrors every destination
//! `mount`/`umount`; live keys are derived from `App::libs` /
//! `App::audiobookshelf_libraries`.

use std::collections::HashSet;

use super::components::{BrowserKey, BrowserKind, ComponentId};
use super::shell::Model;
use mbv_core::config::ServiceKind;

impl Model {
    /// Every destination `ComponentId::Browser(BrowserKey{..})` the current
    /// Service catalog could produce, independent of the active tab or view
    /// mode: per Emby library all five browse kinds (`Generic` / `Movies` /
    /// `HomeVideos` / `TvShows` / `Music`, keyed by `library.library.id`),
    /// per Audiobookshelf library both browse kinds (`AudiobookshelfBook` /
    /// `AudiobookshelfPodcast`). The reconciliation pass (1.2) matches
    /// mounted `Browser` and `InlineSearch` ids against these keys by
    /// `library_id`.
    pub(super) fn live_destination_keys(&self) -> HashSet<ComponentId> {
        let mut keys = HashSet::new();
        for tab in &self.app.libs {
            let library_id = tab.library.id.clone();
            for kind in [
                BrowserKind::Generic,
                BrowserKind::Movies,
                BrowserKind::HomeVideos,
                BrowserKind::TvShows,
                BrowserKind::Music,
            ] {
                keys.insert(ComponentId::Browser(BrowserKey {
                    service: ServiceKind::Emby,
                    library_id: library_id.clone(),
                    kind,
                }));
            }
        }
        for library in &self.app.audiobookshelf_libraries {
            for kind in [
                BrowserKind::AudiobookshelfBook,
                BrowserKind::AudiobookshelfPodcast,
            ] {
                keys.insert(ComponentId::Browser(BrowserKey {
                    service: ServiceKind::Audiobookshelf,
                    library_id: library.id.clone(),
                    kind,
                }));
            }
        }
        keys
    }

    /// Retire destination components whose Service library left the catalog.
    /// Iterates the maintained `mounted_destinations` registry (TuiRealm's
    /// `Application` has no component enumeration), so stale discovery is
    /// complete: it finds a retired library's mounted `InlineSearch` with no
    /// `Model` pointer, and a retired `Browser` whose pointer was already
    /// cleared by a narrow/drill transition. Each mounted `Browser(key)` /
    /// `InlineSearch(key)` whose `key.library_id` has no live library is
    /// `umount`ed, its registry entry removed, and any active-destination
    /// pointer still equal to it cleared to `None`.
    ///
    /// Collect the retired ids first (snapshot before mutating `application`
    /// while iterating the registry). Idempotent and cheap enough to run
    /// once per tick immediately before `sync_active_destination`.
    pub(super) fn reconcile_destination_mounts(&mut self) {
        let live_library_ids: HashSet<String> = self
            .live_destination_keys()
            .iter()
            .filter_map(|id| match id {
                ComponentId::Browser(key) | ComponentId::InlineSearch(key) => {
                    Some(key.library_id.clone())
                }
                _ => None,
            })
            .collect();

        let retired: Vec<ComponentId> = self
            .mounted_destinations
            .iter()
            .filter_map(|id| match id {
                ComponentId::Browser(key) | ComponentId::InlineSearch(key)
                    if !live_library_ids.contains(&key.library_id) =>
                {
                    Some(id.clone())
                }
                _ => None,
            })
            .collect();
        for id in &retired {
            let _ = self.application.umount(id);
            self.mounted_destinations.remove(id);
            self.clear_destination_pointer(id);
        }
    }

    /// Record a destination component as mounted in the maintained registry.
    /// Called at every destination `mount` site (Emby browser, TV, Music, ABS
    /// podcast, ABS book, inline search) so the registry is the shell's
    /// source of truth for stale-discovery.
    pub(super) fn register_destination(&mut self, id: &ComponentId) {
        self.mounted_destinations.insert(id.clone());
    }

    /// Forget a destination component from the maintained registry. Called at
    /// every destination `umount` site (including `dismiss_inline_search`)
    /// alongside the `application.umount`.
    pub(super) fn unregister_destination(&mut self, id: &ComponentId) {
        self.mounted_destinations.remove(id);
    }

    /// Clear any active-destination pointer still equal to `id` (the shell's
    /// `*_id` fields are pointers, not ownership; `None` suppresses the
    /// per-draw render gate).
    fn clear_destination_pointer(&mut self, id: &ComponentId) {
        for pointer in [
            &mut self.emby_browser_id,
            &mut self.tv_workspace_id,
            &mut self.music_workspace_id,
            &mut self.abs_podcast_id,
            &mut self.abs_book_id,
        ] {
            if pointer.as_ref() == Some(id) {
                *pointer = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::render::make_movie_app;
    use crate::app::tests::make_item;
    use crate::app::{LibraryTab, TabSelection};

    fn browser_id(library_id: &str, kind: BrowserKind) -> ComponentId {
        ComponentId::Browser(BrowserKey {
            service: ServiceKind::Emby,
            library_id: library_id.into(),
            kind,
        })
    }

    /// Two Emby libraries, both with a mounted `Browser` component (one
    /// generic, one tvshows), and the model's pointers set to match. `retain`
    /// selects which library stays in the catalog.
    fn two_browser_model(retain: usize) -> Model {
        let mut app = make_movie_app();
        // Second library with a distinct id.
        let mut second = LibraryTab::new(make_item("Series", "CollectionFolder"));
        second.library.id = "lib-series".into();
        second.library.collection_type = "tvshows".into();
        app.libs.push(second);

        let mut model = Model::new(app);
        model
            .application
            .mount(
                browser_id("lib-movies", BrowserKind::Generic),
                Box::new(crate::app::components::BrowserComponent::new_for_kind(
                    BrowserKind::Generic,
                )),
                vec![],
            )
            .expect("mount lib-movies browser");
        model
            .application
            .mount(
                browser_id("lib-series", BrowserKind::TvShows),
                Box::new(crate::app::components::BrowserComponent::new_for_kind(
                    BrowserKind::TvShows,
                )),
                vec![],
            )
            .expect("mount lib-series browser");
        model.register_destination(&browser_id("lib-movies", BrowserKind::Generic));
        model.register_destination(&browser_id("lib-series", BrowserKind::TvShows));
        model.emby_browser_id = Some(browser_id("lib-movies", BrowserKind::Generic));
        model.tv_workspace_id = Some(browser_id("lib-series", BrowserKind::TvShows));
        // `make_movie_app` seeds lib-movies at index 0; lib-series was pushed
        // at index 1. retain is the catalog index to KEEP: retain=0 keeps
        // lib-movies (drop lib-series = retire its browser), retain=1 keeps
        // lib-series (drop lib-movies = retire its browser).
        model.app.libs.remove(if retain == 0 { 1 } else { 0 });
        model
    }

    #[test]
    fn live_destination_keys_enumerates_every_kind_per_emby_library() {
        let mut app = make_movie_app();
        let mut second = LibraryTab::new(make_item("Series", "CollectionFolder"));
        second.library.id = "lib-series".into();
        second.library.collection_type = "tvshows".into();
        app.libs.push(second);
        let model = Model::new(app);

        let keys = model.live_destination_keys();
        // Emby: 5 kinds for each of the two libraries.
        assert_eq!(keys.len(), 10);
        for kind in [
            BrowserKind::Generic,
            BrowserKind::Movies,
            BrowserKind::HomeVideos,
            BrowserKind::TvShows,
            BrowserKind::Music,
        ] {
            assert!(keys.contains(&browser_id("lib-movies", kind)));
            assert!(keys.contains(&browser_id("lib-series", kind)));
        }
        // No ABS keys when the ABS catalog is empty.
        assert!(!keys.iter().any(|id| matches!(id, ComponentId::Browser(key) if key.service == ServiceKind::Audiobookshelf)));
    }

    #[test]
    fn live_destination_keys_enumerates_abs_book_and_podcast() {
        use mbv_core::audiobookshelf::AudiobookshelfLibrary;
        let mut app = crate::app::tests::make_app_stub();
        app.audiobookshelf_libraries.push(AudiobookshelfLibrary {
            id: "abs-lib".into(),
            name: "ABS".into(),
            media_type: "book".into(),
        });
        let model = Model::new(app);

        let keys = model.live_destination_keys();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&ComponentId::Browser(BrowserKey {
            service: ServiceKind::Audiobookshelf,
            library_id: "abs-lib".into(),
            kind: BrowserKind::AudiobookshelfBook,
        })));
        assert!(keys.contains(&ComponentId::Browser(BrowserKey {
            service: ServiceKind::Audiobookshelf,
            library_id: "abs-lib".into(),
            kind: BrowserKind::AudiobookshelfPodcast,
        })));
    }

    #[test]
    fn reconcile_unmounts_only_retired_library_and_clears_its_pointer() {
        let mut model = two_browser_model(0); // keep lib-movies, drop lib-series
        let kept = browser_id("lib-movies", BrowserKind::Generic);
        let retired = browser_id("lib-series", BrowserKind::TvShows);

        assert!(model.application.mounted(&kept));
        assert!(model.application.mounted(&retired));
        assert_eq!(model.emby_browser_id, Some(kept.clone()));
        assert_eq!(model.tv_workspace_id, Some(retired.clone()));

        model.reconcile_destination_mounts();

        // The dropped library's browser is gone and its pointer cleared.
        assert!(!model.application.mounted(&retired));
        assert_eq!(model.tv_workspace_id, None);
        // The still-live library's browser is untouched.
        assert!(model.application.mounted(&kept));
        assert_eq!(model.emby_browser_id, Some(kept));
    }

    #[test]
    fn reconcile_keeps_inline_search_of_live_library() {
        let mut model = two_browser_model(1); // keep lib-series, drop lib-movies
        let inline_id = ComponentId::InlineSearch(BrowserKey {
            service: ServiceKind::Emby,
            library_id: "lib-series".into(),
            kind: BrowserKind::TvShows,
        });
        model
            .application
            .mount(
                inline_id.clone(),
                Box::new(crate::app::components::InlineSearchComponent::new()),
                vec![],
            )
            .expect("mount inline search");
        model.register_destination(&inline_id);
        let retired = browser_id("lib-movies", BrowserKind::Generic);

        model.reconcile_destination_mounts();

        assert!(model.application.mounted(&inline_id));
        assert!(!model.application.mounted(&retired));
        assert_eq!(model.emby_browser_id, None);
    }

    /// Reviewer block regression: a stale mounted `InlineSearch` whose
    /// library left the catalog is retired even when no `Model` pointer names
    /// it (inline searches have no `*_id` field).
    #[test]
    fn reconcile_retires_unpointed_stale_inline_search() {
        let mut model = two_browser_model(0); // keep lib-movies, drop lib-series
                                              // No Model pointer: mount an inline search for the retired library.
        let stale_inline = ComponentId::InlineSearch(BrowserKey {
            service: ServiceKind::Emby,
            library_id: "lib-series".into(),
            kind: BrowserKind::TvShows,
        });
        model
            .application
            .mount(
                stale_inline.clone(),
                Box::new(crate::app::components::InlineSearchComponent::new()),
                vec![],
            )
            .expect("mount stale inline search");
        model.register_destination(&stale_inline);
        // No pointer references the stale inline search (it has none).

        model.reconcile_destination_mounts();

        assert!(
            !model.application.mounted(&stale_inline),
            "unpointed stale InlineSearch must be retired"
        );
        assert!(!model.mounted_destinations.contains(&stale_inline));
    }

    /// Reviewer block regression: a stale mounted `Browser` whose library
    /// left the catalog is retired even when its `*_id` pointer was already
    /// cleared by a narrow/drill transition (the pointer is `None`, so the
    /// registry is the only discovery source).
    #[test]
    fn reconcile_retires_unpointed_stale_browser() {
        let mut model = two_browser_model(0); // keep lib-movies, drop lib-series
        let stale_browser = browser_id("lib-series", BrowserKind::TvShows);
        // Simulate the narrow/drill transition: the pointer was cleared but
        // the component stays mounted and registered (keep-mounted).
        model.tv_workspace_id = None;

        assert!(model.application.mounted(&stale_browser));
        assert_eq!(model.tv_workspace_id, None);
        assert!(model.mounted_destinations.contains(&stale_browser));

        model.reconcile_destination_mounts();

        assert!(
            !model.application.mounted(&stale_browser),
            "unpointed stale Browser must be retired via the registry"
        );
        assert!(!model.mounted_destinations.contains(&stale_browser));
    }

    /// keep-destination-components-mounted task 3.4: a music library visited
    /// in both album-folder and generic views never leaves two mounted
    /// destination components sharing its `library_id`, and retiring the
    /// library removes every destination key it could produce.
    ///
    /// The `*_component_id()` gates are mutually exclusive on `collection_type`
    /// (design D1 mitigation): the Music workspace mounts only in the
    /// album-folder view, and the generic Emby browser gate excludes the
    /// `Music` kind, so the generic view never mounts a second destination for
    /// the same library. Reconciliation keys on `library_id` presence, so when
    /// the library leaves the catalog BOTH the `Music` and `Generic` keys it
    /// can produce are retired together.
    #[test]
    fn music_library_never_has_two_mounted_destinations_and_both_retire_together() {
        let mut app = crate::app::tests::make_app_stub();
        app.tab = TabSelection::EmbyLibrary(0);
        app.music_levels = vec!["group".into(), "album".into()];
        let mut library = make_item("Music", "CollectionFolder");
        library.id = "lib-music".into();
        library.is_folder = true;
        library.collection_type = "music".into();
        let mut group = make_item("Alpha", "MusicArtist");
        group.id = "group-0".into();
        group.is_folder = true;
        let mut album = make_item("First Album", "MusicAlbum");
        album.id = "album-1".into();
        album.artist = "Alpha".into();
        album.is_folder = true;
        app.libs.push(LibraryTab {
            nav_stack: vec![
                crate::app::BrowseLevel {
                    parent_id: "lib-music".into(),
                    title: "Music".into(),
                    items: vec![group],
                    total_count: 1,
                    cursor: 0,
                    scroll: 0,
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,
                    all_items: None,
                    letter_filter: None,
                    music_grouping: None,
                },
                crate::app::BrowseLevel {
                    parent_id: "group-0".into(),
                    title: "Alpha".into(),
                    items: vec![album],
                    total_count: 1,
                    cursor: 0,
                    scroll: 0,
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,
                    all_items: None,
                    letter_filter: None,
                    music_grouping: None,
                },
            ],
            ..LibraryTab::new(library)
        });
        let mut model = Model::new(app);
        model.app.layout.main.wide_music_area = ratatui::layout::Rect::new(0, 0, 100, 30);
        model.app.layout.main.wide_music_right_area = ratatui::layout::Rect::new(50, 0, 50, 30);

        // Album-folder view: the Music workspace is the destination.
        model.sync_music_workspace();
        let music_id = model
            .music_workspace_id
            .clone()
            .expect("Music workspace mounted");
        assert_eq!(library_id_of(&music_id), "lib-music");
        assert!(model.application.mounted(&music_id));
        assert!(model.mounted_destinations.contains(&music_id));

        // Generic view: the Emby browser gate excludes the Music kind, so no
        // second destination mounts for the same library — at most one
        // non-InlineSearch destination per library_id at a time (D1
        // mutual-exclusion mitigation).
        model.sync_emby_browser();
        assert_eq!(model.emby_browser_id, None);
        let mounted_for_lib: Vec<_> = model
            .mounted_destinations
            .iter()
            .filter(|id| library_id_of(id) == "lib-music")
            .collect();
        assert_eq!(
            mounted_for_lib.len(),
            1,
            "no two destination components may share a library_id after visiting both views"
        );

        // Simulate the risk scenario the D1 mitigation names: both keys the
        // music library can produce are mounted (the generic-view fallback
        // plus the album-folder Music workspace), sharing one library_id.
        let generic_id = browser_id("lib-music", BrowserKind::Generic);
        model
            .application
            .mount(
                generic_id.clone(),
                Box::new(crate::app::components::BrowserComponent::new_for_kind(
                    BrowserKind::Generic,
                )),
                vec![],
            )
            .expect("mount generic fallback browser");
        model.register_destination(&generic_id);
        assert_ne!(music_id, generic_id);
        assert!(model.application.mounted(&music_id));
        assert!(model.application.mounted(&generic_id));

        // Retire the library: reconciliation keys on library_id presence, so
        // BOTH destinations for the retired library are unmounted together.
        model.app.libs.remove(0);
        model.reconcile_destination_mounts();
        assert!(
            !model.application.mounted(&music_id),
            "the Music destination must be retired"
        );
        assert!(
            !model.application.mounted(&generic_id),
            "the Generic destination must be retired"
        );
        assert!(!model.mounted_destinations.contains(&music_id));
        assert!(!model.mounted_destinations.contains(&generic_id));
    }

    fn library_id_of(id: &ComponentId) -> &str {
        match id {
            ComponentId::Browser(key) | ComponentId::InlineSearch(key) => &key.library_id,
            _ => panic!("expected a destination id"),
        }
    }
}
