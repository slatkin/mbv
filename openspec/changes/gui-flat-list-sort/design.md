# Design

## Context

See proposal.md for the motivation and the scope table. The behavior is in
`specs/gui-client/spec.md`. This is the current state that shapes the approach:

- The core holds a date order (`GuiDateOrder { Newest, Oldest }`,
  `src/app/gui_library/collection.rs`) inside `GuiCollectionSelection`, beside the checked set.
  Only the four collection sites read it: Music (it reads nothing), Podcasts, Feeds, and Emby
  feed-group. `GuiLibraryCommand::DateOrder` and `CollectionOp::Order` set it. The launch
  state saves it as `GuiDestinationCollections.order`. No window code calls `press_date_order`
  (`crates/mbv-gui/src/window/library/state.rs`), so no control paints it.
- The plain Emby library and Audiobooks have no collection site, so the collection path cannot
  hold their order.
- The flat lists sort in their projections: `emby_projection.rs` (plain root list in the
  server's `SortName` order; the feed-group items by date), `audiobooks_projection.rs` (author
  key, then title), `podcasts_projection.rs` and `feeds_projection.rs` (by date through
  `compare_gui_date_order`), and `shows_projection.rs` (`gui_shows_new_rows`, date added).
  Every one of these lists is in memory as a whole, so a sort change needs no fetch.
- The window decides letter headings and the rail from the list kind. `build_emby_rows` uses
  `feed_group`, Audiobooks always uses headings, and Podcasts, Feeds, and Shows New never do.
- `AudiobookshelfBook` (`crates/mbv-audiobookshelf/src/catalog_books.rs`) parses `updatedAt`
  from the `/api/libraries/{id}/items` item DTO but not `addedAt`. The Audiobookshelf server
  writes `addedAt: this.createdAt.valueOf()` (epoch milliseconds) into its library item JSON
  (`server/models/LibraryItem.js`, line 981 on the current `master` branch), in the same object
  that carries `updatedAt`.
- The GUI launch state is written at orderly exit only and restored at start
  (`src/app/gui_library/launch.rs`). The destination's stable identity is `TabIdentity`: a
  Service library id, or `Feeds`.

## Goals / Non-Goals

**Goals:**

- One sort type, one store, one command, and one launch field for every flat list.
- A sort change costs no fetch.
- The headings and the rail follow the active sort, through one rule in one place.

**Non-Goals:**

- No server-side sort, and no change to the plain Emby root fetch or the Shows browse level.
- No change to the TUI's letter-range modes, the TUI's Audiobookshelf order, or TUI state.
- No article stripping for titles that the Service gives no sort name.
- The Shows view stays one launch field for all Shows libraries. This change does not touch it.

## Decisions

### D1. One `GuiListSort` type beside the date order

A new module `src/app/gui_library/list_sort.rs` holds
`pub enum GuiListSort { Title, Author, Newest, Oldest }`. It also holds `label()` (the control's
text, owned by the core so the two layers cannot drift) and `date_order() -> Option<GuiDateOrder>`
(`Newest` and `Oldest` map to the existing `GuiDateOrder`, and `Title` and `Author` map to
`None`). It holds `is_name_sort()` (`Title` or `Author`) too. `GuiDateOrder` and
`compare_gui_date_order` stay as the date comparator, and they still own the "undated last"
rule.

*Alternative*: replace `GuiDateOrder` with the new enum everywhere. Rejected, because the
comparator would then have to handle `Title` and `Author` arms that it cannot order.

### D2. A sort target per flat list, with its choices and default

`pub enum GuiSortTarget { Library(usize), Audiobooks(usize), Podcasts(usize), Feeds, ShowsNew(usize) }`.
`Library` covers the plain mode and the feed-group mode of an Emby library, because both modes
offer the same choices and the same identity. The choices and the default live in
`list_sort.rs`, with no other copy:

| Target                     | Choices                       | Default |
| -------------------------- | ----------------------------- | ------- |
| `Library` (plain)          | Title, Newest, Oldest         | Title   |
| `Library` (feed-group)     | Title, Newest, Oldest         | Newest  |
| `Audiobooks`               | Title, Author, Newest, Oldest | Author  |
| `Podcasts`, `Feeds`        | Title, Newest, Oldest         | Newest  |
| `ShowsNew`                 | Title, Newest, Oldest         | Newest  |

`App::gui_list_sort(target) -> GuiListSort` reads the stored sort. When the store holds none, or
holds a sort outside the target's choices, it returns the default. A stored `Author` on a
library is an example of a sort outside the choices.

### D3. The store keys by `TabIdentity`

`App.gui_list_sorts` is a small `Vec<(TabIdentity, GuiListSort)>` in insertion order. A set
replaces the entry in place or pushes a new one. The key is the stable identity, not `lib_idx`,
for three reasons. It is the "sticky per library" key. The restore can install the whole saved
list at start, before any catalog lands. And a saved sort for a library that is absent this
session survives to the next save. A target resolves to its identity through the existing
`gui_destination_identity`. `ShowsNew(lib_idx)` uses the Shows library's identity, and that
library has no other flat list.

*Alternative*: a `HashMap<GuiDestinationKind, _>` mapped to the identity at save time.
Rejected, because the restore would have to wait for the catalog, and a library that is offline
for one session would lose its sort.

### D4. The date order leaves the collection selection

`GuiCollectionSelection.order`, `CollectionOp::Order`, the `order` half of
`CollectionOp::Restore`, the sites' `has_order`, `stored_order`, and `install_order`, the
accessors `gui_podcast_date_order`, `gui_feeds_date_order`, and `gui_emby_feed_order`, and
`GuiCollections.order` all go away. The projections read `gui_list_sort` instead. This leaves
one owner of list order. It also stops a sort-only change from counting as a stored collection
selection (`has_stored_selection`), so a sort change no longer saves an All entry as a side
effect.

### D5. One wire command replaces `DateOrder`

`GuiLibraryCommand::DateOrder` becomes `ListSort { target: GuiSortTarget, sort: GuiListSort }`
in the core and in `crates/mbv-gui/src/bridge/library.rs`. The dispatch arm in
`src/app/gui_library.rs` is exhaustive. It ignores the command, with a log line through
`log_ignored_library_command`, when the target names no presented flat list (a stale
`lib_idx`, or a `ShowsNew` whose library is not a TV library) or when the sort is not one of the
target's choices.

### D6. The projections sort by the active sort, and rows carry a rail letter only for a name sort

Each flat projection reads `gui_list_sort(target)` once per frame and sorts its rows. Every
sort's last tie-break is the stable id, so equal keys never reorder between frames. Title
compares are case-insensitive (lowercased keys).

- **Plain Emby**: under Title, the list keeps the server's `SortName` order, which is the order
  today. Under Newest and Oldest it sorts with `compare_gui_date_order` over
  `emby_date_added_secs`.
- **Emby feed-group**: under Title it sorts by `effective_sort_str`. The dates stay as they are
  today.
- **Audiobooks**: under Author, the order is today's (author sort key, then title). Under Title
  it sorts by title, then author sort key. The dates use `added_at_ms` (D9), converted to
  seconds.
- **Podcasts and Feeds**: under Title they sort by the episode or entry title, then by date
  Newest. The dates stay as they are today.
- **Shows New**: under Title it sorts by the series name, then the season number
  (`parent_index_number`), then the episode number (`index_number`). The dates stay as they are
  today. Each sort runs before the de-duplication by item id.

Rows carry `sort_letter: Option<char>`, which is `Some` exactly while the list's sort is a name
sort. `GuiEntryRow` and `GuiShowsRow` gain the field. `GuiEmbyRow` already has it.
`GuiAudiobooksRow.sort_letter` becomes optional. The letter comes from the existing
`sort_letter` helper over the key that the sort compares: the sort name, the title, or the
author key. The rail and the headings are thus one fact that the core derives. The window never
re-decides it from the list kind.

### D7. The frames carry the sort, and one segmented control paints it

Every flat list frame (`GuiEmbyList`, `GuiAudiobooksList`, `GuiPodcastsList`, `GuiFeedsList`,
and `GuiShowsList` while the view is New) carries
`sort: Option<GuiListSortFrame { target, active, choices }>`. A new prop-driven component
`crates/mbv-gui/src/window/components/library_sort_control.rs` paints the choices as a
segmented control with the active choice marked. It takes its labels from `GuiListSort::label`.
Every destination already paints the one shared header
(`crates/mbv-gui/src/window/components/library_list_header.rs`). That header gains a
`sort: Option<GuiListSortFrame>` prop and paints the control last on the heading line. The
control is thus the rightmost item and sits directly right of the collection dropdown or the
Shows view dropdown. The five list components only pass their frame's `sort`, so the placement
has one owner. A press calls
`LibraryState::select_list_sort(target, sort)`, which sends the command and resets that list's
scroll store to the top. The control registers no key. `press_date_order`,
`GuiCollectionsFrame::next_order`, and `order_label` go away.

### D8. One heading builder for the flat lists

`crates/mbv-gui/src/window/library/list.rs` gains one generic builder. It puts a letter heading
before each run of rows with the same `sort_letter`, after the filter, and builds flat rows with
no headings when the rows carry no letter. The plain/feed-group Emby list, the Audiobooks list,
the Podcasts list, the Feeds list, and the Shows New list use it. This replaces
`heading_emby_rows`, the Audiobooks heading code, and the `feed_group` switch in
`build_emby_rows`. Each list paints the rail when its rows carry letters. `list.rs` is at 582
lines, so the builder goes in a new child module `list/lettered.rs` if the file would pass 800
lines.

### D9. Audiobooks parses `addedAt`

`AudiobookshelfBook` gains `added_at_ms: Option<u64>` from the item DTO's `addedAt`
(`#[serde(rename = "addedAt", default)]`, beside `updatedAt`). A missing or zero value is `None`,
so that book sorts last in both date sorts. No new request is made: the field rides on the
items page that the catalog fetch already reads.

### D10. The launch state gains `sorts` and migrates `order`

`GuiLaunchState` gains
`#[serde(default)] sorts: Vec<GuiLaunchListSort { destination: TabIdentity, sort: GuiLaunchSort }>`,
where `GuiLaunchSort` is the snake_case enum `title | author | newest | oldest`. The version
stays 1, because the field is additive and an older build ignores it. Save writes the store's
entries in store order. A collection entry no longer writes `order`: the field gets
`skip_serializing_if = "Option::is_none"` and stays readable. Restore installs each saved sort
whose identity the session has not set yet, so a restore never clobbers a choice that the
session already made. Then, for each collection entry with `order: Some(_)` whose destination
has no sort in the file, it installs `Newest` or `Oldest`.

### D11. Domain term

`CONTEXT.md` gains *List sort*: the single-choice order of a flat *GUI Library panel* list
(Title, Author on Audiobooks, Newest, Oldest), sticky per destination. Its *Avoid* line names
date order (for the control) and sort pill. The *Letter rail* entry says that the rail shows
while the list's sort is a name sort.

## Risks / Trade-offs

- [The `addedAt` evidence comes from Audiobookshelf source, not from a probe of the user's
  server.] → `updatedAt` already parses from the same DTO, and the server writes both from the
  same model. A server that omits it gives `None`, and that book sorts last. Nothing fails.
- [The sort persists only through an orderly exit, as the collection selection does.] → This
  is accepted. It is the same write point as every other GUI launch field.
- [Under Title, Podcasts, Feeds, YouTube, and Shows New show a rail and headings that they have
  never shown.] → This is intended (spec rule). The user does the layout pass.
- [Title on the plain Emby list trusts the server's `SortName` order, but the other lists
  lowercase the title locally, so article handling differs ("The ..." files under H on Emby,
  under T elsewhere).] → This is accepted. Only Emby gives a sort name.
- [The header title line now holds a trigger and a three- or four-segment control on a narrow
  panel.] → This is a layout risk, left to the user's visual pass. The spec fixes only the order
  of the items on the line.

## Migration Plan

The change is additive in the launch file. An old file loads, and its feed-based `order` becomes
the destination's sort (D10). An older build that reads a new file ignores `sorts`, and it reads
a missing `order` as Newest. Rollback costs only the saved sorts.
