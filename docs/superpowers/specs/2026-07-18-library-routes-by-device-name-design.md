# Library Routes by Device Name — Design

**Status:** Approved by user, ready for implementation planning.

**Context:** #223 shipped library-based remote routing (`[daemon_routes]` in
`config.toml`), but its config requires typing a raw connection string per
library — `Music = "tcp://musicserver.local:PORT"` or
`"*" = "unix:///run/mbvd.sock"`. That's not something a typical user thinks
in; nobody assigns "which library plays where" in terms of protocols and
socket addresses. This redesign replaces the raw-address config with a
device-name-based model, resolved the same way F3's Sessions panel and
#236's auto-reconnect already resolve a device to a live connection — no new
network-discovery mechanism is introduced.

## 1. Overview

`daemon_routes` is renamed `library_routes`. Its values change from raw
connection strings to **device names** — the same friendly name (defaulting
to hostname) that already appears in F3's session list:

```toml
[library_routes]
Music = "living-room-pc"
```

At connect time, mbv resolves the real address by looking up a currently-live
Emby session with that device name — the exact mechanism
`App::session_direct_endpoint` already uses for F3's "connect to session" and
#236's auto-reconnect `DirectSession` lookup. No raw address is ever typed or
displayed to the user.

This is a config/UX change only. It does not change the underlying
suspend/restore/connect machinery, #222's connect-lifecycle rules (lazy
connect, fallback-with-warning, no retry/parking), or #236's auto-reconnect
behavior.

## 2. Config model

- Section renamed: `[daemon_routes]` → `[library_routes]`. The old name
  implied this was daemon-protocol configuration; it isn't — it's "which
  library goes to which connection," and the connection is resolved via a
  session/device name, not a daemon endpoint.
- Library name: matched case-insensitively, same convention as
  `hidden_libraries`/`feed_view_libraries`.
- Value: a device name (a plain string), matched case-insensitively against
  a live session's `device_name` at connect time — mirroring how #236's
  `DirectSession` lookup already compares names.
- Still hand-editable in `config.toml`, same as `hidden_libraries` today —
  the F2 Settings UI (section 3) is the primary, recommended path, not the
  only one.
- No `"*"` wildcard key. The parser has exactly one shape: library name →
  device name.
- **Breaking change, no migration.** Existing `daemon_routes` entries
  (`tcp://…`, `unix://…`) are invalid under the new format and are
  dropped/warned on like any other config format change — this is a
  single-user repo with no released config-compat guarantee.

## 3. Settings UX flow (F2)

- New settings row, **"Library routes"**, showing a summary of current
  assignments (e.g. `Music → living-room-pc`, or `None set`) — same shape as
  the existing "Hidden libraries" row.
- Opening it shows the full library list (like today's hidden-libraries
  multiselect), but selecting a library opens a **device sub-picker** for
  that one library instead of toggling a checkbox:
  - Fetches the live session list (the same call F3 already makes) and
    filters to sessions that are actual mbv/mbvd instances — the same
    `sess.client.eq_ignore_ascii_case("mbv")` filter `session_direct_endpoint`
    already applies — excluding this machine itself (routing to yourself is
    meaningless).
  - Picking a device assigns that library to it. Picking "Local (no route)"
    clears any existing assignment for that library.
  - If no other mbv devices are currently visible, show an explanatory empty
    state (e.g. "No other mbv devices found right now — make sure the
    target is running and connected") rather than a bare empty list.
- Existing assignments still display correctly even when that device is
  currently offline (e.g. `Music → living-room-pc` shows as configured; it
  just won't connect until that device is back up, per #222's existing
  fallback rule).
- Assignment is **live-list only, no manual/offline entry** — you can only
  assign a route to a device that is currently visible in the session list
  at the moment you set it up.

## 4. Connect-time resolution + error handling

- When playback/enqueue starts from a routed library, mbv looks up the
  assigned device name in `[library_routes]`, then does a live session-list
  lookup using the **unfiltered** session list (the same
  `get_sessions_unfiltered()` added for #236's auto-reconnect fix) so an
  idle-but-still-connected target device isn't wrongly treated as gone.
- Found, and it's a real mbv/mbvd session → resolve its address the same way
  `session_direct_endpoint` does today (host + advertised direct-connect
  port) and connect, exactly like today's flow.
- Not found (device offline, not running, or no longer an mbv session) →
  same fallback #222 already defines: fall back to local playback with a
  warning toast, never a hard error, never an indefinite block.
- The existing "don't mix routes in one queue" rule (reject an enqueue whose
  route conflicts with the current queue's route) is unchanged — it's keyed
  by library name (`active_route: Option<String>`), not by the resolved
  address, so nothing about it needs to change.
- Net effect: starting playback in a routed library now costs one extra
  "list sessions" API call to resolve the device — the same cost
  auto-reconnect's `DirectSession` case already pays, applied to a second
  call site, not a new kind of latency.

## 5. Explicit scope boundaries (non-goals)

- No same-host daemon routing (`unix://`/`Local`) — dropped entirely; #222
  and #223 are remote-connection features only.
- No `"*"` wildcard/catch-all route.
- No migration from the old `tcp://`/`unix://` config format.
- No manual/offline device entry in the picker.
- No new network-discovery protocol (mDNS, etc.) — this reuses the existing
  Emby-session-based resolution mechanism (`session_direct_endpoint`); it
  does not add a new one.
- No change to the underlying connect/suspend/restore machinery, #222's
  fallback rules, or #236's auto-reconnect.
- No device-nickname system — a device's name is still whatever it already
  is today (defaults to hostname via `/etc/hostname`). A user-settable
  nickname independent of hostname is an explicit follow-up candidate, not
  part of this design.

## Acceptance criteria

- `[library_routes]` in `config.toml` maps library name → device name;
  `[daemon_routes]` and its `tcp://`/`unix://`/`"*"` forms are no longer
  recognized.
- The F2 Settings panel has a "Library routes" row; editing it lists
  libraries, and picking one opens a device sub-picker sourced from the live,
  mbv-filtered session list (excluding the local device).
- Assigning a library to a device requires that device to be live in the
  session list at assignment time; existing assignments still display when
  their device is currently offline.
- Starting playback/enqueue from a routed library resolves the device via
  the live, unfiltered session list and connects the same way F3/#236
  already do; a resolution miss falls back to local playback with a warning,
  never a hard error.
- The queue mixed-route rejection behavior is unchanged.

## Related

- #222 — connection lifecycle (lazy connect, fallback + warning, no
  retry/parking) that this design's resolved connections still follow.
- #223 — original library-routing feature; this design changes its config
  and resolution mechanism only, not its playback/queue behavior.
- #236 — auto-reconnect on startup; this design reuses its
  `get_sessions_unfiltered()` addition and mirrors its `DirectSession`
  device-name-matching approach.
