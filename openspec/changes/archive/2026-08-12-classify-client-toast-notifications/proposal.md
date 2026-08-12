## Why

Every TUI toast looks identical (muted red), rings the terminal bell, and attempts `notify-send` — whether it reports progress, success, or failure. The only severity mechanism is the `flash_status` (2s) vs `flash_status_high` (5s) split, so severity is conflated with TTL, errors don't stand out, and routine progress messages generate notification noise.

## What Changes

- Introduce `ToastSeverity { Neutral, Success, Warning, Error }`: progress/info, completed action, recovered-with-fallback, unrecovered failure.
- Replace `flash_status`/`flash_status_high` with `flash(msg, severity)`; display duration derives from severity (Neutral/Success 2s, Warning/Error 5s — preserves today's exact TTLs).
- Remove the terminal bell from the toast path entirely; gate desktop `notify-send` on `severity != Neutral`. Neutral toasts always render in-app.
- Color the toast row by severity (green/yellow/red palette tokens); Neutral toasts and prompts use standard status-bar styling.
- One-pass migration of every `flash_status`/`flash_status_high` call site to a severity; both wrappers deleted.
- Copy fix riding the migration: `"Playing on remote: …"` → `"Requesting playback: …"` at the 4 call sites that flash before the command is even sent (plus their 3 test assertions).

## Capabilities

### New Capabilities

- `toast-notification-semantics`: severity classification for TUI toasts — classes, display durations, notification side effects, and row colors.

### Modified Capabilities

(none)

## Out of Scope

Deferred to follow-up issues; none of these block classification:

- Toast copy overhaul (`Error: {e}` → "Couldn't …" phrasing across all messages)
- Toast suppression (internal-jargon toasts like "Remote tracking stopped", "No session connected", item-count flashes, background retries)
- Enqueue success toast firing before remote sync confirms (`actions.rs`)
- "Terminal too short for left visualizer" render-time flash (state mutation in the render path)
- Playlist fetch errors rendering as an empty list; search toast duplication with inline error
- Prompt copy (next-up, skip-intro) and the `notify_with_actions` prompt bell

## Impact

- `notify_actions.rs`: `ToastSeverity` + `flash(msg, severity)`; bell removed from the toast path; old wrappers deleted.
- `app_struct.rs` / `construct.rs`: new `status_severity: ToastSeverity` field.
- `palette.rs`: green/yellow toast tokens. `render/mod.rs`: severity-colored row background; prompts keep status-bar styling.
- ~27 files of mechanical call-site migrations per the design mapping rule.
- Existing test assertions updated to the new model (no new tests).
- Effectively reverts the toast bell from #196; the `notify_with_actions` prompt bell is unchanged.
