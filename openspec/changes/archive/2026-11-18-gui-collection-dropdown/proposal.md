# Proposal

## Why

The GUI Library panel shows its collections as a row of pills. A destination with many collections
(16 YouTube folders, every Podcast show, every Feeds subscription) overflows the row, and the row
selects only one collection at a time. The `mbv Svelte Player` artboard (page 7) replaces the row
with one dropdown filter that checks one or more collections. The feed-based destinations also have
no way to read their list oldest first.

## What Changes

- **BREAKING (GUI only)**: the pill row goes away in every GUI destination. A collection dropdown
  replaces it in Music, Podcasts, Feeds, and an Emby library in feed-group mode (YouTube). The TUI
  does not change.
- The dropdown selection is **All** or one or more checked collections, never a mix. Check a
  collection: All unchecks. Uncheck the last collection: the selection goes back to All. Check All:
  every collection unchecks. The list is the de-duplicated union of the checked collections.
- The dropdown trigger shows a **Clear** control while the selection is not All. Clear sets All.
- The open dropdown has a filter field that narrows its collection rows.
- The dropdown is pointer-only. It adds no keybinding. While the filter field is open it takes the
  typed keys, and Esc closes the dropdown.
- **All** replaces the Podcasts and Feeds **Latest** pill. Music gains **All**: today one group is
  always selected.
- The Podcasts state pills (All, Unplayed, Played) go away in the GUI, as the Feeds state pills did.
- The Emby feed-group **New** pill goes away for now. The TV destination gains its own single-choice
  **New** view instead.
- YouTube, Podcasts, and Feeds get an **Oldest / Newest** toggle beside the trigger. Newest is the
  default. Undated rows stay last in both orders. A YouTube folder no longer reads oldest first by
  default.
- Shows loses its old pill row and gains one dropdown with the single choices **All**, **New**, and
  **Upcoming**. All is the default and is the current series list with its Hero and Workspace. New
  is the old Shows Latest episodes list, and Upcoming is the old Shows Upcoming premieres list; both
  merge every TV library and use their existing data sources and row types. This is a view selector,
  not a collection filter and not a key binding.
- Upcoming rows in Shows are display-only: a press does not select them, they do not play or enqueue,
  and the Hero keeps its item. Selecting a dropdown entry replaces the active view and closes the
  popover.
- GUI Home has no Latest episodes or Upcoming section: those two lists live in the Shows destination.
- Every collection destination keeps its selection across a restart through the same GUI launch
  entry: Music groups, Podcast shows, Feeds subscriptions, and YouTube folders, with the date order
  where one is held. An old Music-only entry still loads. The Shows view persists in its own launch
  field; an old file without the field loads as All.
- The Library Status bar location names All, or the checked collections joined.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: the pill requirements ("One collection pill is selected at a time", "The pill
  selection persists per destination", "Each collection pill keeps its last picked album") become
  collection dropdown requirements. New requirements cover the date order toggle, the Shows
  single-choice view dropdown with its New and Upcoming lists, and the dropdown filter field among
  the open text fields that keep the keys. The GUI Home sections are removed. The Library Status bar
  location names the checked set.

## Impact

- `src/app/gui_library/`: a shared collection-set type and date order in the core; the Music,
  Podcasts, Feeds, and Emby feed-group selections move to it; the commands that set a pill are
  replaced; Shows gains a separate view-selection type and command, and its New and Upcoming lists;
  Home loses the two sections.
- `src/app/dispatch/feeds/feed.rs`: a folder fetch that does not move the TUI-shared feed-group
  selection, so the GUI can load more than one folder.
- `crates/mbv-gui/src/bridge/`: one collection frame replaces the per-destination pill frames.
- `crates/mbv-gui/src/window/`: a prop-driven dropdown presentation component serves the collection
  dropdowns and the Shows view dropdown; a new collection dropdown component replaces `LibraryPillRow`
  and the five destination pill components; the window holds the dropdown's open state and filter
  text.
- `CONTEXT.md`: the *Collection pill* term becomes *Collection dropdown*.
- `crates/mbv-config` GUI launch state: each entry adds a collection `target` and an optional date
  `order`; `group_ids` is still a list and an empty list means All. An entry without the new fields
  loads as Music. The snapshot gains a separate Shows `shows_view` field, whose absence loads as All.
- No new dependency.
