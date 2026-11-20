# Proposal

## Why

Every flat GUI Library list has one fixed order. A Movies library reads only by name. A podcast,
feed, or YouTube folder reads only newest first. The core already holds a Newest/Oldest date
order for the feed-based lists, but the GUI paints no control for it (the spec says so today).
The user wants to choose the order of any flat list and wants each library to keep its choice.

## What Changes

- Every flat GUI Library list gets one sort control: a segmented control on the list header's
  title line. It is the rightmost item on that line. Where the header has a dropdown trigger
  (the collection dropdown or the Shows view dropdown), the control sits immediately to the
  right of the trigger.
- The flat lists and their choices:

  | List                              | Choices                        | Default |
  | --------------------------------- | ------------------------------ | ------- |
  | Plain Emby library (Movies, ...)  | Title, Newest, Oldest          | Title   |
  | Audiobooks                        | Title, Author, Newest, Oldest  | Author  |
  | Podcasts                          | Title, Newest, Oldest          | Newest  |
  | Feeds                             | Title, Newest, Oldest          | Newest  |
  | Emby library in feed-group mode   | Title, Newest, Oldest          | Newest  |
  | Shows, New view                   | Title, Newest, Oldest          | Newest  |

  Each default is the list's order today, so no list reorders when the change lands.
- Newest and Oldest sort by the date the list already reads: date added for Emby rows
  (`DateCreated`) and for Audiobooks (the Audiobookshelf item's `addedAt`), the publish date for
  Podcasts and Feeds. Undated rows stay last in both orders.
- Title sorts by the row's title: the Emby sort name, the book title, the episode or entry
  title, and the show name on a Shows New row. Author keeps the Audiobooks order that exists
  today (author sort key, then title).
- A list sorted by Title or Author shows the letter headings and the A-Z letter rail. A list
  sorted by Newest or Oldest shows neither. This is the existing rail rule. It now applies to
  each list's active sort and not to the list's kind, so Podcasts, Feeds, YouTube, and Shows
  New show a rail when you sort them by Title.
- The sort is sticky per destination. Each library keeps its own
  choice while the GUI Client runs and across a restart, in a new `sorts` field of the GUI
  launch state that the destination's stable identity keys. A collection entry no longer
  writes the date order. An old entry's `order` loads as that destination's sort when the file
  holds no sort for it. The Feeds tab is one destination with its own identity.
- The control is pointer-only and binds no key, like the collection dropdown and the Shows view
  dropdown. A sort change keeps the Hero's pick and the filter text, and scrolls the list to its
  top.
- The core's date-order command becomes a list-sort command that names a destination and a sort.
  A sort that the destination does not offer (Author outside Audiobooks) is an ignored command.
- Out of scope: Home, the Music tree, the Shows All tree, Shows Upcoming (it stays soonest
  first), Search results, and the Playlists screen. The TUI Client does not change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-client`: "Feed-based lists paint newest first" becomes a requirement for the sort control
  of every flat list. "The collection selection persists per destination" stops carrying the
  date order, and a new requirement makes the list sort persist per destination. "The letter
  rail jumps a name-sorted list" follows the active sort. "The collection dropdown sits in the
  list header" admits the sort control at the right end of the title line. "Shows paints the New
  and Upcoming lists" lets the New list follow its sort.

## Impact

- `src/app/gui_library/`: a new `GuiListSort` type and its per-destination store keyed by the
  destination's `TabIdentity`, and its choices and default for each flat list. The date order
  leaves `GuiCollectionSelection` and the collection sites. `GuiLibraryCommand::DateOrder` is
  replaced by a list-sort command. The plain Emby, Audiobooks, Podcasts, Feeds, Emby feed-group,
  and Shows New projections sort by the active sort, and their rows carry a rail letter only
  under Title or Author. `launch.rs` saves and restores the sorts and migrates an old `order`.
- `crates/mbv-config/src/gui_launch_state.rs`: a new `sorts` list (serde default empty). The
  `order` field of a collection entry stays readable and is no longer written. The version stays
  1.
- `crates/mbv-audiobookshelf/src/catalog_books.rs`: `AudiobookshelfBook` parses the library
  item's `addedAt`.
- `crates/mbv-gui/src/bridge/`: the frames of the six flat lists carry the active sort and its
  choices. The wire command replaces `DateOrder`.
- `crates/mbv-gui/src/window/`: a new prop-driven segmented sort control in the list headers.
  The flat list builders paint letter headings and the rail from the active sort. The unused
  `press_date_order` goes away.
- No new dependency.
