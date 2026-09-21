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

The component emits `ShellRequest::MusicArtistTrackActivate { target: MusicArtistTarget, track_id: String }` from every artist-Workspace activation route: keyboard Enter, `HeroActivate`, and the Workspace row double-click. Both fields are stable opaque identities. The shell resolves the chosen `EmbyItem` and the ordered in-scope track list from its existing artist-detail cache for `target`, then sends the full flattened discography with the selected track's start index to the existing routed playback executor. Tracks before the selected one remain in the queue; playback begins at the selected index and naturally continues through the remainder. Resolving in the shell keeps queue content out of the component and follows the component-message identity boundary.

Queue source: add `QueueSource::Artist { name: String }`, so persisted queue state and the queue's source presentation name the discography rather than reusing `Album` (which stays the album-Workspace source). Existing serialized variants remain unchanged. A newly written `artist` variant is intentionally forward-only: older binaries are not expected to deserialize state written by this newer binary. Alternatives: reuse `QueueSource::Album` (rejected — wrong metadata/scope) or `Collection` (rejected — collides with folder play).

The album-Workspace paths keep `MusicTrackActivate`/`play_album_track` unchanged.

### D4 — Pointer focus is an explicit typed intent

The tree's pointer arms already emit `MusicAlbumCursor` (which the shell turns into Library panel focus), but only when the selected album actually changes, and the artist branch emits nothing on a repeat click. So a click could resolve a row and never move panel focus.

Decision: every tree gesture that resolves an artist, album, or track row requests Library panel focus. Selection arms return their resolved selection intent when that intent already focuses Library and otherwise emit a new `ShellRequest::LibraryPanelFocus`; double-click/play and context-menu handlers also focus Library as part of handling their resolved target. `MusicArtistTracks` keeps its existing focus side effect. This matches the other owners' row gestures without introducing a second routing site or reading component state in a `sync_*`.

### D5 — Double-click reaches the owner; the owner decides expand vs play

The panel currently turns a non-Wide double-click on a hero-bearing browser into an overlay open. Add a named owner policy `double_click_opens_hero_overlay()` (default `browser_rows_are_hero_bearing()`), overridden to false by `MusicContent`. Non-Wide double-clicks then fall through to the owner's existing `DoubleClick` arm in `music_interaction.rs`, which resolves the painted node first (ADR 0024) and then: toggles expansion for an artist root or an album leaf with cached track children; claims an album leaf without cached children as a no-op; or, for a track item, emits the play-now intent.

### D6 — Confirmation reuses the existing pending-action/confirm machinery

A double-clicked track resolves to `ShellRequest::MusicTrackPlayNow { album_id, track_id }`. Its handler uses the same shell-side album-track resolver and routed playback executor as Enter; only its pre-execution gate differs. It inspects the canonical target queue that the routed operation would replace. If that queue is populated — local or directly controlled remote — it stores `PendingQueueAction::PlayItems` and asks with `ConfirmAction::ReplacePopulatedQueue`; if empty, it executes immediately. Confirmation therefore cannot drift into a second playback executor.

### D7 — Go to Library: diagnose before fixing

The reported symptom is a silent no-op (no tab change, no toast), but every failure arm in `spawn_navigate_to_item`/`activate_recursive_album` sends a `LibEvent::Error` that flashes. A silent no-op therefore points at a path that returns before emitting, or at a landing whose state is replaced before it is applied. The implementation starts from a failing tick-level repro of the user's shape (grouped music, retained destination, queued track) and instruments each stage; the fix is driven by what the repro shows, and the landed behavior must satisfy `item-library-navigation`'s existing track-landing requirement plus this change's no-silent-no-op requirement.

Leading candidates to check: the queue context-menu item's `item_type`/`album_id` not matching the library's browse-level album identity, and a landing that is never applied because a later event replaces the nav stack before the component re-anchors.

## Risks / Trade-offs

- [Unfiltered Enter no longer expands an artist root] → unfiltered expansion remains reachable on Left/Right, filtered Enter retains its local expansion behavior, and tests cover both paths; only the unfiltered Enter-toggle arm is deleted.
- [Discography playback can enqueue hundreds of tracks] → it reuses the same resolution and queue replacement as an artist "Play" action, which already spans the artist; the confirmation gate only applies to the new double-click path.
- [Confirmation on a populated queue could surprise on a double-click] → it applies only to the tree double-click play intent, against the queue the routed operation will replace; Enter-on-track keeps its immediate behavior.
- [`QueueSource::Artist` is unknown to older binaries] → old variants retain their representation, but state written with the new variant is forward-only; cross-version downgrade reading is not supported.
- [Go to Library root cause unknown] → the diagnosis task produces the failing test first; the spec's no-silent-no-op requirement bounds the fix even if the root cause differs from the leading candidates.

## Open Questions

- Which silent path causes the Go to Library no-op, and whether the fix is in the resolve stage, the activation stage, or the shell's application of the landing. Answered by the diagnosis task's failing repro before the fix is written.
