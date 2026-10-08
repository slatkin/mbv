# Proposal

## Why

Playing a TV episode on a watched remote Emby session renders the queue card's title overlay differently from local playback: no show logo in the upper-left corner, and the episode name as a single top row instead of the show-context/logo + bottom-episode-title layout local playback draws. Both paths draw the same overlay surface, so a user switching between local and remote playback sees an unexplained presentation change. Local behavior is correct and stays as-is.

## What Changes

- The Emby session parser extracts `SeriesName` and `ImageTags.Logo` from the session's `NowPlayingItem` into `SessionInfo` (new optional fields). Absent fields degrade gracefully to today's behavior.
- A watched remote session's now-playing item that is an Episode with a series name draws the same two-part title as local playback: show name as the context part, episode name as the title part.
- A watched remote session's now-playing item resolves a logo owner exactly like the local path: an Episode with a series id fetches the series logo (`{series_id}:Logo`); a Movie with a session-advertised logo etag fetches its own logo (`{id}:Logo:{etag}`). The logo replaces the top text row under the existing overlay rules.
- Cast attachments keep today's one-part title (a Cast now-playing name carries no series context).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `queue-artwork-title-overlay`: the "Playing on a remote target" scenario currently fixes a remote session's overlay to a one-part title drawn from the session's now-playing name; it changes to draw the same two-part title and logo the local path draws whenever the session payload carries the needed facts, degrading to today's one-part text when it does not.

## Impact

- `crates/mbv-emby`: `SessionInfo` gains two optional now-playing fields; `client_sessions.rs` parses them.
- `src/app/shell/playback.rs`: `slotless_playback_title_parts` builds a two-part title for Emby-session episodes.
- `src/app/state/projection/card.rs`: the slotless (remote-session) projection resolves a logo owner and issues the logo fetch, mirroring the local path.
- No protocol, queue-authority, or player changes. The overlay composer and painter are already item-kind agnostic; only the projection feeds them.
