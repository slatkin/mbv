# Design

## Context

See `proposal.md` — Why. The relevant existing seams:

- `MusicContent` (`src/app/components/music_content*.rs`, `music_interaction.rs`) is the Grouped Music owner: it interprets local tree chords and pointer gestures, owns tree selection/expansion, and emits typed `ShellRequest`s for anything outside its authority. `MusicTreeBrowser` (`music_tree*.rs`) is the single tree owner/painter.
- `LibraryPanel` (`library_panel/panel.rs`) is the framework focus boundary and owns the pointer-geometry dispatch; it currently intercepts a non-Wide double-click on a hero-bearing browser and opens the Library Hero overlay before the owner sees it.
- The shell owns playback/queue authority: `App::play_album_track` resolves an album's cached playable tracks and starts from a track; `ConfirmModal` + `pending_queue_action` already model "ask, then run a queue replacement".
- Artist detail (`ArtistDetailProjection.track_groups`) is the shell-projected, disc/track-ordered artist Workspace snapshot, retained by `MusicContent` as its context.

## Goals / Non-Goals

**Goals**

- One entry per Hero-bearing tree row (Enter), with expansion state preserved.
- Track rows readable at a glance (numbered like the Workspace rows).
- Artist-Workspace playback spans the artist's in-scope discography, not one album.
- Pointer gestures on the tree behave like every other library list (focus + expand/play).
- Music "Go to Library" lands or reports; no silent no-op.

**Non-Goals**

- Changing album-leaf Enter, album-Workspace playback scope, or the artist action (play/enqueue/shuffle) semantics.
- Changing the tree's filter/search, action resolution, or row painting beyond the track-number label.
- Adding any new tree painter or second owner.

## Decisions

### D1 — One Hero-entry resolution, used by both Enter and Right

The artist-root Hero entry already exists for Right on an expanded root: Wide takes the inline artist Workspace cursor (`enter_artist_workspace_focus`); non-Wide emits the overlay-opening `MusicArtistActivate`. With no tree filter active, Enter uses that same resolution and the unfiltered Enter-toggles-expansion arm is deleted. Unfiltered expansion stays on Left/Right. While filtering, Enter retains the existing filter-local expansion arm and never opens or focuses a Hero.

Alternatives: keep Enter as toggle and add another chord for the Hero (rejected — the user asked for parity with album leaves); make both Enter and Right open the Hero (rejected — Right already owns expand-then-enter and the user chose to keep it).

The non-Wide gate `LibraryContentOwner::hero_overlay_enter_available` must return true for an artist root only when the tree filter is inactive, so the panel's pre-owner Enter path opens the overlay without bypassing filtered interaction. While filtering, both Wide and non-Wide retain the filter's existing local artist-root expansion behavior and do not open or focus a Hero.

### D2 — Track-number formatting lives in one helper

`music_content_workspace.rs` already formats Workspace track rows as `"{number}. {title}"` with an index fallback. Extract that into one label helper and use it both for the Workspace rows and for the tree's `MusicTreeTrack` projection in `set_content` (the tree only receives the label, per its existing "shell-projected cache data only" rule). No numbering logic enters the tree model.

### D3 — Discography playback is one typed intent plus a shell-side resolution

The component emits `ShellRequest::MusicArtistTrackActivate { target: MusicArtistTarget, track_id: String }` from every artist-Workspace activation route: keyboard Enter, `HeroActivate`, and the Workspace row double-click. Both fields are stable opaque identities. The shell resolves the chosen `EmbyItem` and the ordered in-scope track list by flattening `ArtistDetailProjection.track_groups` for the matching cache entry, then sends that list with the chosen track's start index to `play_items_routed`. Tracks before the selected one remain in the queue; playback begins at the selected index and naturally continues through the remainder. Resolution failure flashes the existing library error and does not replace a queue. Resolving in the shell keeps queue content out of the component and follows the component-message identity boundary.

This changes only which existing `EmbyItem` sequence reaches the routed playback executor. It adds no queue-source variant, persistence shape, ctrl payload, daemon behavior, or queue presentation. Artist-discography playback reuses the queue-source behavior already used by Music track playback; the album-Workspace paths keep `MusicTrackActivate`/`play_album_track` unchanged.

### D4 — Pointer focus is applied by each resolved action's shell handler

The tree's pointer arms already emit `MusicAlbumCursor` (whose shell handler focuses Library), but only when the selected album actually changes; `MusicArtistTracks` itself has no focus side effect, repeat clicks can emit nothing, and `LibraryContentOwner::on_slot_event` can return only one `Msg`. A separate focus request cannot accompany a context-menu or play request across that boundary.

Decision: every tree gesture that resolves an artist, album, or track row emits one semantic request whose shell handler first focuses Library and then performs its action. Existing `MusicAlbumCursor` keeps that behavior. Add `LibraryPanelFocus` only for resolved gestures with no other external effect (repeat selection, local expansion, childless-album claim, and wheel over the painted tree). The Music track-play, artist-track, and row-context-menu handlers focus Library before their effects; for the generic `RowContextMenu` request this is carried by a Music-specific request rather than changing other destinations. No component emits two messages for one gesture, and no shell handler re-resolves pointer coordinates.

### D5 — Double-click reaches the owner; the owner decides expand vs play

The panel currently turns a non-Wide double-click on a hero-bearing browser into an overlay open. Add a named owner policy `double_click_opens_hero_overlay()` (default `browser_rows_are_hero_bearing()`), overridden to false by `MusicContent`. Rewrite the owner's `DoubleClick` arm in `music_interaction.rs` (the existing arm has no node-kind dispatch) so it resolves the latest painted node first (ADR 0024) and then toggles expansion for an artist root or an album leaf with cached track children; claims a childless album leaf without changing state; or emits the stable-ID play-now intent for a track item.

The same node-kind rule applies while the local tree filter is active. Production Grouped Music filtering paints the tree and leaves the flat Inline Search result carrier empty, so pointer handling must branch to filtered tree hit geometry rather than the legacy seeded-carrier harness path. Filter-forced visibility does not overwrite persistent expansion: double-click updates persistent expansion and the forced filtered projection remains visible until the filter closes.

### D6 — One grouped-track resolver feeds one ordered confirmation path

There is no existing Grouped Music track path that preserves autoload: `MusicTrackActivate` currently calls `play_album_track`, which always resolves and replaces an album queue, while generic `select_item` autoload derives siblings from the browse-level parent and is the wrong scope for the grouped tree. Add one shell resolver used by tree-track Enter and double-click. With autoload enabled it resolves the selected album's cached playable tracks in disc/track order and starts at the selected track; with autoload disabled it resolves only the selected Audio item. Both results enter the existing playback/admission executor as `PendingQueueAction::PlayItems`.

Add `ConfirmAction::ReplacePopulatedQueue`; this is new confirmation policy, not an existing variant. The handler inspects the canonical playback-target queue (local or directly controlled remote). An empty queue executes immediately. A populated queue stores the complete pending action and asks before replacement. On confirmation, local saved-playlist protection still runs through `replace_queue_or_prompt`: if that queue is dirty, the existing save/discard prompt follows as a second step before execution. Cancellation at either step leaves queue and playback unchanged. The remote path has no local dirty-playlist step. The pending action is the only executable payload, so confirmation cannot drift into a second playback executor.

### D7 — Go to Library builds the configured Grouped Music landing

The silent no-op is a missing migration at the landing boundary. `NavigateLanding::Album` currently reconstructs its path from Emby's physical folder ancestors, while `MusicContent` is eligible only when the resulting nav-stack depth is at the configured `music.levels` album position (`music_owner_key` → `is_music_group_view`/`is_viewing_album_folders`). A flat library, or any server ancestor chain whose depth differs from that configuration, can therefore switch the tab and install a stack for which no Music owner is eligible. The generic Emby owner deliberately excludes Music, so the projection then returns early with no fallback painter and no error.

Music item navigation SHALL resolve the album through the same configured album-index shape used by Grouped Music and Inline Search: a canonical `AlbumSearchEntry` whose ancestors follow `music.levels`, not raw `get_ancestors` depth. The worker builds every configured level and applies `retain_grouped_music_level_items` before publishing one prepared landing. The drain validates the target library and prepared stack before committing the tab, nav stack, saved Library position, or retained-owner re-anchor. A library-ID miss, configured-path miss, grouped-state construction failure, or rejected apply emits the existing Library error and leaves those values unchanged; no Album landing arm returns silently.

After commit, the shell reuses `on_recursive_album_activated` to bind the activated album, re-anchor retained `MusicContent`, and carry the requested track identity until album tracks arrive. If the fetched album tracks do not contain that identity, the album landing remains valid but mbv flashes that the requested track is no longer available in the album; it does not silently select the first track. This is an item-navigation mismatch, not a filesystem check or a second navigation rollback.

Alternatives: relax `music_owner_key` to infer Grouped Music from arbitrary server depth (rejected — it would make presentation ownership depend on server shape rather than configured navigation roles); retain raw Emby ancestors and add a generic Music fallback painter (rejected — creates a second owner/painter instead of completing the migration).

## Risks / Trade-offs

- [Unfiltered Enter no longer expands an artist root] → unfiltered expansion remains reachable on Left/Right, filtered Enter retains its local expansion behavior, and tests cover both paths; only the unfiltered Enter-toggle arm is deleted.
- [Discography playback can enqueue hundreds of tracks] → it reuses the artist-detail cache's settled ordering and the routed queue replacement executor; artist-Workspace activation keeps its existing no-new-confirmation behavior, while the populated-queue gate applies to tree-track Enter and double-click.
- [Confirmation on a populated queue interrupts track activation] → it applies to tree-track Enter and double-click only when the target queue would be replaced; an empty target queue still plays immediately.
- [Configured Grouped Music path cannot resolve the queued album] → reject the prepared landing through the existing Library-error path before changing the visible or saved destination state.
- [The queued track disappeared after its album resolved] → keep the valid album landing, report the missing track explicitly, and do not substitute the first track.
