# mpv Owns the Skip-Intro and Next-Up Prompts

**Status:** current as of `remove-legacy-keyboard-endpoint`.
**This is a deliberate removal, not a gap.** Do not re-add a TUI prompt for
either of these without reading the "Re-adding" section below.

## What was removed

The skip-intro and next-up decisions used to have three simultaneous user
interfaces. Two of them are gone.

| UI | Status |
| --- | --- |
| mpv on-screen button (Lua) | **kept — sole interface** |
| TUI status-bar `(Y/n)` prompt | removed |
| Desktop notification with actions | removed |

## Why

mpv already renders a complete, clickable prompt for both, and drives its own
lifecycle without the TUI:

| | skip intro | next up |
| --- | --- | --- |
| Shown by | `script-message mbv-skip-intro <end_secs>` (`player_runtime.rs`) | `PlayerCommand::NextUpShow` → `script-message mbv-next-up` |
| Drawn by | `scripts/mbv_intro.lua` | `scripts/mbv_visibility.lua` |
| Accepted by | `mbtn_left` → seeks locally, emits `mbv-skip-intro-play` | `mbtn_left` → emits `mbv-next-up-play` |
| Self-dismisses on | `seek` | `start-file` |
| Cancelled by | `PlayerCommand::SkipIntroDismiss` | `PlayerCommand::NextUpDismiss` |

The TUI prompt added no capability, and cost three things:

1. **A status-bar prompt.** It wrote `App.status` with
   `status_expires = None` — the same field toasts use, with the TTL disabled
   as a sentinel meaning "this is a prompt." Prompts and toasts are different
   things (`toast-notification-semantics`); sharing one slot is what made them
   indistinguishable.
2. **An invisible modal that stole focus.** `PlaybackPromptComponent` was
   mounted *and* made `application.active()` whenever the prompt state was set,
   but rendered only when desktop notifications were off or had failed. With
   notifications working, an invisible component owned focus and swallowed
   every keystroke — pressing `q` during a skip-intro window dismissed the
   prompt instead of quitting.
3. **A fourth input path.** The notification action channel
   (`notif_action_tx` → `drain_notif_actions`) mutated the same prompt state
   from outside the keyboard router entirely, so no routing policy could
   describe it.

An optional, expiring, need-not-be-answered decision does not fit a modal, and
a modal is the only shape the keyboard router offers for a blocking question.
Deleting the TUI path removes the mismatch rather than encoding it.

## What stayed

`App.next_up_item` **remains**. It is not prompt state: `PlayerEvent::NextUpPlay`
(raised when the user clicks mpv's button) takes it to resolve the queue index
for `PlayerCommand::JumpTo`. Every existing clear site still applies.

`App.skip_intro_end_ticks` was deleted. The Lua script performs the seek itself
(`mp.set_property_number('time-pos', secs)`), so the field had no reader after
the prompt was removed — only writes and clears.

`always_skip_intro` is unaffected: `PlayerEvent::IntroStarted` still auto-seeks
when it is set, and never shows a prompt in that case.

## Known gap

With a remote or packaged daemon, mpv renders on the **daemon's** display. The
button is then wherever that display is, which may not be in front of the
person driving the TUI. The desktop notification used to cover that case
locally.

This is accepted for now. If it needs covering, the notification path
(`notify_with_actions` + the `skip_intro:*` / `next_up:*` arms of
`drain_notif_actions`) is the right thing to restore — not the status-bar
prompt.

## Re-adding

If a TUI affordance comes back, it must satisfy all three:

- It is not written to `App.status`. That field is toasts, with a TTL.
- It does not take focus. An optional, expiring prompt that swallows keys is
  the bug that was removed. A non-focusable indicator plus a chord owned by the
  Keyboard Router (ADR 0023) is the shape that fits.
- Its visibility condition and its focus condition are the same condition.
