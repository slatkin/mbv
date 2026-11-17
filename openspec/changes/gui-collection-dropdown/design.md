# Design

## Context

See `proposal.md` for the reasons and `specs/gui-client/spec.md` for the behavior.

Current state:

- One shared pill row, `LibraryPillRow` (`crates/mbv-gui/src/window/components/library_pill_row.rs`),
  is painted by five components: `library_pills.rs` (Music), `library_shows_pills.rs`,
  `library_emby_pills.rs`, `library_podcasts_pills.rs`, and `library_feeds_pills.rs`. A press sends a
  per-destination command. `window/library/pill_list.rs` holds the `pill_select` intent that turns a
  request into the command.
- The core holds one value per destination:
  - Music: `App::gui_library_groups: HashMap<usize, String>`. `gui_selected_group` falls back to the
    first group. `gui_music_union` already loops over every selected pill and de-duplicates by album
    id (`src/app/gui_library/projection.rs`).
  - Podcasts: `GuiPodcastPill { Latest, State(filter), Show(id) }` (`src/app/gui_library/podcasts.rs`).
    `commit_gui_podcast_show_scope` writes `committed_show_pill: Option<String>`, which the
    TUI-shared bounded fan-out reads.
  - Feeds: `GuiFeedPill { Latest, Subscription(url) }` (`src/app/gui_library/feeds.rs`). Every entry
    is already in `feed_tab.all_entries`.
  - Emby feed group: `GuiEmbyFeedGroup { Latest, All, Folder(id) }` (`src/app/gui_library/emby_projection.rs`).
    The fetch (`src/app/dispatch/feeds/feed.rs`) reads the TUI-shared `FeedHomeVideoState::selection`,
    sets one `fetching` slot, loads the full set (first page, then a total refill), and caches it in
    `items: HashMap<FeedHomeVideoSelection, Vec<EmbyItem>>`. All is fetched `Descending` and a folder
    `Ascending`, both by `DateCreated`.
  - Shows: `GuiShowsMode { Latest, Upcoming, All }`. Latest loads into the library's `nav_stack`
    (`spawn_tv_latest`); Upcoming reads `tv_upcoming_snapshots`, keyed by library id.
- The GUI launch state (`crates/mbv-config/src/gui_launch_state.rs`) saves Music as
  `GuiDestinationCollections { group_ids: Vec<String>, picks }`. The list shape is already there.
- The window holds the Library panel's interaction state; the core holds the data selection (spec
  "The window content holds the Library panel's interaction state"). The GUI Library panel takes its
  own keys in the window (ADR 0036). The pills are pointer-only now ("no key SHALL select a pill").

## Goals / Non-Goals

**Goals:**

- One selection type and one dropdown component for every collection destination, so the four
  destinations cannot drift.
- The TUI does not change: no TUI-shared store holds a set.

**Non-Goals:**

- The Home redesign. Latest episodes and Upcoming are only appended to the current Home list.
- A keyboard path into the dropdown. It is pointer-only, as the pills are.
- Removal of the TUI's Podcasts state pills, Emby New pill, or Shows modes.

## Decisions

### D1. One core selection type: `GuiCollectionSet`

New module `src/app/gui_library/collection.rs`:

```rust
pub enum GuiCollectionSet { All, Checked(Vec<String>) }   // Checked is never empty
pub enum GuiDateOrder { Newest, Oldest }                  // Default: Newest
```

`GuiCollectionSet` owns the rules: `toggle(key)` (check, or uncheck and fall back to All),
`all()`, `contains(key)`, and `retain_present(&[String])` (drops vanished keys and falls back to
All). The keys are the stable Service ids: the Music group id, the podcast show `library_item_id`,
the subscription URL, and the Emby folder id. `Checked` keeps press order; the projections paint in
the collection's own order, so the order inside the set has no effect.

Alternative: a `BTreeSet` plus a separate `all: bool`. Rejected: it can express "All and checked",
which the spec forbids.

### D2. The GUI holds its own set; TUI-shared stores keep one value

- Music: `gui_library_groups` becomes `HashMap<usize, GuiCollectionSet>`. `gui_selected_group`
  becomes `gui_selected_groups`, which returns every group for All. The union code is reused.
- Podcasts: `GuiPodcastSelection.pill` becomes `shows: GuiCollectionSet` plus `order: GuiDateOrder`.
  `GuiPodcastPill` and its `State` arm are deleted. The fan-out scope written to
  `committed_show_pill` is `Some(id)` for exactly one checked show, and `None` (every show, today's
  Latest loads) for All or several shows. The projection filters the merged list to the checked
  shows.
- Feeds: `GuiFeedsSelection.pill` becomes `subscriptions: GuiCollectionSet` plus `order`.
  `GuiFeedPill` is deleted.
- Emby feed group: a new GUI map `gui_emby_feed_sets: HashMap<usize, (GuiCollectionSet,
  GuiDateOrder)>`. `GuiEmbyFeedGroup` and the New view are deleted. The GUI no longer calls
  `apply_feed_home_video_group_selection`, so the TUI-shared `selection` stays as the TUI set it.

Alternative: widen the TUI stores to sets. Rejected: it changes TUI behavior, and #986 dropped TUI
parity for the GUI Library panel.

### D3. YouTube fetches each checked folder without moving the TUI selection

`feed.rs` gains `spawn_feed_home_video_fetch(lib_idx, target: FeedHomeVideoSelection)`. It is the
body of today's selection fetch, with the target passed in instead of read from `state.selection`.
The existing path calls it with `state.selection`, so the TUI does not change. The GUI ensure step
(`ensure_gui_emby_selection`) finds the first target (All, or the first checked folder) that has no
cache entry and that `fetching` does not name, and starts one fetch. It starts the next one on a
later frame, when `fetching` is clear. One fetch runs at a time, as today.

The Emby `getItems` endpoint takes one `ParentId` (dev.emby.media `ItemsService/getItems`), so one
query cannot cover several folders. The cache already holds full lists, so the union is a merge of
cached lists, de-duplicated by item id.

### D4. The date order is a local sort

Every feed-based list is held in full (D3 for Emby; the merged Podcasts and Feeds lists today), so
the order is a sort in the projection, with no refetch:

- Emby: by `EmbyItem::date_added` (the `DateCreated` field, already requested), then by id.
- Podcasts and Feeds: the current newest-first comparison, reversed for Oldest, with undated rows
  moved last after the reversal.

The sort key comparison lives in `collection.rs` as one function over `Option<i64>` seconds, so the
"undated last in both orders" rule has one owner and one unit test.

### D5. Commands

`GuiLibraryCommand` replaces `SelectCollection`, `PodcastsSetPill`, `FeedsSetPill`, the Emby
set-feed-group command, and `ShowsSetMode` with:

```rust
CollectionToggle { target: GuiCollectionTarget, key: String }
CollectionAll   { target: GuiCollectionTarget }          // All row and Clear
DateOrder       { target: GuiCollectionTarget, order: GuiDateOrder }

enum GuiCollectionTarget { Music(usize), Podcasts(usize), Feeds, EmbyFeed(usize) }
```

Each arm is exhaustive per target. A key that the target's catalog does not hold is a stale frame;
it is logged and ignored, as stale commands are today. `DateOrder` on `Music` is a documented
no-op arm.

### D6. One frame, one component

The bridge replaces `GuiMusicPillFrame`, `GuiPillFrame<P>`, the Emby pill frame, and the Shows pill
frame with one `GuiCollectionsFrame`:

```rust
struct GuiCollectionsFrame {
    target: GuiCollectionTarget,
    all: bool,
    entries: Vec<GuiCollectionEntry>,   // { key, name, checked }, in catalog order
    noun: GuiCollectionNoun,            // Groups | Shows | Subscriptions | Folders
    order: Option<GuiDateOrder>,        // None for Music
}
```

The window gets one component, `LibraryCollectionDropdown` (`window/components/library_collection_dropdown.rs`),
with the trigger, the Clear control, the date order toggle, and the popover. The popover paints with
`menu_card`, as the row menu and Queue chooser do. The trigger label is All, the one checked name, or
the count with the noun (for example "2 folders"). `LibraryPillRow`, `LibraryPillButton`,
`library_pills.rs`, the four destination pill components, and `window/library/pill_list.rs` are
deleted with their tests.

### D7. The window holds the open state and the filter text

`LibraryState` gains `collections_open: bool` and `collections_filter: String`. They are interaction
state, so they live in the window (spec "The window content holds the Library panel's interaction
state"). The filter matches with `filter_match` (`window/library/list.rs`), the rule the Library filter uses.
The window closes the dropdown and clears the text on a press outside it, on Esc, and on a
destination change. While it is open, the window's Library key handler gives typed characters,
Backspace, and Esc to the filter before any other Library key, as the Library filter does. No
binding is added to `src/app/input/`: the GUI Library panel takes its own keys in the window
(ADR 0036), and the dropdown adds no new chord.

### D8. Shows is the series list only

The GUI Shows destination always uses `TvContentMode::All`. `GuiShowsMode`, its pill frame, and the
`ShowsSetMode` command are deleted. The Latest and Upcoming flat-row select paths in
`src/app/gui_library/shows.rs` go with them. The TUI code paths (`spawn_tv_latest`, the Upcoming
snapshot) stay, because the TUI and Home use them.

### D9. Home appends two sections

- **Latest episodes**: a new GUI store `gui_home_tv_latest: HashMap<String, Vec<EmbyItem>>` keyed by
  TV library id. At GUI launch, for each Emby TV library, the core runs the same
  `get_latest_episodes(parent_id, 30)` call that `build_tv_latest_level` makes, on a worker thread,
  and sends one new `LibEvent` that fills the store. It does not touch `nav_stack`. The section
  merges every library's list by `date_added`, newest first, de-duplicated by item id.
- **Upcoming**: at GUI launch, the core requests the eager Upcoming fetch for each TV library through
  the existing once-per-session guard, and the section merges `tv_upcoming_snapshots` by premiere
  date, soonest first.
- `GuiHomeRowId` gains `LatestEpisode(String)` (item id) and `Upcoming(String)` (the premiere's
  stable key). `LatestEpisode` resolves through the Emby item path that Recently added Emby rows use.
  `HomeSelect`, `HomePlay`, `HomePlayNext`, and `HomeQueue` on an `Upcoming` id are documented no-op
  arms, so the Hero keeps its item.

### D10. Music save format does not change

`group_ids` holds the checked groups; an empty list means All. The save writes an entry only for a
library whose selection the session changed, as today. Restore drops vanished group ids through
`retain_present`, which falls back to All. An old file always holds one group, so it restores as
that one checked group. The kept album per group (`picks`) keeps its format; D11 says when a pick is
written.

### D11. When a Music pick is kept

The per-group pick (`gui_library_picks`) is written only while exactly one group is checked. While
All or several groups are checked, the pick is a session-only `gui_music_union_pick` per library.
On a selection change the Hero resolves: one group, its kept pick, else its first album; All or
several, the current album if the union holds it, else the union's first album.

### D12. CONTEXT.md

The *Collection pill* term becomes *Collection dropdown*: "A *GUI Library panel* dropdown that
narrows the list to one or more content sets that the Service holds: Music groups, Podcast shows,
Feeds subscriptions, or Emby feed-group folders. Its selection is All or a set of checked
collections. _Avoid_: multi-pill, filter pill, collection pill." The *Library Status bar* term names
the checked collections.

## Risks / Trade-offs

- [All in Music loads every group's album list] → The loads go through the existing per-group album
  store, one at a time, and the list shows as loading until they land. A library with many groups
  pays this only when the user selects All.
- [Several Emby folders load one after another] → Each folder is cached for the session, so the cost
  is paid once per folder. The list paints the folders that landed and shows the loading state for
  the rest.
- [Podcasts with several shows uses the All fan-out] → The same loads as today's Latest pill; no new
  load path.
- [The in-flight change `gui-swap-queue-and-library-sides` renames the panel-order test that names
  the pill row] → Whichever change lands second updates the region name in that test. The region
  keeps the name `pills` in the hit-region list so the order test does not change meaning.
- [Upcoming rows look selectable] → They paint as rows, but they take no select. This is accepted
  until the Home redesign.

## Migration Plan

No data migration. The launch-state file keeps its format (D10). Rollback is a revert: an older
build reads a multi-group `group_ids` and selects its first present group.
