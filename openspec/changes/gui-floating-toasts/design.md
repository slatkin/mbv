# Design

## Context

See proposal.md (Why) and the delta specs for the behavior.

Today the Client core keeps one toast slot on `App`: `status: String`,
`status_expires: Option<Instant>`, and `status_severity: ToastSeverity`
(`src/app/state/app_struct.rs`). `App::flash` (`src/app/dispatch/notify.rs`)
overwrites the slot and starts the desktop notification. About 190 call sites
call `flash`. `App::toast_presentation`
(`src/app/state/projection/chrome_status.rs`) gates the slot on liveness and
desktop delivery (`system_notifications`, the global `notif_failed` flag) and
gives `Option<StatusToast>`. `StatusBarProjection` carries it to both Clients.

The GUI copies the projection into `GuiStatusBar { prefix_armed, toast }`
(`crates/mbv-gui/src/bridge/frame/chrome.rs`), the compared `Status` section
(`crates/mbv-gui/src/sections.rs`). Three components paint the toast inline:
`queue_footer.rs`, `library_status_bar.rs`, `settings_status_bar.rs`. The
footer drops its `PREFIX` badge while a toast is live.

Other writers of the slot:

- `player_event.rs`: `PlayerEvent::SkipIntroPlay`, `PlayerEvent::MpvQuit`, and
  the stopped path clear the slot.
- `player_event.rs`: `if self.status.starts_with("Next up:")` clears the slot.
  No code flashes a "Next up:" toast any more, so this branch is dead.
- `client_startup.rs`: clears the slot when it is not live. This only removes
  expired text, which the expiry pass already does.
- `App::expire_status_toast`, called from the TUI sync pass
  (`src/app/shell/chrome_panels.rs`).

About 100 test lines in about 30 test files read `app.status` or
`app.status_severity` to assert the newest toast.

Blitz 0.3.0-beta.4 honours `pointer-events: none` in its hit test
(`blitz-dom` `node.rs`), and the window already uses it for the scrubber
hover and the queue insertion line. Window presses go to the core hit map;
a component that reports no region takes no press.

## Goals / Non-Goals

**Goals:**

- One list of live toasts in the Client core, with the cap, the repeat rule,
  and per-toast expiry in one place with unit tests.
- One floating component that paints the list in the GUI.
- The TUI keeps its current status row behavior with the smallest change.

**Non-Goals:**

- No enter or exit animation. Cards appear and go at once.
- No per-toast desktop delivery state. The `notif_failed` flag stays global,
  as today.
- No change to which messages are toasts, their text, or their severity.

## Decisions

### D1. `ToastList` replaces the slot fields

A new private module `src/app/state/toasts.rs` holds `ToastList`, and
`App` gets one field `toasts: ToastList` in place of `status`,
`status_expires`, and `status_severity`.

```text
ToastList { entries: Vec<LiveToast>, next_id: u64 }   // oldest first
LiveToast { id: ToastId, text: String, severity: ToastSeverity, expires: Instant }

push(text, severity, now)
  1. drop entries whose expires <= now
  2. drop the entry with the same text and severity, if there is one
  3. if 5 entries remain, drop the oldest
  4. append the new entry, id = next_id++, expires = now + severity.ttl()
live(now) -> iterator over entries with expires > now, oldest first
expire(now)  -> drop entries whose expires <= now
clear()      -> drop all entries
```

`MAX_LIVE_TOASTS: usize = 5` is a constant in that module. The module takes
`now` as a parameter, so its tests need no clock and no sleep.

Alternative: keep the slot and add a second list for the GUI. Rejected: two
stores of the same fact can disagree.

### D2. `ToastId` and `ToastCard` in `mbv-ui-model`

`crates/mbv-ui-model/src/toast.rs` gains:

```rust
pub struct ToastId(pub u64);                    // Copy, Eq, Hash, Debug
pub struct ToastCard { pub id: ToastId, pub toast: StatusToast }
```

`StatusToast` does not change, so the TUI path does not change. The id
gives each GUI card a stable key: a card that stays does not remount when an
older card goes, and a repeated toast gets a new id, so its card moves to the
bottom.

### D3. Presentation reads

In `chrome_status.rs`:

- `fn renders_in_app(&self, severity) -> bool`: Neutral, or
  `!system_notifications`, or `notif_failed`. This is the current gate, now
  per toast.
- `toast_presentation(&self) -> Option<StatusToast>` (TUI): the newest live
  toast that renders in-app.
- `toast_cards(&self) -> Vec<ToastCard>` (GUI, `pub`, re-exported from
  `lib.rs`): every live toast that renders in-app, oldest first.

### D4. `flash` and the clear sites

- `flash` calls `self.toasts.push(msg, severity, Instant::now())` after the
  desktop notification attempt. Its signature does not change, so the 190
  call sites do not change.
- `expire_status_toast` becomes `self.toasts.expire(Instant::now())`.
- `SkipIntroPlay`, `MpvQuit`, and the stopped path call
  `self.toasts.clear()`. This keeps today's behavior: they cleared the one
  slot, which was all toasts.
- The dead "Next up:" branch is deleted: the field it reads is gone, and no
  producer exists.
- The `client_startup.rs` clear is deleted: `live()` never shows an expired
  toast, so it has nothing to do.
- `status_toast_live` is deleted when it has no caller.

### D5. Test reads

`App` gets `#[cfg(any(test, feature = "test"))]` accessors `newest_toast_text()
-> &str` (empty when none) and `newest_toast_severity() -> ToastSeverity`
(default when none). They read the newest entry whether or not it expired,
as the old field reads did. The existing tests change `app.status` to
`app.newest_toast_text()` and `app.status_severity` to
`app.newest_toast_severity()`. Tests that set the fields by hand
(`chrome_status.rs`, `home.rs`) use `toasts.push` with an explicit `now`.

### D6. GUI section `Toasts`

- `GuiStatusBar` loses its `toast` field; it keeps `prefix_armed`.
  `GuiToast` gains `id: ToastId` and is built from `ToastCard`.
- A new compared section `Section::Toasts` carries `GuiToasts(Vec<GuiToast>)`,
  built each tick from `app.toast_cards()` and sent only when it differs. A
  compared section rebuilds on the idle-cadence tick, so expiry needs no new
  deadline code (the same way the toast left the `Status` section before).
- The window keeps the section in its own signal, as for `Status`.

### D7. `ToastStack` component

`crates/mbv-gui/src/window/components/toast_stack.rs` paints the list. The
root (`window/app.rs`) mounts it once, last, after the overlay match inside
the attached branch, so it paints over the panels, the Settings screen, and the
modal overlays. It mounts nothing while the list is empty.

Layout and paint:

- Container: `position: absolute; right: 16px; bottom: 52px` (over the 40 px
  footer band with a 12 px gap); `z-index: 40` inline (the modals use
  `z-30`); `display: flex; flex-direction: column; gap: 8px;
  align-items: flex-end`; `pointer-events: none` on the container and every
  card. Width `min(360px, 100% - 32px)` so a narrow pinned panel fits.
- Card: `--canvas-popover` background, 1 px `--canvas-popover-border`,
  `--canvas-popover-shadow`, 6 px radius, 10 px by 12 px padding, 13 px text
  that wraps, a 3 px left border in the severity colour from
  `window::status::toast_color`, and a 16 px severity icon in that colour.
- Icons: Neutral `Info`, Success `CircleCheck`, Warning `TriangleAlert`,
  Error `CircleX`, ported from `@lucide/svelte` 1.70.0 the same way as the
  existing icons, in a new child module `window/icons/toast.rs` (because
  `icons.rs` is at 710 lines).
- Each card has `key: "{id}"` and the marker `data-toast-card` with the
  severity in `data-toast-severity`, so tests query it.

The cards report no layout region, so the core hit map never names them, and
`pointer-events: none` keeps the CSS hover veil and the pointer target on what
is under them.

### D8. Status lines drop the toast

- `queue_footer.rs`: the centre paints only the armed `PREFIX` badge; the
  pill-drop rule goes.
- `library_status_bar.rs` and `settings_status_bar.rs`: they paint only the
  armed state after the field.
- `window::status::toast_color` stays; the toast stack is now its only
  caller.

### D9. Docs

`docs/architecture/gui-client.md` ("The Status bar in the footer line") and
the `CONTEXT.md` *GUI Library panel* bottom-bar entry change to say the status
lines show no toasts and name the floating toast stack. The
`toast-notification-semantics` Purpose line is updated at archive to cover
both Clients.

## Risks / Trade-offs

- [Several cards cover the queue totals and rows for up to 5 s] → The cards
  take no pointer input, so the rows under them still work. The cap of 5
  limits the height.
- [Idle-cadence expiry: a card can stay up to one cadence after its expiry]
  → This is the same delay the inline toast has today.
- [`notif_failed` is global: after one failure, all later toasts show
  in-app] → This is today's behavior; no change.
- [`app.rs` is at 773 lines] → The root adds one component call (about 3
  lines). The component and its props live in their own file.
- [About 100 test lines change] → The change is mechanical through the D5
  accessors.
