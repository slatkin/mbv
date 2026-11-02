# Proposal

## Why

The user wants to watch Twitch in mbv. mbv has no way to play a URL that does not come from a Service. mpv already resolves Twitch URLs through its built-in yt-dlp hook (the same path that plays YouTube Feed entries today). So mbv needs only an entry point and a queue item, not a stream resolver.

## What Changes

- A new configurable keybind action opens a TUI text prompt. The user pastes a Twitch URL and presses Enter. mbv plays the URL at once, through the same play-now path as other items.
- The prompt accepts only URLs whose host is Twitch. It refuses other hosts. It does not examine the URL form further (live channel, VOD, and clip URLs all pass).
- A new QueueItem kind, **Twitch item**, holds the URL and a title. It lives in the canonical queue of the Owner like every other item. Pause, play, stop, the queue panel, and MPRIS act on it.
- The Player owner gives the URL to mpv exactly as given. mpv and yt-dlp resolve it. Duration and seek are what mpv reports.
- A Twitch item has no resume position and no played state.
- Cast and Emby Session outputs refuse a Twitch item. Only mpv plays it.
- A new additive ctrl capability negotiates Twitch item transport. Peers without it do not receive Twitch slots and cannot submit them. `CTRL_PROTOCOL_VERSION` does not change.
- `CONTEXT.md` gets the term *Twitch item*.

## Capabilities

### New Capabilities

- `twitch-url-playback`: the Twitch URL prompt, the host check, the Twitch item, its playback through mpv, and its refusal by Cast and Emby Session outputs.

### Modified Capabilities

- `ctrl-protocol`: adds the additive Twitch queue capability and its per-peer gating.

## Impact

- `crates/mbv-queue/`: new `QueueItem::Twitch` variant, kind, content identity, serde tag, admission.
- `crates/mbv-player/`: mpv URL source for a Twitch item.
- `crates/mbv-ctrl/`, `crates/mbv-daemon/`, `crates/mbv-remote-player/`, `crates/mbvd/`, `protocol/ctrl/v10/`: new capability, slot filter, submission refusal, fixtures.
- `crates/mbv-daemon/src/cast_resolve.rs`: Twitch item is uncastable. `emby_session/load.rs` already refuses non-Emby items.
- `crates/mbv-keybinds/`, `src/app/input/`, `src/app/dispatch/`: new keybind action.
- `crates/mbv-components/`, `crates/mbv-render/`, `crates/mbv-ui-msg/`: the prompt component and painter.
- Runtime dependency: `yt-dlp` on the host that runs the Player owner. mbv adds no Rust dependency.
- Persisted `queue_state.json` can now hold a `Twitch` tag. An older mbv binary cannot load a queue that contains one.
