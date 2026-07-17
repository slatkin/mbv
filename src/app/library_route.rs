//! Library-scoped daemon routing resolvers (#223).
//!
//! Extracted out of `app/mod.rs` (which had grown very large) as a review
//! follow-up: these methods are the pure route-resolution layer -- they
//! decide *which* daemon (if any) a play/enqueue action should target, but
//! don't themselves perform the connect/swap (that's `apply_route_for_playback`
//! and `switch_to_library_route`, which stay in `app/mod.rs` since they're
//! entangled with the same suspend/restore machinery the Sessions-panel
//! direct-remote path uses).

use super::*;

/// How long a `library_route_cache` entry (#223) stays trusted before a
/// repeat lookup re-resolves from scratch, so a mid-session library
/// reorganization on the Emby server self-heals without requiring an app
/// restart (post-grilling revision item 5; candidate 15-30 minutes,
/// chosen at the low end of that range as a session-lifetime TUI cache).
const LIBRARY_ROUTE_CACHE_TTL: Duration = Duration::from_secs(15 * 60);
/// How long a malformed-`daemon_routes`-entry status warning stays
/// suppressed for a given library name before it's allowed to flash again
/// (`daemon_routes_warned`). A one-time-per-process suppression could mean
/// a user who never saw the first toast (e.g. it fired from a background
/// Home-tab ancestor lookup) never gets told why a library never routes,
/// even after navigating straight to it and trying to play. Re-arming
/// periodically keeps the warning from spamming on every resolution
/// attempt while still resurfacing it on a timescale a user would notice.
const DAEMON_ROUTE_WARNING_COOLDOWN: Duration = Duration::from_secs(5 * 60);
/// Soft cap on `library_route_cache` size. The cache is otherwise
/// unbounded (one entry per distinct item id ever played/enqueued from a
/// cross-library aggregate view), which could grow without limit over a
/// very long session with a large, varied Continue Watching/Favorites
/// history. Reaching this cap triggers an expired-entry prune (see
/// `route_for_item_via_ancestors`) rather than a hard eviction, so it's a
/// backstop against unbounded growth, not a working-set limit under
/// normal use.
const LIBRARY_ROUTE_CACHE_PRUNE_THRESHOLD: usize = 2000;

impl App {
    /// True when `self.player` is remote for a reason other than library
    /// routing (#223): a Sessions-panel attached session, or a
    /// non-library-route direct-remote/local-daemon connection. Library
    /// routing must never engage -- for play or enqueue -- while this is
    /// true, since it would otherwise swap `self.player` out from under a
    /// connection it doesn't own. Still lets library routing run when
    /// `active_route` is already `Some(..)` (so it can re-evaluate, swap,
    /// or restore -- that's its job); only skips it when the current
    /// remote state belongs to a different, non-library-route mechanism.
    /// Consolidated from three call sites (`play_item`, `play_items_routed`,
    /// `enqueue_route_conflict`) that previously duplicated this condition
    /// verbatim.
    pub(super) fn in_non_library_thin_client_mode(&self) -> bool {
        self.connected_session_id.is_some()
            || (self.player.is_remote() && self.active_route.is_none())
    }

    /// Resolves the configured daemon route for a library name (#223):
    /// looks up `daemon_routes` (exact match, then `"*"` wildcard) and
    /// parses the endpoint string. Returns `(lowercased_library_name,
    /// endpoint)` on a match with a valid endpoint. A malformed endpoint
    /// string is logged and treated as no match, rather than failing the
    /// whole app -- one bad `daemon_routes` entry never blocks other
    /// routes or local playback. Also surfaces a one-time status-bar
    /// warning (`daemon_routes_warned` gates it to once per bad library
    /// name per process, since this method is called on every resolution
    /// attempt) so a malformed entry is visible in app status, not just
    /// diagnosable in logs (#223, post-grilling revision item 3).
    pub(super) fn resolve_route_for_library(
        &mut self,
        library_name: &str,
    ) -> Option<(String, mbv_core::remote_player::DaemonEndpoint)> {
        let name = library_name.trim();
        if name.is_empty() {
            return None;
        }
        let raw = mbv_core::config::resolve_daemon_route(&self.daemon_routes, name)?;
        match mbv_core::remote_player::DaemonEndpoint::parse(raw) {
            Ok(endpoint) => Some((name.to_lowercase(), endpoint)),
            Err(e) => {
                log::warn!(
                    target: "library_route",
                    "daemon_routes entry for library {name:?} has an invalid endpoint {raw:?}: {e}"
                );
                let warn_key = name.to_lowercase();
                let now = Instant::now();
                let should_flash = match self.daemon_routes_warned.get(&warn_key) {
                    Some(last_flashed) => {
                        now.duration_since(*last_flashed) >= DAEMON_ROUTE_WARNING_COOLDOWN
                    }
                    None => true,
                };
                if should_flash {
                    self.daemon_routes_warned.insert(warn_key, now);
                    self.flash_status_high(format!(
                        "daemon_routes entry for library {name:?} is invalid ({e}); using local playback"
                    ));
                }
                None
            }
        }
    }

    /// Nav-context route resolution for library-scoped views (Library
    /// tab, Power View, Album/Artist drill-down, in-library search) --
    /// the active library is already known from navigation state
    /// (`LibraryTab::library`), so no network call is needed (#223).
    pub(super) fn route_for_active_library_view(
        &mut self,
        lib_idx: usize,
    ) -> Option<(String, mbv_core::remote_player::DaemonEndpoint)> {
        let name = self.libs.get(lib_idx)?.library.name.clone();
        self.resolve_route_for_library(&name)
    }

    /// Cross-library aggregate view (Continue Watching/Next Up, Favorites)
    /// route resolution: walks the item's ancestor chain via
    /// `EmbyClient::get_ancestors` to find the owning library
    /// (`CollectionFolder`), then matches it against `daemon_routes`.
    /// A *successful* lookup (whether it finds an owning library or
    /// confirms there isn't one) is cached per item id for the session, so
    /// a repeated play/enqueue of the same item never re-fetches. A
    /// *failed* lookup (transient error) is never cached, so it retries
    /// on the item's next play/enqueue attempt instead of being stuck at
    /// `None` until the process restarts (#223, post-grilling revision).
    pub(super) fn route_for_item_via_ancestors(
        &mut self,
        item_id: &str,
    ) -> Option<(String, mbv_core::remote_player::DaemonEndpoint)> {
        // No routes configured at all -- this must be a true no-op for the
        // common case (no `[daemon_routes]` in config.toml), not just "no
        // match": every other resolver in this file is a synchronous,
        // no-network lookup, but this one's `get_ancestors` fallback is a
        // real HTTP round-trip. Without this guard, every first play of a
        // distinct Home-tab item (Continue Watching/Next Up/Favorites)
        // would pay a blocking network call that can never resolve to
        // anything for a user who never opted into library routing.
        if self.daemon_routes.is_empty() {
            return None;
        }
        if self.library_route_cache.len() >= LIBRARY_ROUTE_CACHE_PRUNE_THRESHOLD {
            // Backstop against unbounded growth over a very long session:
            // drop everything already past its TTL before doing anything
            // else, rather than growing forever (#223 review follow-up).
            // Checked on every call (not just before an insert) so a
            // string of failed lookups -- which never insert -- can't
            // keep the cache pinned above the threshold indefinitely.
            let now = Instant::now();
            self.library_route_cache.retain(|_, (_, cached_at)| {
                now.duration_since(*cached_at) < LIBRARY_ROUTE_CACHE_TTL
            });
        }
        if let Some((cached, cached_at)) = self.library_route_cache.get(item_id) {
            if Instant::now().duration_since(*cached_at) < LIBRARY_ROUTE_CACHE_TTL {
                return cached
                    .clone()
                    .and_then(|name| self.resolve_route_for_library(&name));
            }
            // Expired -- fall through and re-resolve as a normal cache miss,
            // so a mid-session library reorganization on the Emby server
            // self-heals without requiring an app restart.
        }
        let ancestors = {
            let client = self.client.lock().unwrap();
            client.get_ancestors(item_id)
        };
        let library_name = match ancestors {
            Ok(chain) => chain
                .into_iter()
                .find(|a| a.item_type == "CollectionFolder")
                .map(|a| a.name),
            Err(e) => {
                log::warn!(
                    target: "library_route",
                    "get_ancestors failed for item {item_id:?}: {e}"
                );
                // Per #223's post-grilling revision: a transient lookup
                // failure is never cached -- only a successful
                // `get_ancestors` call (whether it finds an owning
                // library or confirms there isn't one) gets memoized.
                // A failed lookup retries on the item's next
                // play/enqueue attempt instead of being stuck at `None`
                // until the process restarts.
                return None;
            }
        };
        self.library_route_cache
            .insert(item_id.to_string(), (library_name.clone(), Instant::now()));
        library_name.and_then(|name| self.resolve_route_for_library(&name))
    }

    /// Resolves the daemon route (if any) that a play/enqueue of `item`
    /// should target: nav-scoped lookup for library-scoped views
    /// (`tab_idx >= 2` -- Library/Power/Album/Artist/in-library search),
    /// ancestor-lookup for cross-library aggregate views (`tab_idx == 0`
    /// -- Home tab). No match in either case means local playback,
    /// unaffected (#223).
    ///
    /// `tab_idx == 1` is the Queue tab -- it has no library of its own
    /// (`lib_tab_offset()` is `2`, so a bare `tab_idx - lib_tab_offset()`
    /// would underflow and panic here, unlike `enqueue_selected`'s existing
    /// `tab_idx == 0` / `tab_idx >= 2` split which already avoids this).
    /// An item played from the Queue tab is already part of whatever queue
    /// is current, so this keeps whatever route is already active rather
    /// than re-resolving from nav context (there is none) or treating "no
    /// nav-scoped resolution" as "no route", which would incorrectly
    /// restore to local every time the Queue tab is used to play/jump
    /// within an already-routed queue.
    pub(super) fn resolve_route_for_play(
        &mut self,
        item: &mbv_core::api::MediaItem,
    ) -> Option<(String, mbv_core::remote_player::DaemonEndpoint)> {
        if self.tab_idx == 0 {
            self.route_for_item_via_ancestors(&item.id)
        } else if self.tab_idx >= 2 {
            let lib_idx = self.tab_idx - self.lib_tab_offset();
            self.route_for_active_library_view(lib_idx)
        } else {
            self.active_route
                .clone()
                .and_then(|name| self.resolve_route_for_library(&name))
        }
    }

    /// Route resolution specifically for `do_enqueue_folder` (#223 follow-up,
    /// see "Design decisions carried forward from review" above): the item
    /// being enqueue-recursive'd may itself *be* a library root
    /// (`item_type == "CollectionFolder"`), in which case `get_ancestors`
    /// returns no ancestor above it and a plain ancestor-lookup resolver
    /// always yields `None`. Check the item's own type first; only fall
    /// back to ancestor lookup for a non-root folder.
    pub(super) fn resolve_route_for_enqueue_folder(
        &mut self,
        item: &mbv_core::api::MediaItem,
    ) -> Option<String> {
        if item.item_type == "CollectionFolder" {
            return self
                .resolve_route_for_library(&item.name)
                .map(|(name, _)| name);
        }
        self.route_for_item_via_ancestors(&item.id)
            .map(|(name, _)| name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{make_app_stub, make_item};
    use crate::app::LibraryTab;

    #[test]
    fn resolve_route_for_library_matches_case_insensitively() {
        let mut app = make_app_stub();
        app.daemon_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
        let resolved = app.resolve_route_for_library("Music");
        assert_eq!(
            resolved,
            Some((
                "music".to_string(),
                mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:9000".parse().unwrap())
            ))
        );
    }

    #[test]
    fn resolve_route_for_library_returns_none_when_unconfigured() {
        let mut app = make_app_stub();
        assert_eq!(app.resolve_route_for_library("Movies"), None);
    }

    #[test]
    fn resolve_route_for_library_skips_invalid_endpoint() {
        let mut app = make_app_stub();
        app.daemon_routes
            .insert("music".to_string(), "notascheme://x".to_string());
        assert_eq!(app.resolve_route_for_library("Music"), None);
    }

    #[test]
    fn resolve_route_for_library_flashes_a_malformed_endpoint_warning_once_per_cooldown() {
        // #223 post-grilling revision item 3: a malformed `daemon_routes`
        // entry must be visible in app status (not just diagnosable in
        // logs), but only flashed once per DAEMON_ROUTE_WARNING_COOLDOWN --
        // this method is called on every resolution attempt, so a
        // per-lookup flash would spam the status bar on repeated plays
        // from the same library.
        let mut app = make_app_stub();
        app.daemon_routes
            .insert("music".to_string(), "notascheme://x".to_string());

        app.resolve_route_for_library("Music");
        assert!(app.status.contains("invalid"));

        app.status = String::new();
        app.resolve_route_for_library("Music");
        assert!(
            app.status.is_empty(),
            "a second lookup within the cooldown window must not flash again"
        );
    }

    #[test]
    fn resolve_route_for_library_reflashes_a_malformed_endpoint_warning_after_cooldown() {
        // Fix for a review gap: a one-time-per-process suppression means a
        // user who misses the first toast (e.g. it fired from a
        // background Home-tab ancestor lookup they weren't looking at)
        // would never be told why a library never routes, even after
        // navigating straight to it and trying to play. Re-arming after
        // DAEMON_ROUTE_WARNING_COOLDOWN fixes that.
        let mut app = make_app_stub();
        app.daemon_routes
            .insert("music".to_string(), "notascheme://x".to_string());
        app.daemon_routes_warned.insert(
            "music".to_string(),
            Instant::now() - DAEMON_ROUTE_WARNING_COOLDOWN - Duration::from_secs(1),
        );

        app.resolve_route_for_library("Music");

        assert!(
            app.status.contains("invalid"),
            "warning must resurface once the cooldown has elapsed"
        );
    }

    #[test]
    fn route_for_active_library_view_uses_nav_state_no_network() {
        let mut app = make_app_stub();
        app.daemon_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
        let mut lib_item = make_item("Music", "CollectionFolder");
        lib_item.id = "lib-music".to_string();
        app.libs.push(LibraryTab {
            library: lib_item,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            power_detail_scroll: Default::default(),
            album_track_focus: None,
            artist_header_focus: None,
        });

        let resolved = app.route_for_active_library_view(0);

        assert_eq!(resolved.map(|(name, _)| name), Some("music".to_string()));
    }

    #[test]
    fn route_for_active_library_view_none_for_unrouted_library() {
        let mut app = make_app_stub();
        let mut lib_item = make_item("Movies", "CollectionFolder");
        lib_item.id = "lib-movies".to_string();
        app.libs.push(LibraryTab {
            library: lib_item,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            power_detail_scroll: Default::default(),
            album_track_focus: None,
            artist_header_focus: None,
        });

        assert_eq!(app.route_for_active_library_view(0), None);
    }

    #[test]
    fn route_for_item_via_ancestors_does_not_cache_a_failed_lookup() {
        // Per #223's post-grilling revision (design decision #1): a
        // transient `get_ancestors` failure must NOT be cached -- only a
        // successful call (whether it finds an owning library or confirms
        // there isn't one) gets memoized. A failed lookup retries on the
        // item's next play/enqueue attempt rather than being stuck at
        // `None` until the process restarts.
        let mut app = make_app_stub();
        app.daemon_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
        // No live server in this stub -- `get_ancestors` always errors.
        let first = app.route_for_item_via_ancestors("item-1");
        assert_eq!(first, None);
        assert!(!app.library_route_cache.contains_key("item-1"));

        // A second call must attempt the lookup again (not short-circuit
        // on a cached failure) -- still `None` here since the stub still
        // has no live server, but critically still uncached afterward.
        let second = app.route_for_item_via_ancestors("item-1");
        assert_eq!(second, None);
        assert!(!app.library_route_cache.contains_key("item-1"));
    }

    #[test]
    fn route_for_item_via_ancestors_is_a_true_no_op_when_no_routes_are_configured() {
        // Regression guard: an empty `daemon_routes` (the common case for
        // a user who never opted into library routing at all) must be a
        // genuine no-op -- no `get_ancestors` HTTP call, not just "no
        // match after a network round-trip." If this guard were missing,
        // every first play/enqueue of a distinct Home-tab item would pay
        // a blocking network call that could never resolve to anything.
        let mut app = make_app_stub();
        assert!(app.daemon_routes.is_empty());

        let resolved = app.route_for_item_via_ancestors("item-1");

        assert_eq!(resolved, None);
        // No lookup was even attempted, successful or not -- nothing gets
        // cached, unlike the failed-lookup case above.
        assert!(!app.library_route_cache.contains_key("item-1"));
    }

    #[test]
    fn route_for_item_via_ancestors_does_not_trust_an_expired_cache_entry() {
        // #223 post-grilling revision item 5: a mid-session library
        // reorganization on the Emby server must self-heal after
        // LIBRARY_ROUTE_CACHE_TTL, not require an app restart. Prime the
        // cache with a stale, EXPIRED entry that (if trusted) would
        // resolve to "music" -- then confirm the resolver ignores it and
        // re-attempts the lookup instead (which errors in this stub with
        // no live server, giving `None`), rather than trusting the stale
        // hit and returning the resolved route.
        let mut app = make_app_stub();
        app.daemon_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
        app.library_route_cache.insert(
            "item-1".to_string(),
            (
                Some("music".to_string()),
                Instant::now() - LIBRARY_ROUTE_CACHE_TTL - Duration::from_secs(1),
            ),
        );

        let resolved = app.route_for_item_via_ancestors("item-1");

        assert_eq!(resolved, None);
    }

    #[test]
    fn route_for_item_via_ancestors_prunes_expired_entries_once_the_cache_is_large() {
        // Review follow-up: library_route_cache had no eviction beyond the
        // per-read TTL check, so it could grow unbounded across a long
        // session. Once the cache reaches LIBRARY_ROUTE_CACHE_PRUNE_THRESHOLD,
        // a fresh insert must first drop every already-expired entry
        // rather than growing forever.
        let mut app = make_app_stub();
        app.daemon_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
        let expired_at = Instant::now() - LIBRARY_ROUTE_CACHE_TTL - Duration::from_secs(1);
        for i in 0..LIBRARY_ROUTE_CACHE_PRUNE_THRESHOLD {
            app.library_route_cache.insert(
                format!("stale-{i}"),
                (Some("music".to_string()), expired_at),
            );
        }
        assert_eq!(
            app.library_route_cache.len(),
            LIBRARY_ROUTE_CACHE_PRUNE_THRESHOLD
        );

        // No live server in this stub -- the lookup for "item-1" itself
        // errors and is never cached, but the prune must still have run
        // as a side effect of crossing the threshold.
        app.route_for_item_via_ancestors("item-1");

        assert!(
            app.library_route_cache.len() < LIBRARY_ROUTE_CACHE_PRUNE_THRESHOLD,
            "expired entries must be pruned once the cache crosses the threshold"
        );
    }

    #[test]
    fn resolve_route_for_play_does_not_panic_from_the_queue_tab() {
        // Regression guard: `tab_idx` values are 0 = Home, 1 = Queue tab,
        // 2.. = library tabs (`lib_tab_offset() == 2`, confirmed against
        // `src/app/input.rs`). An `if tab_idx == 0 { .. } else { lib_idx =
        // tab_idx - lib_tab_offset() }` shape (as opposed to `enqueue_selected`'s
        // existing `tab_idx == 0` / `tab_idx >= 2` split) underflows a `usize`
        // subtraction (1 - 2) and panics when called from the Queue tab. The
        // Queue tab has no library of its own -- the item being played is
        // already part of whatever queue is current, so `resolve_route_for_play`
        // must fall through to "keep the current `active_route`" instead of
        // either panicking or wrongly resolving a nav-scoped library.
        let mut app = make_app_stub();
        app.tab_idx = 1;
        let mut item = make_item("Song", "Audio");
        item.id = "song-1".to_string();

        // Local queue: no route to keep.
        assert_eq!(app.resolve_route_for_play(&item), None);

        // Already routed: the Queue tab must not clear or re-resolve the
        // route out from under an in-progress routed queue.
        app.daemon_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
        app.active_route = Some("music".to_string());
        assert_eq!(
            app.resolve_route_for_play(&item).map(|(name, _)| name),
            Some("music".to_string())
        );
    }

    #[test]
    fn resolve_route_for_enqueue_folder_matches_a_library_root_folder_by_its_own_name() {
        // #223 follow-up: `get_ancestors` on a library root returns no
        // `CollectionFolder` ancestor above it (there isn't one), so a plain
        // ancestor-lookup resolver always yields `None` for the library root
        // item itself. `do_enqueue_folder` can receive exactly that item (the
        // user enqueue-recursive's an entire library from its root), so this
        // helper checks the item's own type first.
        let mut app = make_app_stub();
        app.daemon_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
        let mut lib_root = make_item("Music", "CollectionFolder");
        lib_root.id = "lib-music".to_string();

        assert_eq!(
            app.resolve_route_for_enqueue_folder(&lib_root),
            Some("music".to_string())
        );
    }

    #[test]
    fn resolve_route_for_enqueue_folder_falls_back_to_ancestor_lookup_for_a_non_root_folder() {
        let mut app = make_app_stub();
        let mut sub_folder = make_item("Some Album", "MusicAlbum");
        sub_folder.id = "album-1".to_string();
        sub_folder.is_folder = true;

        // No live server in this stub -- `get_ancestors` errors, so this
        // must fall through to the ancestor-lookup path (not treat every
        // folder as a library root) and resolve to `None`, not panic.
        assert_eq!(app.resolve_route_for_enqueue_folder(&sub_folder), None);
    }
}
