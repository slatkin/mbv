//! Shell wiring for the Movies/HomeVideos/Generic embedded content owner
//! (`EmbyLibraryContent`, task 6.1, design D2). Mirrors `shell_home_content.rs`'s
//! shape: the owner lives inside the mounted `LibraryPanel`, addressed by
//! `LibraryKey::Service { .. }`, and the shell projects Model-owned
//! browse snapshots into it at the same writer seams `shell_emby_library.rs`
//! already calls for TV's embedded `TvContent`
//! (the former standalone browser lifecycle) — this file supplies the
//! three functions take for the three migrated kinds, so every existing
//! writer call site keeps working unchanged for both paths.
//!
//! Typed effects need no new dispatch: this owner emits the same
//! `ShellRequest::Browser*`/`EmbyLibrary*` messages the mounted
//! `EmbyLibraryContent` did, and `shell_emby_library.rs::handle_emby_library_request` /
//! `shell_messages.rs`'s dispatch are keyed only by the active tab, not by
//! which component or owner sent the message.

use super::components::emby_library_content::EmbyLibraryIdentity;
use super::components::emby_library_content::{BrowserOwnerPush, EmbyLibraryContent};
use super::components::{LibraryKey, LibraryKind};
use super::shell::Model;
use super::TabSelection;
use mbv_core::config::ServiceKind;

impl Model {
    /// The active tab's migrated-owner identity, when the active Emby
    /// library's kind is one of the three this owner serves (design D2's
    /// `LibraryKey::Service`). `None` for every other tab, including a TV or
    /// Music library (still served by their own mounted components) or a
    /// non-`EmbyLibrary` tab.
    pub(super) fn active_emby_library_owner(&self) -> Option<(usize, LibraryKey, LibraryKind)> {
        let TabSelection::EmbyLibrary(index) = self.app.tab else {
            return None;
        };
        let library = self.app.libs.get(index)?;
        let kind = LibraryKind::from_collection_type(&library.library.collection_type);
        if !matches!(
            kind,
            LibraryKind::Generic | LibraryKind::Movies | LibraryKind::HomeVideos
        ) {
            return None;
        }
        let key = LibraryKey::Service {
            service: ServiceKind::Emby,
            library_id: library.library.id.clone(),
            kind,
        };
        Some((index, key, kind))
    }

    /// Mutably borrow the owner installed for `key` inside the mounted
    /// `LibraryPanel`, creating it with `kind` on first reach (mirrors
    /// `update_home_owner`).
    fn update_emby_library_owner<R>(
        &mut self,
        key: &LibraryKey,
        kind: LibraryKind,
        f: impl FnOnce(&mut EmbyLibraryContent) -> R,
    ) -> Option<R> {
        self.update_library_owner(key.clone(), || Box::new(EmbyLibraryContent::new(kind)), f)
    }

    /// The browse identity of library `index`'s current level (mirrors
    /// `shell_emby_library.rs::browse_identity`, reused verbatim as the shared
    /// `EmbyLibraryIdentity` shape).
    fn emby_library_owner_identity(&self, index: usize) -> EmbyLibraryIdentity {
        let lib = &self.app.libs[index];
        let level = lib.nav_stack.last();
        EmbyLibraryIdentity {
            depth: lib.nav_stack.len(),
            parent_id: level.map(|l| l.parent_id.clone()).unwrap_or_default(),
            letter_filter: level.and_then(|l| l.letter_filter.as_ref().map(|f| f.index)),
            sort_by: level.map(|l| l.sort_by.clone()).unwrap_or_default(),
            sort_order: level.map(|l| l.sort_order.clone()).unwrap_or_default(),
            unplayed_only: level.is_some_and(|l| l.unplayed_only),
            feed_group: lib
                .feed_home_video
                .as_ref()
                .map(|s| s.selected_group_index()),
        }
    }

    /// Event-scoped content projection for the migrated owner (mirrors
    /// former standalone projection's body): mirrors the shell-owned browse
    /// snapshot — the feed/home-video group's selected items when active,
    /// the ordinary nav-stack level otherwise — into the owner, re-seeding
    /// position only on a real identity change, then prefetches nearby
    /// Movie posters at the owner's now-authoritative cursor (#287,
    /// `App::fetch_nearby_movie_posters`, moved here from
    /// the old draw path since that call no longer
    /// runs for these three kinds).
    pub(super) fn push_emby_library_owner_content(
        &mut self,
        index: usize,
        key: &LibraryKey,
        kind: LibraryKind,
    ) {
        // Group-level loading belongs to the shell projection seam, not the
        // render path: ensure the owner receives complete content before it
        // is painted.
        self.app.ensure_feed_home_video_group_level(index);
        let feed_group_view = self.app.is_feed_home_video_group_view(index);
        let show_letter_pills = self.app.should_show_letter_pills(index);
        let (items, total_count, library_total, letter_filter, loading, cursor, scroll) =
            if feed_group_view {
                let items = self.app.feed_home_video_selected_items(index);
                let (cursor, scroll) = self.app.libs[index]
                    .feed_home_video
                    .as_ref()
                    .map(|state| (state.video_cursor, state.video_scroll))
                    .unwrap_or((0, 0));
                let loading = self.app.libs[index]
                    .feed_home_video
                    .as_ref()
                    .map(|state| state.loading)
                    .or_else(|| {
                        self.app.libs[index]
                            .nav_stack
                            .first()
                            .map(|root| root.loading)
                    })
                    .unwrap_or(false);
                let total_count = items.len();
                (items, total_count, None, None, loading, cursor, scroll)
            } else {
                let cursor = self.app.libs[index]
                    .nav_stack
                    .last()
                    .map_or(0, |l| l.resting().cursor());
                let scroll = self.app.libs[index]
                    .nav_stack
                    .last()
                    .map_or(0, |l| l.resting().scroll());
                let ctx = self.app.library_list_render_ctx(index, cursor);
                (
                    ctx.items,
                    ctx.total_count,
                    ctx.library_total,
                    ctx.letter_filter,
                    ctx.loading,
                    cursor,
                    scroll,
                )
            };
        // Keep the painted names and launch identities aligned from one
        // structural source; the snapshot resolves a selected group to its
        // folder content ID, never to the display name.
        let (feed_groups, feed_group_ids): (Vec<String>, Vec<String>) = self.app.libs[index]
            .feed_home_video
            .as_ref()
            .map(|s| {
                s.groups
                    .iter()
                    .map(|g| (g.folder.name.clone(), g.folder.id.clone()))
                    .unzip()
            })
            .unwrap_or_default();
        let feed_group_cursor = self.app.feed_home_video_selected_group_index(index);
        let poster_window = items.clone();
        let latest_items = self
            .tv_latest_snapshots
            .get(&self.app.libs[index].library.id)
            .map(|snapshot| {
                snapshot
                    .items
                    .iter()
                    .filter_map(|item| item.as_emby().cloned())
                    .collect()
            })
            .unwrap_or_default();
        let push = BrowserOwnerPush {
            items,
            latest_items,
            total_count,
            library_total,
            letter_filter,
            loading,
            group_pills: feed_group_view,
            show_letter_pills,
            feed_groups,
            feed_group_ids,
            feed_group_cursor,
        };
        let identity = self.emby_library_owner_identity(index);
        let landed_cursor = self.update_emby_library_owner(key, kind, |owner| {
            owner.set_content(push);
            if owner.note_browse_identity(identity) {
                owner.apply_position(cursor, scroll);
            }
            owner.cursor()
        });
        if let Some(cursor) = landed_cursor {
            self.app.fetch_nearby_movie_posters(&poster_window, cursor);
        }
    }

    pub(super) fn push_active_emby_library_owner_content(&mut self) {
        if let Some((index, key, kind)) = self.active_emby_library_owner() {
            self.push_emby_library_owner_content(index, &key, kind);
        }
    }

    pub(super) fn active_emby_library_owner_is_latest(&self) -> bool {
        let Some((_, key, _)) = self.active_emby_library_owner() else {
            return false;
        };
        self.application
            .get_component(&super::components::ComponentId::Library)
            .and_then(|component| {
                component
                    .as_any()
                    .downcast_ref::<super::components::library_panel::LibraryPanel>()
            })
            .and_then(|panel| panel.owner(&key))
            .and_then(|owner| owner.as_any().downcast_ref::<EmbyLibraryContent>())
            .is_some_and(EmbyLibraryContent::latest_mode)
    }
}
