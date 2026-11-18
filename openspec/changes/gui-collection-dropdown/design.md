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
  - Shows: the old `GuiShowsMode` pill row is gone. The TUI still uses `spawn_tv_latest` and
    `tv_upcoming_snapshots`, keyed by library id. The GUI's New and Upcoming lists read those same
    data sources and are selected by a single-choice view dropdown, not a collection set.
- The GUI launch state (`crates/mbv-config/src/gui_launch_state.rs`) saves one
  `GuiDestinationCollections { destination, target, group_ids, order, picks }` per collection
  destination whose selection the session changed. An older entry has no `target` or `order`; it
  loads as a Music group selection. The Shows view is a separate `shows_view` field, not a collection
  entry.
- The window holds the Library panel's interaction state; the core holds the data selection (spec
  "The window content holds the Library panel's interaction state"). The GUI Library panel takes its
  own keys in the window (ADR 0036). The pills are pointer-only now ("no key SHALL select a pill").

## Goals / Non-Goals

**Goals:**

- One selection type and one dropdown component for every collection destination, so the four
  destinations cannot drift. The TV view is a separate small selection type that reuses the dropdown
  presentation only.
- The TUI does not change: no TUI-shared store holds a set.

**Non-Goals:**

- The Home redesign. The two TV lists are only presented in the Shows destination; Home keeps its
  two existing sections.
- A keyboard path into the dropdown. The collection dropdown and the Shows view dropdown are
  pointer-only, as the pills are; the view dropdown adds no chord and no filter field.
- Removal of the TUI's Podcasts state pills, Emby New pill, or Shows modes.

## Decisions

### D1. One core selection type: `GuiCollectionSet`

New module `src/app/gui_library/collection.rs`:

```rust
pub enum GuiCollectionSet { All, Checked(Vec<String>) }   // Checked is never empty
pub enum GuiDateOrder { Newest, Oldest }                  // Default: Newest
```

`GuiCollectionSet` owns the rules: `toggle(key)` (check, or uncheck and fall back to All),
`normalized(Vec<String>)` (an empty list is All), `retain_present(&[String])` (drops vanished keys
and falls back to All), and `effective(Option<&[String]>)` (`None` keeps the set — the catalog has
not landed yet — `Some` retains). One stored selection type,
`GuiCollectionSelection { set, order }`, is what every destination's store holds. The keys are the
stable Service ids: the Music group id, the podcast show `library_item_id`,
the subscription URL, and the Emby folder id. `Checked` keeps press order; the projections paint in
the collection's own order, so the order inside the set has no effect.

Alternative: a `BTreeSet` plus a separate `all: bool`. Rejected: it can express "All and checked",
which the spec forbids.

### D2. The GUI holds its own set; TUI-shared stores keep one value

- Music: `gui_library_groups` becomes `HashMap<usize, GuiCollectionSelection>` (one stored
  selection: the set plus the order, whose order half Music never reads). `gui_selected_group`
  becomes `gui_selected_groups`, which returns every group for All. The union code is reused.
- Podcasts: `GuiPodcastSelection.pill` becomes `shows: GuiCollectionSet` plus `order: GuiDateOrder`.
  `GuiPodcastPill` and its `State` arm are deleted. The fan-out scope written to
  `committed_show_pill` is `Some(id)` for exactly one checked show, and `None` (every show, today's
  Latest loads) for All or several shows. The projection filters the merged list to the checked
  shows.
- Feeds: `GuiFeedsSelection.pill` becomes `subscriptions: GuiCollectionSet` plus `order`.
  `GuiFeedPill` is deleted.
- Emby feed group: a new GUI map `gui_emby_feed_sets: HashMap<usize,
  GuiCollectionSelection>`. `GuiEmbyFeedGroup` and the New view are deleted. The GUI no longer calls
  `apply_feed_home_video_group_selection`, so the TUI-shared `selection` stays as the TUI set it.

Alternative: widen the TUI stores to sets. Rejected: it changes TUI behavior, and #986 dropped TUI
parity for the GUI Library panel.

### D3. YouTube fetches each checked folder without moving the TUI selection

`feed.rs` gains `spawn_feed_home_video_fetch(lib_idx, target: FeedHomeVideoSelection)`. It is the
body of today's selection fetch, with the target passed in instead of read from `state.selection`.
The existing path calls it with `state.selection`, so the TUI does not change. The GUI ensure step
(`ensure_gui_emby_selection`) finds the first target (All, or the first checked folder) that has no
cache entry and that `fetching` does not name, and starts one fetch. It starts the next one on a
later frame, when `fetching` is clear. One fetch runs at a time, as today. A failed fetch leaves no
positive cache entry, so the ensure step offers that target again; only a user press (a collection
toggle or All) runs that ensure step again, so an automatic repaint or tick never retries a failed
group and no retry storm starts. The same rule covers the Music per-group album store.

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
"undated last in both orders" rule has one owner and one unit test. The GUI sends no `DateOrder`
command, so GUI lists stay on Newest; the Oldest path stays for the core command (D5).

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
no-op arm. No GUI control sends `DateOrder` (D6a); the command stays for the core and later callers.

### D6. One frame, one component

The bridge replaces `GuiMusicPillFrame`, `GuiPillFrame<P>`, the Emby pill frame, and the old Shows
pill frame with one `GuiCollectionsFrame` for the four collection destinations. Shows carries its own
view frame beside the list (D8):

```rust
struct GuiCollectionsFrame {
    target: GuiCollectionTarget,
    all: bool,
    entries: Vec<GuiCollectionEntry>,   // { key, name, checked }, in catalog order
    noun: GuiCollectionNoun,            // Groups | Shows | Subscriptions | Folders
}
```

The window gets one prop-driven dropdown presentation component
(`window/components/library_dropdown.rs`) for the trigger, the rows, and the popover.
`LibraryCollectionDropdown` (`window/components/library_collection_dropdown.rs`) supplies the
collection frame's entries and filters to it. Shows supplies its three view rows and no filter
through the same presentation component; no collection code names Shows.
with the trigger and the popover. The popover paints with
`menu_card`, as the row menu and Queue chooser do. The trigger label is All, the one checked name, or
the count with the noun (for example "2 folders"). `LibraryPillRow`, `LibraryPillButton`,
`library_pills.rs`, the four destination pill components, and `window/library/pill_list.rs` are
deleted with their tests.

### D6a. Placement and form follow the artboard (page 7)

The dropdown is painted by `LibraryListHeader`, not by the panel. The header takes an optional
collections frame. The title line is: the list title at the left, and the trigger at the right end
when the frame is present, and nothing else. No destination paints a selection label on the right
of the title line: the destinations with collections paint the trigger in its place, because the
trigger names the selection, and Movies (a plain Emby library) and the Shows destination paint no
label at all (user decision 2026-11-18). The panel paints nothing between `LibraryTopBar` and the
list region: the selector row and its `pills` region are deleted.

The trigger is a rectangular button with the list filter field's corner radius, an outline in the
accent colour, and a chevron that points down while closed and up while open. It is not a capsule
and not a muted pill.

The popover is absolutely positioned under the trigger inside the header, over the list, with its
right edge aligned with the trigger's right edge, and paints as the artboard shows: a filter strip
with a teal `/` and the placeholder (for example "Find a category" for folders), a divider under it,
a checkbox row for All first with an extra gap after it, and one checkbox row per collection with a
highlighted band and a bold name on a checked row. The TV view dropdown omits the filter strip and
paints the selected view with a radio-style mark; it still uses the same trigger, popover, row look,
and focus hook. The artboard's footer does not paint: the popover has no footer strip, and Esc still
closes the dropdown (D7).

The artboard shows no Clear control and no date order toggle, and the user has dropped both from the
GUI. All is selected by pressing the All row, and the GUI shows no date order control; the core keeps
the `DateOrder` command (D4, D5).

### D7. The window holds the open state and the filter text

`LibraryState` gains `collections_open: bool` and `collections_filter: String`. They are interaction
state, so they live in the window (spec "The window content holds the Library panel's interaction
state"). The filter matches with `filter_match` (`window/library/list.rs`), the rule the Library filter uses.
The window closes the dropdown and clears the text on a press outside it, on Esc, and on a
destination change. While it is open, the window's Library key handler gives typed characters,
Backspace, and Esc to the filter before any other Library key, as the Library filter does. No
binding is added to `src/app/input/`: the GUI Library panel takes its own keys in the window
(ADR 0036), and the dropdown adds no new chord.

### D8. Shows has a single-choice view dropdown

The GUI Shows destination's list header paints one dropdown with exactly three choices: **All**, **New**,
and **Upcoming**, in that order. All is the default; it is the current series list with its letter rail,
Hero, and Workspace. A press on one choice replaces the active choice and closes the popover. The
choice is a view of the destination, not a collection set: `GuiCollectionSet`, `GuiCollectionTarget`,
`GuiCollections`, `apply_collection`, and the collection commands have no Shows variant.

The choice's own core type is `GuiShowsView { All, New, Upcoming }`, held per Shows library in
`App::gui_shows_views` (an absent entry is All). `GuiLibraryCommand::ShowsSetView` carries the library
index and the choice and installs it through one `App` method. The frame carries a `view` field with
the active choice; the window paints the three labelled entries from the view type. The window
reuses the prop-driven dropdown presentation with a radio-style selected mark and no filter field;
it takes no new chord. The
selection is saved in `GuiLaunchState::shows_view`, whose absence loads as All (D14).

The old `GuiShowsMode`, its pill frame, and the `ShowsSetMode` command stay deleted. The TUI's TV modes
stay unchanged. The GUI's series selection and Workspace still read the All series tree; New and
Upcoming use the flat rows of D9.

### D9. The two TV lists move from Home into Shows

Home keeps only `Continue watching` and `Recently added`; its `Latest episodes` and `Upcoming`
sections and the Home-only launch work are deleted. The same data sources and row types now paint in
the Shows list under the new choices; no second list store is introduced.

- **New**: the old Shows Latest episodes list, built from `build_tv_latest_level`'s
  `get_latest_episodes(parent_id, 30)` source. The GUI keeps one merged per-library store
  (`gui_shows_new: HashMap<String, Vec<EmbyItem>>`) and its completion event, filled once at GUI
  launch on worker threads. It never touches `nav_stack`. The rows merge every TV library's list by
  `date_added`, newest first, de-duplicated by item id, and a New row selects, plays, and enqueues as a
  Recently added Emby row does.
- **Upcoming**: `tv_upcoming_snapshots`, filled through the existing eager per-library fetch and
  once-per-session guard at GUI launch. The rows merge every TV library's premieres by premiere date,
  soonest first. An Upcoming row is display-only: `ShowsSelect`, `ShowsPlay`, `ShowsPlayNext`, and
  `ShowsQueue` on an Upcoming row are documented no-op arms, and the Hero keeps its item.

### D10. One saved entry per collection destination

`group_ids` holds the checked ids of the entry's target; an empty list means All. The save writes
an entry only for a destination whose selection the session changed, as today. Restore drops
vanished ids through `retain_present`, which falls back to All. An old Music-only entry still
restores its checked groups. The kept album per group (`picks`) keeps its format; D11 says when a
pick is written. D13 names the shared path that writes and restores every target.

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

### D13. Every collection target shares the launch save path

One `GuiDestinationCollections` entry holds the stable destination identity, a `target` kind
(`music`, `podcasts`, `feeds`, or `emby_feed`), the checked ids in press order, the date order where
the site stores one, and the Music picks. Music groups, Podcast shows, Feeds subscriptions, and
Emby feed folders all write and restore through this shape. Restore matches both the destination
and the target, so saved ids never land in another dropdown. An entry written before `target`
existed holds Music groups, so a missing `target` defaults to Music; a missing `order` means no
saved order. The shared site trait supplies each destination's set, order, and picks, and refuses
to overwrite a selection the session made before the restore completed.

### D14. The Shows view has its own launch field

`GuiLaunchState` gains `shows_view`, a separate three-value field saved at orderly exit and restored
after the destination opens. It is not a `collections` entry and never uses a collection target.
A file written before the field existed deserializes to All. The restore applies the saved view to
the opened Shows destination through the same `ShowsSetView` path a press uses; a destination that is
not Shows ignores it.

## Risks / Trade-offs

- [All in Music loads every group's album list] → The loads go through the existing per-group album
  store, one at a time, and the list shows as loading until they land. A library with many groups
  pays this only when the user selects All.
- [Several Emby folders load one after another] → Each folder is cached for the session, so the cost
  is paid once per folder. The list paints the folders that landed and shows the loading state for
  the rest.
- [Podcasts with several shows uses the All fan-out] → The same loads as today's Latest pill; no new
  load path.
- [The panel-order test names a `pills` region] → The `pills` region is deleted with the pill row.
  The panel-order test drops it. The dropdown is part of the list header, not a panel region.
- [Upcoming rows look selectable] → They paint as rows in the Shows destination, but they take no
  select, play, or enqueue. This is accepted until a later Home redesign.
- [A merged New or Upcoming list shows under one library's tab] → It is the same merged list the two
  Home sections carried; the destination's dropdown chooses the list, not the library's browse
  history.

## Migration Plan

No file rewrite: an entry without `target` loads as Music and one without `order` has no saved
order (D13). An older build ignores the added fields and reads `group_ids` and `picks` as before.
Rollback is a revert.
