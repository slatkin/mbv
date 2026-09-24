use crate::app::infra::ui_util::natural_sort_key;
use crate::app::notify_actions::ToastSeverity;
use crate::app::{App, PendingQueueAction, ReplacementExecutor, RoutedReplacementPrep};
use mbv_core::api::EmbyItem;
use rand::seq::SliceRandom;

fn sort_playable_items(items: &mut Vec<EmbyItem>) {
    items.retain(|item| !item.is_folder);
    items.sort_by_key(|item| natural_sort_key(item.sort_key()));
}

impl App {
    /// Shuffle from the generic Emby browser's component-resolved selected
    /// item (task 5.3d, Emby browser shuffle decoupling). `EmbyLibraryContent`
    /// resolved the item at its component-local cursor; when that item is a
    /// folder the folder itself is shuffled, otherwise the current browse
    /// level's parent is shuffled (falling back to the library id exactly as
    /// the legacy shuffle path did). The folder target comes from the supplied item,
    /// never from re-reading `BrowseLevel.cursor`.
    pub(in crate::app) fn shuffle_play_selected(&mut self, lib_idx: usize, item: EmbyItem) {
        let explicit_folder = item.is_folder.then_some(item.id.clone());
        self.shuffle_play_target(lib_idx, explicit_folder);
    }

    /// Shared explicit-target tail for both shuffle entry points. `explicit_folder`
    /// is the folder id to shuffle when the caller already knows the selected
    /// item is a folder (the typed browser path supplies it from the
    /// component-resolved item); when `None`, the current browse level's
    /// parent is shuffled, falling back to the library id.
    pub(in crate::app) fn shuffle_play_target(
        &mut self,
        lib_idx: usize,
        explicit_folder: Option<String>,
    ) {
        // Defensive bounds check; the shell also
        // derives a fresh `lib_idx`, so a synchronous tab change can race in).
        if lib_idx >= self.libs.len() {
            return;
        }
        let parent_id = match explicit_folder {
            Some(id) => id,
            None => {
                let lib = &self.libs[lib_idx];
                lib.nav_stack
                    .last()
                    .map(|l| l.parent_id.clone())
                    .unwrap_or_else(|| lib.library.id.clone())
            }
        };
        // Delegate to the same fetch the context menu's Shuffle action uses
        // (`ContextAction::ShuffleFolder` -> `shuffle_folder`), rather than
        // duplicating this logic against `get_all_videos_recursive`, which
        // only requests Episode/Movie/Video types and so silently excludes
        // Audio -- Ctrl+S on a music album (all-Audio contents) always
        // fetched zero items and reported "Nothing to shuffle" even though
        // the album had playable tracks, while the context menu (already on
        // `get_all_playable_recursive`, which includes Audio) worked fine.
        self.shuffle_folder(lib_idx, &parent_id);
    }

    /// Expands the focused artist's ordered album leaves into one playable
    /// sequence. Each album is normalized with the same non-folder filtering
    /// and natural track ordering as the existing folder effects; concatenating
    /// those results preserves album/tree order without replacing the queue per
    /// album.
    pub(in crate::app) fn play_music_albums(&mut self, albums: Vec<EmbyItem>, shuffle: bool) {
        let Some(client) = self.emby_client() else {
            self.flash(
                "Emby is unavailable".into(),
                crate::app::notify_actions::ToastSeverity::Warning,
            );
            return;
        };
        let client = client.lock().unwrap();
        let mut items = Vec::new();
        for album in albums {
            match client.get_all_playable_recursive(&album.id) {
                Ok(mut album_items) => {
                    sort_playable_items(&mut album_items);
                    items.extend(album_items);
                }
                Err(e) => {
                    drop(client);
                    self.flash(format!("Couldn't load folder: {e}"), ToastSeverity::Error);
                    return;
                }
            }
        }
        drop(client);
        if items.is_empty() {
            self.flash(
                if shuffle {
                    "Nothing to shuffle"
                } else {
                    "Nothing to play"
                }
                .into(),
                ToastSeverity::Error,
            );
            return;
        }
        let source = if shuffle {
            items.shuffle(&mut rand::rng());
            crate::config::QueueSource::Shuffle
        } else {
            crate::config::QueueSource::Unknown
        };
        // Keep the Library focused: `play_items_routed` only moves focus when
        // the caller is not already in the Library panel. The gate defers the
        // single composed replacement and its save to `run_replacement`.
        self.request_queue_replacement(
            PendingQueueAction::PlayItems {
                items,
                start_idx: 0,
                source,
                autostart: true,
            },
            ReplacementExecutor::Routed(RoutedReplacementPrep::MusicAlbums),
        );
    }

    /// `collection_type` is the source library's Emby collection type; it
    /// labels the confirmed replacement with `QueueSource::Collection`. The
    /// source is set inside the gated confirmed path (via the action), never
    /// here, so cancelling the prompt leaves the queue source unchanged
    /// (design D4).
    pub(in crate::app) fn play_folder(&mut self, folder_id: &str, collection_type: String) {
        let Some(client) = self.emby_client() else {
            self.flash(
                "Emby is unavailable".into(),
                crate::app::notify_actions::ToastSeverity::Warning,
            );
            return;
        };
        let client = client.lock().unwrap();
        match client.get_all_playable_recursive(folder_id) {
            Ok(mut items) => {
                sort_playable_items(&mut items);
                if items.is_empty() {
                    drop(client);
                    self.flash("Nothing to play".into(), ToastSeverity::Error);
                    return;
                }
                drop(client);
                self.request_queue_replacement(
                    PendingQueueAction::PlayItems {
                        items,
                        start_idx: 0,
                        source: crate::config::QueueSource::Collection { collection_type },
                        autostart: true,
                    },
                    ReplacementExecutor::Routed(RoutedReplacementPrep::Folder),
                );
            }
            Err(e) => {
                drop(client);
                self.flash(format!("Couldn't load folder: {e}"), ToastSeverity::Error);
            }
        }
    }

    pub(crate) fn is_tvshows_library(&self, lib_idx: usize) -> bool {
        self.libs[lib_idx].library.collection_type == "tvshows"
    }

    /// Whether the given Emby library is a tvshows library. The index
    /// arrives explicitly from the shuffle chain (`shuffle_play_selected` /
    /// `execute_context_action` pass the library the folder was reached
    /// through), so this no longer reads the selected tab. Bounds-misses
    /// return false (defensive; never substitute library zero).
    pub(in crate::app) fn active_lib_is_tvshows(&self, lib_idx: usize) -> bool {
        lib_idx < self.libs.len() && self.is_tvshows_library(lib_idx)
    }

    pub(in crate::app) fn shuffle_folder(&mut self, lib_idx: usize, folder_id: &str) {
        // TV libraries shuffle from a video-only fetch (Episode/Movie/Video)
        // so a season/series shuffle can't pull in stray Audio items (e.g.
        // theme songs); every other library type keeps the broader
        // playable-items fetch used for enqueue/play-all, which does
        // include Audio (needed for music libraries -- see the bug this
        // replaced).
        let is_tvshows = self.active_lib_is_tvshows(lib_idx);
        let Some(client) = self.emby_client() else {
            self.flash(
                "Emby is unavailable".into(),
                crate::app::notify_actions::ToastSeverity::Warning,
            );
            return;
        };
        let client = client.lock().unwrap();
        let fetch = if is_tvshows {
            client.get_all_videos_recursive(folder_id)
        } else {
            client.get_all_playable_recursive(folder_id)
        };
        match fetch {
            Ok(mut items) => {
                items.retain(|i| !i.is_folder);
                if items.is_empty() {
                    drop(client);
                    self.flash("Nothing to shuffle".into(), ToastSeverity::Error);
                    return;
                }
                items.shuffle(&mut rand::rng());
                drop(client);
                self.request_queue_replacement(
                    PendingQueueAction::PlayItems {
                        items,
                        start_idx: 0,
                        source: crate::config::QueueSource::Shuffle,
                        autostart: true,
                    },
                    ReplacementExecutor::Routed(RoutedReplacementPrep::ShuffleFolder),
                );
            }
            Err(e) => {
                drop(client);
                self.flash(format!("Couldn't load folder: {e}"), ToastSeverity::Error);
            }
        }
    }
}
