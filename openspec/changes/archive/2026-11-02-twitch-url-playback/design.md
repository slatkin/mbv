# Design

## Context

See `proposal.md` (Why) and the specs in this change for the behavior.

Facts in the code today:

- `QueueItem` (`crates/mbv-queue/src/items.rs`) has three variants: `Emby`, `Feed`, `Audiobookshelf`. Serde uses a flat, internally tagged shape (`QueueItemSerialize`) and a manual decode that lists the known tags. The schema derives from `QueueItemSerialize`.
- The Player builds the mpv URL in `mpv_url_for_queue_item` (`crates/mbv-player/src/run/queue.rs`) from `MpvUrlSource`. `prepare_source` (`crates/mbv-player/src/sources.rs`) sends Emby and Feed through `PreparedSource::plain`.
- A Feed entry with a YouTube `link` already plays through mpv's built-in yt-dlp hook. `--load-scripts=no` (`crates/mbv-player/src/mpv_session/spawn.rs`) does not turn the built-in hook off. The same path resolves Twitch URLs.
- The shell play-now path is `App::submit_queue_item(item, true)` (`src/app/dispatch/actions.rs`), as `play_feed_entry` uses it.
- `SavePlaylistComponent` (`crates/mbv-components/src/save_playlist.rs`) is a modal text prompt with a dimmed backdrop, buttons, and a typed intent Msg. It is the model for the new prompt.
- Cast source resolution (`crates/mbv-daemon/src/cast_resolve.rs`, `prepare_cast_source`) matches every `QueueItem` variant. Emby Session admission (`crates/mbv-daemon/src/emby_session/load.rs`, `admit`) already refuses every non-Emby item.
- Per-peer queue filtering lives in `unified_queue_state_for_peer` (`crates/mbv-daemon/src/control_queue.rs`) and `broadcast_state_gated` (`crates/mbv-daemon/src/ctrl.rs`). `abs_queue_transport_rejection` is the model for a submission refusal.

## Goals / Non-Goals

**Goals:**

- A Twitch item is a first-class `QueueItem` variant. Every exhaustive match names it.
- The host check lives in the source-of-truth type, not in the component.
- No new Rust dependency. No stream resolver in mbv.

**Non-Goals:**

- Twitch chat, channel browse, follows, login, or any Twitch API call.
- Stream quality selection, ad filtering, or low-latency tuning.
- Title lookup from Twitch or from mpv `media-title`.
- Any entry point outside the TUI.

## Decisions

### D1. `QueueItem::Twitch(TwitchItem)` in `mbv-queue`

`TwitchItem { url: String, title: String }`, serde tag `"Twitch"`. Kind `QueueItemKind::Twitch`. Content identity `QueueItemContentId::Twitch(url)`. `required_service()` is `None`. `media_kind()` is `"Video"`, so audio-only owners refuse it through the existing rule. `duration()` is `None`. `playlist_item_id()` is empty.

*Alternative:* an ad-hoc `FeedEntry` with `feed_id: None`. Rejected by the user: a Twitch URL is not a FeedEntry (`CONTEXT.md`), and Cast would try to cast the page URL.

### D2. Host check in `TwitchItem::from_url`

`TwitchItem::from_url(&str) -> Result<TwitchItem, TwitchUrlError>` trims the input, requires an `http` or `https` scheme, takes the authority up to the first `/`, `?`, or `#`, drops any port, lowercases it, and accepts `twitch.tv` or a host that ends in `.twitch.tv`. It does not examine the path. `TwitchUrlError` is a domain error type (no `thiserror`). The parse is a small hand split, the same approach as `url_authority_and_path` in `mbv-feed`. mbv-queue gets no `url` crate.

The title is the path without the leading `/` and without query or fragment (`somechannel`, `videos/123456`). An empty path gives the host.

*Why here:* AGENTS.md puts admission and validation outside components. The shell calls `from_url` on submit. The component only edits text.

### D3. mpv receives `item.url`

Add `MpvUrlSource::Twitch(&TwitchItem)`. `mpv_url_for_queue_item` returns `item.url` unchanged. `prepare_source` sends it through `PreparedSource::plain`. The Player adds no yt-dlp option. mpv defaults apply.

### D4. No playback state

Every Feed-state and Audiobookshelf-progress site that matches on `QueueItem` gets an explicit no-op arm for `Twitch` with a short reason comment. No wildcard arm (AGENTS.md). Invariant 06 (`docs/invariants/06-queue-progress-application-sites.md`) gets one line: a Twitch item applies no progress at any site.

### D5. Outputs

`prepare_cast_source` returns `Uncastable` for `QueueItem::Twitch`, through a new `CastError` constructor that names the item. Emby Session `admit` needs no code change. Its let-else already refuses the item and names it.

### D6. Ctrl capability `twitch-queue`

Additive, no version bump (recipe *Add a ctrl protocol variant*, step 1). Add `CTRL_CAP_TWITCH_QUEUE = "twitch-queue"`, advertise it in `CtrlHello::current()` and `DaemonHelloCaps`, add `supports_twitch_queue`. Record it in `HelloClassification` and on the client record. Filter Twitch slots for peers without it in both filter sites. Add `twitch_queue_transport_rejection` next to `abs_queue_transport_rejection` and call it at the same sites.

The Client also checks the capability of its Player owner before it submits (capability first, rejection as backstop, as the ctrl-protocol spec requires). If the owner lacks it, the shell shows a refusal and sends nothing.

Packaged `mbvd` runs the same `mbv-daemon` library, so it admits and plays Twitch items when its host has `yt-dlp`. `daemon_admits` needs no Twitch branch beyond the generic rule.

### D7. Prompt component and keybind action

- Keybind action `twitch_url_open`, section Global, default chord `Shift+T` (unused today), rebindable. Follow recipe *Add a keybind action*.
- New `TwitchUrlComponent` in `crates/mbv-components/` with the shape of `SavePlaylistComponent`: owns the input text, button hit regions, and gestures. It sends `ShellRequest::TwitchUrlIntent(TwitchUrlIntent::Submit(String) | Cancel)` through `mbv-ui-msg`.
- The shell owns open and close (mount and unmount through the TuiRealm focus stack). On `Submit`, it calls `TwitchItem::from_url`. On error, it pushes the refusal into the component (`push_*`) and leaves it mounted. On success, it unmounts the prompt and calls `submit_queue_item(QueueItem::Twitch(item), true)`.
- The painter lives in `crates/mbv-render/` beside `render_save_playlist_content`.

### D8. Exhaustive arms across the workspace

Adding the variant breaks every `match` on `QueueItem` at compile time. Each site gets a deliberate arm: display name and title parts (title only, no context part), queue row presentation, multi-select and context menu actions (Twitch rows offer the generic queue actions only), and any Emby- or Feed-specific action (documented no-op).

## Risks / Trade-offs

- [`yt-dlp` missing on the Owner host] → mpv fails the load. mbv reports it as a failed load of that slot (spec). No pre-check.
- [Twitch ads] → yt-dlp does not filter ad segments. The user can see Twitch ad screens. Accepted; out of scope.
- [Live stream has no end] → a Twitch item in the middle of a queue blocks the items after it until the user skips. Play-now puts it in front, so this is the expected case.
- [Downgrade] → an older mbv cannot decode a persisted queue with a `Twitch` tag. The existing decode error path applies. Accepted.
- [Title is only the URL path] → no channel display name. Accepted for "no more than the URL".

## Migration Plan

None. The variant and capability are additive. Old queues load unchanged.
