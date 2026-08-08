## Context

`flash_status` (2s TTL) and `flash_status_high` (5s TTL) in `notify_actions.rs` both ring the terminal bell, attempt `notify-send`, and render with the same red `TOAST_BG`. `App.status` + `App.status_expires` hold the current toast; prompts reuse the same field with `status_expires = None`. ~103 call sites across 27 files. See proposal.md for motivation.

## Goals / Non-Goals

**Goals:**

- Four-class severity; colors and notification gating are the user-visible payoff.
- Migration preserves every current TTL with zero per-site decisions.
- A purely mechanical call-site pass; no logic changes.

**Non-Goals:**

- Copy overhaul, toast suppression, async-timing fixes, prompt changes (proposal → Out of Scope).
- The `notify_with_actions` prompt bell.

## Decisions

### Semantic variant names

`ToastSeverity { Neutral, Success, Warning, Error }` names the *event*, not the color; palette binding is a render detail. (An earlier draft of this change named variants by color — `Green`/`Yellow`/`Red` — coupling intent to presentation.)

### Duration derives from severity

`flash(msg, severity)`: Neutral/Success → 2s, Warning/Error → 5s. This is exactly today's `flash_status`/`_high` split, so every existing TTL is preserved automatically. Alternative considered: `flash(msg, severity, duration)` with per-site durations — rejected: ~103 extra decisions for no current need. Add an override later if a toast ever needs a non-standard TTL.

### Bell removed, not gated

`flash()` never rings the terminal bell. The bell has proven annoying and is likely to be removed entirely; gating it by severity would build logic for a dead-end feature. The `notify_with_actions` prompt bell is a separate surface and is unchanged.

### notify-send gated on severity

`severity != Neutral` attempts `notify-send` (when system notifications are enabled); on success the in-app row is hidden, as today. Neutral always renders in-app, silently. This is the noise-reduction payoff of classification.

### Prompts identified by expiry, not a sentinel severity

Prompts already set `status_expires = None`; the render path uses status-bar styling when there is no expiry and the severity color otherwise. `status_severity` is a plain `ToastSeverity` field — no `Option` overloading `None` to mean "prompt" (two overlapping signals for one fact).

### Migration mapping

One pass over every call site:

| Today | Becomes |
|---|---|
| `flash_status` — progress/info/lifecycle | `Neutral` |
| `flash_status` — requested action completed | `Success` |
| `flash_status_high` — failed but recovered via fallback | `Warning` |
| `flash_status_high` — failed, no recovery | `Error` |

Judgment guide: **Success** = it happened (saved, cleared, connected, renamed, enqueued). **Neutral** = in progress or FYI (requesting, scanning, loading, mode changes, "nothing playing"). **Warning** = failed but something sensible happened anyway (fell back to local playback, adopted local state). **Error** = failed and nothing happened (couldn't connect/load/save, empty selection, refusals like "Nothing to enqueue").

The `Can't mix libraries in a routed queue` call migrates like everything else (Error). Separate planned work may delete that message — a one-line conflict either way. Both wrappers are deleted at the end of the pass.

### "Playing on remote" copy rides the migration

Four call sites flash `"Playing on remote: {label}"` on the line *before* the session command is submitted (`actions.rs` ×2, `action.rs`, `queue_actions.rs`) — claiming a fact about an unconfirmed async operation, while the direct-remote path already says `"Requesting playback: {label}"`. Migration touches these lines anyway; adopt the truthful string and update the 3 assertions in `action_tests.rs`.

## Risks / Trade-offs

- Misclassification judgment calls during migration → mapping table above plus review pass; colors make mistakes immediately visible.
- Assertions on old strings or bell behavior → updated in the same pass (task 3.2).
- In-flight branches touching the same lines (route-conflict removal) → one-line conflicts.
