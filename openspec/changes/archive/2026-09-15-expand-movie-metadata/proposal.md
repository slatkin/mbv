## Why

A Movie's Wide hero is the thinnest detail surface in mbv: release date, runtime, and the overview
paragraph. The Emby server already holds the rest — the full genre list, provider links (IMDb, TMDb,
TheTVDB, Trakt), directors, and cast — and returns all of it in the same list response mbv already
issues once `ExternalUrls` joins the `Fields` list. The detail is fetched and discarded today; this
change keeps it and renders it.

## What Changes

- The Movie Wide hero's overview Main content box gains a cast & crew table under the overview text:
  two aligned columns, no header row and no rules, every director first, then the top 9 cast, each row
  a name and a role. Roles come from Emby's `People`; when the provider leaves `Role` empty the person
  type is shown instead.
- The Movie hero's metadata rows gain **Genres** (every genre, not just the first) and a **Link** row
  carrying the item's IMDb provider link only (**reversed 2026-09-15**: originally every provider name,
  e.g. `IMDb  TMDb  Trakt` — narrowed to IMDb-only by user direction, other providers' links are not
  shown), after the existing release-date and runtime rows.
- The link name opens the provider URL in the user's browser when clicked (**reversed 2026-09-15** from
  the original OSC 8 design below: terminal-side ctrl-click is unavailable while the application
  captures the mouse, so the click is resolved by the Library panel against the label geometry it
  painted and handled by the same system-opener path the feed link uses, `xdg-open`/`open`/`start` per
  platform). The URL is opened only when it is a sanitized `http`/`https` URL with no control bytes.
- `EmbyItem` carries `people`, `genres`, and `external_urls`. The first-genre-only `genre` field is
  replaced by `genres`, and the unused `director` field is replaced by `people`.

Non-goals:

- TV Series, home videos, Music, Audiobookshelf and Feeds heroes keep today's content — the new rows
  and the cast table are keyed on the Emby item type `Movie`, not on the destination.
- The Narrow inline hero keeps its current shape: it renders the new metadata rows as plain text, but
  no cast table and no clickable links. This is a Wide Hero pane change only.
- No clickable links outside the hero pane, and no keyboard binding for opening a provider link.
- The `idle-feed-rotation` OSC 8 feed-title requirement stays unimplemented. It can share the same
  escape helper later; implementing it is not part of this change.

## Capabilities

### New Capabilities

None. This extends the existing Library panel presentation.

### Modified Capabilities

- `library-panel`: the overview Main content box renders when the item has overview text **or** Movie
  credits, and the Movie hero content gains genres and links metadata rows plus a cast & crew table
  that link names make clickable through OSC 8 hyperlinks.

## Impact

- `crates/mbv-core/src/api_types.rs`, `crates/mbv-core/src/api_types_parsing.rs` — `EmbyItem` gains
  `people`, `genres`, `external_urls`; `genre` and the unused `director` field go.
- `crates/mbv-core/src/api_client_library.rs`, `crates/mbv-core/src/api_client_playlists.rs` —
  `ExternalUrls` joins the two `Fields` query strings.
- `src/app/components/library_panel/hero.rs` — the Movie branch of the shared Emby hero producer emits
  the genres row, the links row, and the credits.
- `src/app/components/library_panel/content.rs` — `HeroContent` gains the optional credits list;
  `HeroFacts` gains the typed links row.
- `src/app/components/library_panel/hero_header.rs` — the overview box grows the credits table, and the
  links row is painted with the OSC 8 escape.
- `src/app/render/components/hero_model.rs` — the Movie meta rows.
- `src/app/render/arrangements/wide_hero.rs` — the shared text painter's per-row cell writing.
- 39 `EmbyItem { .. }` literal sites across `src/` and `crates/` test modules absorb the new fields.
- No new dependency, no new request, no new routing site, and no change to keyboard policy.
