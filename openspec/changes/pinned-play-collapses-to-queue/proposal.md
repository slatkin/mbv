# Proposal

## Why

In the pinned panel, the expanded width shows the library over the tiled windows. When the user
starts playback from it, the panel stays expanded and covers the windows. The user must press `x`
to see what plays. A play that starts now ends the browse, so the panel can collapse to the queue
view without the extra key.

The same work also found that a remote Emby Play command (from the websocket) moves panel focus
to the queue. A remote command must not change local UI interaction.

## What Changes

- While mbv runs in the pinned panel at its expanded width, a user play that starts playback now
  collapses the panel to its collapsed width and shows the queue view with queue focus. This is
  the same view that `x` gives. It applies to Enter, Play all, Shuffle, menu Play and Shuffle,
  saved-playlist Play and Shuffle, and a play that waited on a playability check. It applies to
  every output: local mpv, a Cast receiver, and a directly controlled Emby session.
- The collapse starts only after the Owner accepts the queue replacement. A cancelled confirm
  prompt, a rejected op, or a timeout keeps the expanded view. One user action collapses at most
  one time.
- If the pinned panel refuses the collapsed width, the existing warning toast shows and the
  view stays expanded. Playback continues. A pending pinned request blocks the collapse request.
- Enqueue does not collapse. At the collapsed width nothing changes.
- A remote Emby Play command over the websocket no longer moves panel focus. It never collapses
  the pinned panel. Its toasts stay.

## Capabilities

### New Capabilities

- `emby-remote-play`: how mbv handles a Play command that an Emby remote-control session sends
  over the websocket, and what local UI state it must leave alone.

### Modified Capabilities

- `panel-mode`: *Pinned launches bind x to the pinned views* gains a third trigger for the
  collapsed state: a user play that starts playback while the panel is expanded.

## Impact

- `src/app/dispatch/action.rs`: a collapse-only pinned view step next to `pinned_view_toggle`.
- `src/app/dispatch/actions.rs` (`play_items_routed`, `play_series_continuation` and
  `play_single_item` inside `play_item`, `submit_queue_item`) and
  `src/app/dispatch/queue/pending_playback.rs`: call the step after an accepted replace.
- `CONTEXT.md` *Pinned panel width*: name every trigger that switches the running width.
- `src/app/dispatch/session/ws_event.rs`: remove the panel focus move from the Play handler.
- No ctrl protocol, Owner, pinwin argument, or configuration change.
