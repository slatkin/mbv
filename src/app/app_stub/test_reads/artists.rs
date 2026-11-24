//! The music Workspace's test fixture (`split-tui-gui-clients` design D4b
//! rule 5): a settled one-album artist-root catalogue whose only album carries
//! a Service (`ArtistItems`) identity, beside a ready Emby runtime — the state
//! the artist-detail and album-track requests are observable from.
//!
//! The shell's `music_workspace` tests seeded that state by writing the
//! catalogue fields directly; the seam names the fixture instead, so those
//! tests keep their claims and reach no field.

use super::super::App;
use mbv_ui_model::browse::{BrowseLevel, BrowseResting, ServerRows};
use mbv_ui_model::library_tab::LibraryTab;
use mbv_ui_model::tab_selection::TabSelection;

/// The catalogue revision the settled grouping carries.
pub const SETTLED_CATALOG_REVISION: u64 = 7;

/// The one album the settled catalogue's artist root groups.
pub const SETTLED_ALBUM_ID: &str = "album-1";

impl App {
    /// Hold `entry` as the artist-detail cache's answer for `key`, as a landed
    /// artist push leaves it.
    #[cfg(any(test, feature = "test"))]
    pub fn seed_artist_detail_cache_for_test(
        &mut self,
        key: mbv_ui_model::music_artist_detail::ArtistDetailKey,
        entry: mbv_ui_model::music_artist_detail::ArtistDetailCacheEntry,
    ) {
        self.artist_detail_cache.insert(key, entry);
    }

    /// A music-grouping app whose artist root resolves one settled album held
    /// under [`SETTLED_ALBUM_ID`] and attributed to `artist-alpha`, with images
    /// enabled and a ready (unroutable) Emby runtime.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn settled_service_artist_app_for_test() -> Self {
        let mut app = super::super::built::make_app_stub();
        app.enable_card_images_for_test();
        app.emby_runtime = ready_emby_runtime_for_test();
        app.seed_music_grouping_levels_for_test();
        app
    }

    /// Hold the Music tab's group and album levels, then settle the album
    /// level's grouped catalogue with every album attributed to `artist-alpha`.
    #[cfg(any(test, feature = "test"))]
    fn seed_music_grouping_levels_for_test(&mut self) {
        self.tab = TabSelection::EmbyLibrary(0);
        self.music_levels = vec!["group".into(), "album".into()];
        self.libs.push(music_library_tab());
        let Some(level) = self
            .libs
            .first_mut()
            .and_then(|tab| tab.nav_stack.last_mut())
        else {
            return;
        };
        for item in &mut level.items {
            item.artist_items = vec![mbv_emby_model::EmbyArtistRef {
                name: "Alpha".into(),
                id: "artist-alpha".into(),
            }];
        }
        let mut catalog = mbv_ui_model::music_grouping::build_grouped_album_catalog(
            &level.items,
            &std::collections::HashMap::<String, String>::default(),
        );
        catalog.revision = SETTLED_CATALOG_REVISION;
        catalog.parent_id.clone_from(&level.parent_id);
        level.music_grouping = Some(mbv_ui_model::music_grouping::MusicGroupingState {
            revision: SETTLED_CATALOG_REVISION,
            candidate: None,
            settled: Some(catalog),
        });
    }
}

/// A ready Emby runtime: a configured, unroutable client whose state is
/// `Ready`, so a launch-time fetch path arms with no live Service.
#[cfg(any(test, feature = "test"))]
pub(super) fn ready_emby_runtime_for_test() -> crate::app::state::service_runtime::EmbyRuntime {
    use mbv_emby::EmbyCredentialExchange;
    let mut client = mbv_emby::EmbyClient::new(crate::app::config::Config::default());
    client.apply_credential_exchange(&EmbyCredentialExchange {
        server_url: "http://127.0.0.1:1".into(),
        user_id: "user-id".into(),
        token: "token".into(),
    });
    crate::app::state::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ))
}

/// The Music tab's two levels: the artist group root, then the one album
/// [`SETTLED_ALBUM_ID`] under `group-0`.
#[cfg(any(test, feature = "test"))]
fn music_library_tab() -> LibraryTab {
    let mut library = mbv_emby_model::test_support::make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.is_folder = true;
    library.collection_type = "music".into();

    let groups = ["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta"]
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let mut group = mbv_emby_model::test_support::make_item(name, "MusicArtist");
            group.id = format!("group-{i}");
            group.is_folder = true;
            group
        })
        .collect();
    let mut album = mbv_emby_model::test_support::make_item("First Album", "MusicAlbum");
    album.id = SETTLED_ALBUM_ID.into();
    album.artist = "Alpha".into();
    album.production_year = 2001;

    LibraryTab {
        nav_stack: vec![
            music_level("lib-music", "Music", groups),
            music_level("group-0", "Alpha", vec![album]),
        ],
        ..LibraryTab::new(library)
    }
}

/// One browse level of the Music tab holding `items`.
#[cfg(any(test, feature = "test"))]
fn music_level(parent_id: &str, title: &str, items: Vec<mbv_emby_model::EmbyItem>) -> BrowseLevel {
    BrowseLevel {
        rows: ServerRows::new(items.len()),
        parent_id: parent_id.into(),
        title: title.into(),
        items,
        resting: BrowseResting::new(0, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    }
}
