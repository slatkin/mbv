//! Library-scoped daemon routing resolvers (#223).
//!
//! Extracted out of `app.rs` (which had grown very large) as a review
//! follow-up: these methods are the pure route-resolution layer -- they
//! decide *which* daemon (if any) a play/enqueue action should target, but
//! don't themselves perform the connect/swap (that's `apply_route_for_playback`
//! and `switch_to_library_route`, which stay in `app.rs` since they're
//! entangled with the same suspend/restore machinery the Sessions-panel
//! direct-remote path uses).

#[cfg(test)]
use crate::app::TabSelection;
use crate::app::{App, Duration, Instant, PanelFocus};

/// How long a `library_route_cache` entry (#223) stays trusted before a
/// repeat lookup re-resolves from scratch, so a mid-session library
/// reorganization on the Emby server self-heals without requiring an app
/// restart (post-grilling revision item 5; candidate 15-30 minutes,
/// chosen at the low end of that range as a session-lifetime TUI cache).
const LIBRARY_ROUTE_CACHE_TTL: Duration = Duration::from_secs(15 * 60);
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
    pub(in crate::app) fn in_non_library_thin_client_mode(&self) -> bool {
        self.connected_session_id.is_some()
            || (self.player.is_remote() && self.active_route.is_none())
    }

    /// Resolves the configured library route for a library name (#256):
    /// a pure, synchronous `library_routes` config read -- no `/Sessions`
    /// call, ever, on this path. Returns `(lowercased_library_name,
    /// endpoint)` when the config has a well-formed `tcp://` entry for
    /// this library. A missing or malformed entry is not an error (no
    /// warning flashed, beyond `resolve_library_route`'s own log line for
    /// the malformed case) -- it's the expected, common case of "not
    /// routed"; #222's existing fallback (stay local, no hard error)
    /// already covers it via the `None` return.
    pub(in crate::app) fn resolve_route_for_library(
        &mut self,
        library_name: &str,
    ) -> Option<(String, mbv_core::remote_player::DaemonEndpoint)> {
        let name = library_name.trim();
        if name.is_empty() {
            log::info!(target: "library_route", "route resolution skipped: empty library name");
            return None;
        }
        let Some(raw) = self.library_routes.get(&name.to_lowercase()) else {
            log::info!(target: "library_route", "configured library missing library={name:?}");
            return None;
        };
        let Ok(endpoint @ mbv_core::remote_player::DaemonEndpoint::Tcp(_)) =
            mbv_core::remote_player::DaemonEndpoint::parse(raw)
        else {
            log::warn!(target: "library_route", "malformed endpoint library={name:?} endpoint={raw:?}; accepted shape is tcp://host:port");
            return None;
        };
        log::info!(target: "library_route", "accepted endpoint library={name:?} endpoint={endpoint}");
        Some((name.to_lowercase(), endpoint))
    }

    /// Nav-context route resolution for library-scoped views (Library
    /// tab, Album/Artist drill-down, in-library search) --
    /// the active library is already known from navigation state
    /// (`LibraryTab::library`), so no network call is needed (#223).
    pub(in crate::app) fn route_for_active_library_view(
        &mut self,
        lib_idx: usize,
    ) -> Option<(String, mbv_core::remote_player::DaemonEndpoint)> {
        let name = self.libs.get(lib_idx)?.library.name.clone();
        self.resolve_route_for_library(&name)
    }

    /// Cross-library aggregate view (Continue Watching/Next Up, Favorites)
    /// route resolution: walks the item's ancestor chain via
    /// `EmbyClient::get_ancestors` to find the owning library
    /// (`CollectionFolder`), then matches it against `library_routes`.
    /// A *successful* lookup (whether it finds an owning library or
    /// confirms there isn't one) is cached per item id for the session, so
    /// a repeated play/enqueue of the same item never re-fetches. A
    /// *failed* lookup (transient error) is never cached, so it retries
    /// on the item's next play/enqueue attempt instead of being stuck at
    /// `None` until the process restarts (#223, post-grilling revision).
    pub(in crate::app) fn route_for_item_via_ancestors(
        &mut self,
        item_id: &str,
    ) -> Option<(String, mbv_core::remote_player::DaemonEndpoint)> {
        // No routes configured at all -- this must be a true no-op for the
        // common case (no `[library_routes]` in config.toml), not just "no
        // match": every other resolver in this file is a synchronous,
        // no-network lookup, but this one's `get_ancestors` fallback is a
        // real HTTP round-trip. Without this guard, every first play of a
        // distinct Home-tab item (Continue Watching/Next Up/Favorites)
        // would pay a blocking network call that can never resolve to
        // anything for a user who never opted into library routing.
        if self.library_routes.is_empty() {
            log::info!(target: "library_route", "route table empty item_id={item_id:?}; staying local");
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
                log::info!(target: "library_route", "ancestor cache hit item_id={item_id:?} library={cached:?}");
                return cached
                    .clone()
                    .and_then(|name| self.resolve_route_for_library(&name));
            }
            log::info!(target: "library_route", "ancestor cache expired item_id={item_id:?}");
            // Expired -- fall through and re-resolve as a normal cache miss,
            // so a mid-session library reorganization on the Emby server
            // self-heals without requiring an app restart.
        }
        log::info!(target: "library_route", "ancestor cache miss item_id={item_id:?}");
        let ancestors = {
            let client = self.emby_client()?;
            let result = client.lock().unwrap().get_ancestors(item_id);
            result
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
        log::info!(target: "library_route", "ancestor resolution succeeded item_id={item_id:?} library={library_name:?}");
        library_name.and_then(|name| self.resolve_route_for_library(&name))
    }

    /// Resolves the daemon route (if any) that a play/enqueue of `item`
    /// should target: nav-scoped lookup for library-scoped views
    /// (`tab_idx >= 2` -- Library/Album/Artist/in-library search),
    /// ancestor-lookup for cross-library aggregate views (`tab_idx == 0`
    /// -- Home tab). No match in either case means local playback,
    /// unaffected (#223).
    ///
    /// Queue focus has no library of its own. If a route is already active,
    /// keep it rather than re-resolving from nav context (there is none);
    /// otherwise resolve from the queued item itself, so a restored queue
    /// can still take its configured route when startup auto-reconnect
    /// missed a live-but-not-yet-visible target session.
    pub(in crate::app) fn resolve_route_for_play(
        &mut self,
        item: &mbv_core::api::EmbyItem,
    ) -> Option<(String, mbv_core::remote_player::DaemonEndpoint)> {
        log::info!(target: "library_route", "route resolution item_id={:?} item_name={:?} library_tab={}", item.id, item.name, self.tab.to_position_with_counts(self.libs.len(), self.feeds_tab_pos()));
        if matches!(self.effective_panel_focus(), PanelFocus::Queue) {
            log::info!(target: "library_route", "resolution path=queue item_id={:?}", item.id);
            self.active_route
                .clone()
                .and_then(|name| self.resolve_route_for_library(&name))
                .or_else(|| self.route_for_item_via_ancestors(&item.id))
        } else if self.tab.is_home() {
            log::info!(target: "library_route", "resolution path=ancestor item_id={:?}", item.id);
            self.route_for_item_via_ancestors(&item.id)
        } else {
            let lib_idx = self.tab.emby_library_index().unwrap();
            log::info!(target: "library_route", "resolution path=power-library item_id={:?} lib_idx={lib_idx}", item.id);
            self.route_for_active_library_view(lib_idx)
        }
    }

    /// Route resolution specifically for `do_enqueue_folder` (#223 follow-up,
    /// see "Design decisions carried forward from review" above): the item
    /// being enqueue-recursive'd may itself *be* a library root
    /// (`item_type == "CollectionFolder"`), in which case `get_ancestors`
    /// returns no ancestor above it and a plain ancestor-lookup resolver
    /// always yields `None`. Check the item's own type first; only fall
    /// back to ancestor lookup for a non-root folder.
    pub(in crate::app) fn resolve_route_for_enqueue_folder(
        &mut self,
        item: &mbv_core::api::EmbyItem,
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
        // #256: resolution is now a pure config read -- no live session
        // lookup, no SESSIONS_LOAD_OVERRIDE seam needed at all.
        let mut app = make_app_stub();
        app.library_routes
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
    fn resolve_route_for_library_returns_none_for_a_malformed_endpoint() {
        // #256: a config value that isn't a valid tcp:// endpoint (stale
        // pre-#256 device name, unix://, etc.) resolves to None rather
        // than being routed or panicking.
        let mut app = make_app_stub();
        app.library_routes
            .insert("music".to_string(), "living-room-pc".to_string());

        assert_eq!(app.resolve_route_for_library("Music"), None);
    }

    #[test]
    fn route_for_active_library_view_uses_nav_state_no_network() {
        // The active library is already known from nav state, and (#256)
        // resolving its routed endpoint is now a pure config read too --
        // this is unconditionally no-network, not just "no get_ancestors
        // round-trip".
        let mut app = make_app_stub();
        app.library_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
        let mut lib_item = make_item("Music", "CollectionFolder");
        lib_item.id = "lib-music".to_string();
        app.libs.push(LibraryTab::new(lib_item));

        let resolved = app.route_for_active_library_view(0);

        assert_eq!(resolved.map(|(name, _)| name), Some("music".to_string()));
    }

    #[test]
    fn resolve_route_for_play_does_not_inherit_library_route_from_queue_side() {
        let mut app = make_app_stub();
        app.library_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
        let mut lib_item = make_item("Music", "CollectionFolder");
        lib_item.id = "lib-music".to_string();
        app.libs.push(LibraryTab::new(lib_item));
        app.panel_focus = PanelFocus::Queue;
        app.tab = TabSelection::EmbyLibrary(0);
        let mut item = make_item("Song", "Audio");
        item.id = "song-1".to_string();

        assert_eq!(app.resolve_route_for_play(&item), None);
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
        app.library_routes
            .insert("music".to_string(), "living-room-pc".to_string());
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
        // Regression guard: an empty `library_routes` (the common case for
        // a user who never opted into library routing at all) must be a
        // genuine no-op -- no `get_ancestors` HTTP call, not just "no
        // match after a network round-trip." If this guard were missing,
        // every first play/enqueue of a distinct Home-tab item would pay
        // a blocking network call that could never resolve to anything.
        let mut app = make_app_stub();
        assert!(app.library_routes.is_empty());

        let resolved = app.route_for_item_via_ancestors("item-1");

        assert_eq!(resolved, None);
        // No lookup was even attempted, successful or not -- nothing gets
        // cached, unlike the failed-lookup case above.
        assert!(!app.library_route_cache.contains_key("item-1"));
    }

    #[test]
    fn resolve_route_for_play_does_not_panic_from_the_queue_tab() {
        // Regression guard: queue focus (`PanelFocus::Queue`) has no library
        // of its own -- the item being played is already part of whatever
        // queue is current, so `resolve_route_for_play` must fall through to
        // "keep the current `active_route`" instead of either panicking or
        // wrongly resolving a nav-scoped library.
        let mut app = make_app_stub();
        app.panel_focus = PanelFocus::Queue;
        let mut item = make_item("Song", "Audio");
        item.id = "song-1".to_string();

        // Local queue: no route to keep.
        assert_eq!(app.resolve_route_for_play(&item), None);

        // Already routed: the Queue tab must not clear or re-resolve the
        // route out from under an in-progress routed queue.
        app.library_routes
            .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
        app.active_route = Some("music".to_string());
        let resolved = app.resolve_route_for_play(&item).map(|(name, _)| name);

        assert_eq!(resolved, Some("music".to_string()));
    }
}
