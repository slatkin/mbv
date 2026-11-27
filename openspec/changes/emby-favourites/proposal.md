# Proposal

## Why

mbv can mark Emby items played but cannot favourite them. Emby clients treat favourites as a
first-class way to keep a short list per library, and a user who favourites in another Emby client
cannot see or change that list in mbv. The official Emby web client (4.10.2.0, read from the
user's own server) defines which items can be favourited and where each library lists them; this
change follows that behaviour instead of inventing a model.

## What Changes

- The right-click menu of an Emby library row gains **Add to Favourites** / **Remove from
  Favourites** on every item Emby allows to be favourited: Movie, Series, Episode, BoxSet, Audio,
  MusicAlbum, MusicArtist, Playlist, Video, and the other types the Emby client's `canRate` rule
  passes. Season, Folder, CollectionFolder, Channel, Program, Timer, SeriesTimer, UserView,
  Studio, and Tag never get the entry. The entry follows the item's current state.
- The Emby client gains the write: `POST /Users/{id}/FavoriteItems/{itemId}` to set and
  `POST /Users/{id}/FavoriteItems/{itemId}/Delete` to clear, as the Emby web client sends them.
- `EmbyItem` carries `UserData.IsFavorite`.
- The GUI Shows view dropdown (All, New, Upcoming) gains two entries, **Favourite Shows** and
  **Favourite Episodes**, as Emby's TV Favorites tab separates them. Favourite Shows lists the
  library's favourite Series in Title order; Favourite Episodes lists its favourite Episodes in
  Title order. Every other non-music Emby library destination in the GUI (plain libraries such
  as Movies) gains a new view dropdown with All and Favourites. Favourites lists that library's
  favourites only: collections first (Series, BoxSet, MusicArtist, MusicAlbum), then media items
  (Episode, Movie, Audio, Video), each group in Title order.
- A Grouped Music library is excluded entirely: it gains no view dropdown, no favourites entry
  on any of its rows, and no Hero heart. A music library that is not grouped (a plain-level
  music library) is not excluded.
- Every non-music Emby Hero gains a heart control in its upper right that shows the item's
  favourite state and toggles it through the same write as the menu entry.
- No cross-library favourites section exists. Home paints none.

Out of scope: a favourite mark on rows, bulk (multi-selection) favouriting, favourite entries on
Queue rows, any favourites affordance on a Grouped Music library (its view dropdown, its rows'
menus, and its Heroes), favouriting Persons,
Live TV channels, and Audiobookshelf or Feed items, and following a favourite change made by
another client before the next refetch.

## Capabilities

### New Capabilities

- `emby-favourites`: which Emby items can be favourited, the menu entry and its write, and the
  per-library Favourites list.

### Modified Capabilities

- `context-menu`: the standard action set gains the favourites entry after the played-state entry;
  three scenarios that list a full menu gain the entry.
- `gui-client`: the Shows view dropdown has five entries; every non-music Emby library
  destination has an All/Favourites view dropdown; a Grouped Music library paints no view
  dropdown. Non-music Emby Heroes carry the heart control.

## Impact

- `mbv-emby-model`: `EmbyItem` field `is_favorite` (`#[serde(default)]`, additive to serialized
  queue items) and a `supports_favourite` rule.
- `mbv-emby`: `parse_item`, a favourite write, and a favourites list fetch (Items plus the Artists
  listing for music libraries).
- `mbv-client-core`: two `ContextAction` variants, the menu entries, and the action's execution,
  parallel to `MarkPlayed` / `MarkUnplayed`. The TUI's shared menu builder gets the entry too; the
  TUI gains no Favourites view.
- `mbv-gui`: the generalised library view type and dropdown, the favourites store, projection,
  frame part, list component, and Hero/play/enqueue/menu resolution for favourite rows.
- `mbv-config`: the GUI launch state keeps writing Shows views as before; no Favourites view is
  ever persisted.
- No Owner change: the write is a Service call from the Client, as Mark Played is today. The
  ctrl v10 schema additively carries `is_favorite` on queue items, because queue items serialize
  `EmbyItem`; it is not a protocol behaviour change.
