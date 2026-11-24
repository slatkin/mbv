# Design

## Context

See proposal.md (Why) for the problem. Current state of the painters in `crates/mbv-gui/src/window/`:

| Row | Painter today | Look source |
| --- | --- | --- |
| Home items | `LibraryItemRow` via `library_home_list.rs` | `name_color(state)`; `played_mark` on Recent rows only (commit `ad064a00e`) |
| Podcasts, Feeds | `paint_item_row` -> `LibraryItemRow` | `name_color(state)`, `episode_progress` |
| Emby, Audiobooks | `LibraryItemRow` in `library/lettered/adapters.rs` | `canvas_var("text")` hard-coded, `progress: None` |
| Shows New / Upcoming | `LibraryItemRow` in `library_shows_list.rs` | `canvas_var("text")` hard-coded |
| Music artist, Shows series | `LibraryItemRow` with `tree_open` | `canvas_var("text")` hard-coded |
| Music album, track | hand-built `button` in `library_list.rs` | own style strings; tracks always muted |
| Shows season, episode | hand-built `button` in `library_shows_list.rs` | own `row_name_color` / `episode_played` lookup |
| Playlists playlist, item | hand-built `button` in `playlists_panel.rs` | own `row_style`, `canvas-selected` fill |

`LibraryItemRowProps` takes `title_color: String`, `background: Option<String>`, and `progress: Option<f32>`, so the look is a caller choice. No type stops a list from painting its own `button`.

The core already derives a play state for every source through `MediaSemanticState` (`crates/mbv-ui-model/src/media_list.rs`: `from_emby`, `from_feed_entry`, `from_audiobookshelf`, `from_queue_item`). The bridge maps it to `GuiHomeRowStateFrame` (`Ordinary`, `Played`, `Active { progress }`). Home, Podcasts, Feeds, and the Shows Workspace episodes carry it. Emby rows, Audiobooks book rows, Shows flat rows, Music tracks, and Playlists items do not.

Home's Recently added items are stored snapshots (`App::home_recently_added`, filled by the Latest fetch). The played and progress update paths do not touch them, so a later play never reaches those rows.

Blitz renders through `anyrender_vello_hybrid` 0.11.0, whose `filters.rs` returns `None` for `FilterEffect::ColorMatrix`. `grayscale()` is a color matrix, so it paints nothing.

## Goals / Non-Goals

**Goals:**

- Two row painters, and the look of each derived only from the row's data and state.
- Removing a look prop makes a bespoke restyle a compile error, not a review finding.
- Every list row that names a playable item gets its real, current play state.

**Non-Goals:**

- The Queue rows, the Workspace tables, and the Hero tracklist.
- Hover controls (the open change `gui-row-hover-actions` adds them to the two painters after this change).
- Any TUI change.

## Decisions

### D1. Two primitives in one module

`crates/mbv-gui/src/window/components/list_row.rs` holds the shared state and look, with `list_row/flat.rs` (`FlatRow`), `list_row/tree.rs` (`TreeRow`), and `list_row/mark.rs` (the played mark). `library_item_row.rs` is deleted; its thumb painter moves to `list_row/flat.rs`.

- `FlatRow` paints every top-level row of every list, flat or tree: the art thumb, the title over its optional subtitle, the trailing text, and, on a tree parent, the open state (`tree_open`), the guide start (`tree_guide`), and `on_toggle`. This is `LibraryItemRow` minus its look props.
- `TreeRow` paints every nested child row and every Playlists row: an optional number cell, the title, the trailing text, the indent (`inset: f32`), the open state of a nested parent, the passing guides, and the subtree's last-row divider (`tail: bool`).

Alternative considered: one row with a `variant` prop. Rejected: the two shapes have different props (thumb and subtitle against number and inset), and a single props struct would carry fields that one shape ignores.

The guides stay the caller's input (`guides: Element`, built with the existing `tree_guide.rs` helpers). Guides are structure that depends on the tree's depth and open state, not row look.

### D2. One row state, no look props

```rust
pub struct RowState {
    /// `None` when the item tracks no play state.
    pub play: Option<GuiHomeRowStateFrame>,
    pub selected: bool,
}
```

Both primitives take `state: RowState` and nothing else that sets a look. Removed from the props: `title_color`, `background`, `progress`, `played_mark`. A list cannot pass a colour, weight, fill, opacity, or style string. Hooks stay (`attr`, `kind`, `id`, plus `hooks: Vec<Attribute>` on `TreeRow` for marks such as `data-library-shows-open`), because tests read them and they paint nothing.

`GuiHomeRowStateFrame` keeps its name. Renaming it touches 79 sites for no behaviour change.

`None` (not tracked) is used for: Emby folders and box sets, Music artists, albums, and tracks (`MediaSemanticState::from_emby` already treats music as `Ordinary`; the GUI shows no mark for music), Shows series and seasons, Playlists playlists, and Shows Upcoming premieres.

### D3. The look

The primitives derive the look from `RowState` with the Everforest tokens in `window/theme.rs` (`EfToken`, `ef_var`):

| State | Title | Subtitle | Trailing text | Mark | Thumb |
| --- | --- | --- | --- | --- | --- |
| not tracked | `Fg`, 600 | `Grey2` | `Fg` | none (slot still reserved) | full |
| unplayed | `Fg`, 600 | `Grey2` | `Fg` | 8 px filled circle, `Green`, label "Unplayed" | full |
| partly played | as unplayed | as unplayed | as unplayed | as unplayed | full, progress run |
| played | `Grey1`, 400 | `Grey1` | `Grey1` | 12 px Lucide `Check`, `Grey1`, label "Played" | opacity 0.35 |

- The mark slot is 12 px square with a 6 px gap before the trailing text, on every row of both types, so trailing text lines up across states and lists. A not-tracked row keeps the empty slot.
- The mark is `role="img"` with `aria-label`. This build does not enable Blitz's accessibility feature, so the label reaches assistive technology only when that feature is turned on. That is out of scope.
- No `grayscale()`: the renderer drops it (Context). Opacity alone.
- The progress run: `FlatRow` keeps today's 3 px run along the thumb's bottom edge. `TreeRow` has no thumb, so it paints the same 3 px track and fill along its bottom edge, under the title-to-trailing span.
- Selection: both types paint the `library-raise` fill. An unselected `TreeRow` paints the `tree-subtree` fill. The Playlists screen's `canvas-selected` fill and 6 px radius go.
- Numbers (track and episode) paint in the `canvas-ordinal` tone in both states.
- `NowPlaying` takes the partly played look: the unplayed tones and mark, with the progress run. No source in D4 produces it, but `RowState` admits it, so the derivation names it instead of defaulting.

These values replace commit `ad064a00e`'s Recently added constants, which move into `list_row.rs`.

### D4. Play state from the core projections

Each projection fills the state from the existing `MediaSemanticState` constructors. No new Service call.

| Row | Source |
| --- | --- |
| `GuiEmbyRow` | `MediaSemanticState::from_emby(item)`; `None` for a folder, box set, or music item |
| `GuiShowsRow` (flat New) | `from_emby(episode)`; `None` for an Upcoming premiere |
| `GuiAudiobooksRow` | `from_audiobookshelf(finished, position, duration)` from the library's stored book progress (the map that `apply_audiobookshelf_catalog_progress` fills); `Ordinary` when no entry exists |
| `GuiMusicTrack` | `None` (music) |
| `GuiPlaylistItemProjection` | `from_emby(item)`; `None` for a music item |

Each matching bridge frame gains `state: Option<GuiHomeRowStateFrame>` with the existing `MediaSemanticState` mapping.

### D5. Recently added follows the live state

`App` gains one helper in `crates/mbv-client-core/src/app/state/app_struct/home_recently_added.rs`:

```rust
pub(in crate::app) fn apply_home_recent_played(
    &mut self, content: &QueueItemContentId, played: bool, position_ticks: i64,
)
```

It updates every stored `home_recently_added` item with that content id. Every path that applies a played or progress change calls it: `apply_stopped_slot_progress` and `handle_track_completed_event` (a play that stops, or one that ends with a next item, which reports `TrackCompleted`), `context_set_played` and `set_feed_entries_played` (mark played or unplayed), `apply_audiobookshelf_socket_progress`, `handle_audiobookshelf_progress` and `handle_audiobookshelf_book_progress` (Audiobookshelf progress on the ctrl route), and `apply_audiobookshelf_catalog_progress`. The Home projection reads the stored item, so the next frame shows the change.

Alternative considered: refetch Latest on every change. Rejected: one network fetch per progress event, and the order of the list could move under the pointer.

### D6. Tests

- `list_row/tests.rs` owns the look contract: one `#[case]` table over the two types and the four states, asserting the mark hook and its label, the dimmed thumb on played, the progress run on partly played, and an identical trailing-text x position across states.
- Each list keeps its existing harness. One existing test per list gains one played row and one unplayed row in its fixture and asserts the mark hook on each. That proves the list passes its state. No new harness.
- One test for D5: a stopped-slot progress that marks an item played changes its Recently added row's state.
- Tests that assert removed props or bespoke styles are deleted with them.

## Risks / Trade-offs

- [The change touches every Library list painter at once] -> Tasks go list by list. Each unit leaves the other lists on the old painter until its own unit runs, and `LibraryItemRow` is deleted only after the last list moves.
- [`gui-row-hover-actions` edits the same painters] -> This change lands first. That change's group 5 is then rewritten against `FlatRow` and `TreeRow`; its other groups do not touch row painters.
- [Music rows lose their muted track title] -> Intended: one look for every tree row.
- [Audiobooks progress map may be empty before the catalog fetch lands] -> Rows paint `Ordinary` (unplayed) until it lands, then repaint from the next frame.

## Migration Plan

No data migration. Rollback is a revert of the change's commits.
