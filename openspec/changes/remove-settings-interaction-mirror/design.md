## Context

See proposal.md for motivation. This section records the exact current shape
of the round trip, verified by reading the code rather than assumed from the
issue, since #615 itself says to treat its file:line pins as ground truth but
notes an ordering dependency ("coordinate with #613... if the legacy painter is
still live") that needs checking, not taking on faith.

**The closed loop, as it exists today:**

1. `SettingsComponent` (`src/app/components/settings.rs`) owns `cursor`,
   `services_cursor`, `scroll` as private fields, updated from its own
   `service_key` / `edit_form` / navigation handling.
2. On activation the component emits `Msg::Service(ServiceRequest::
   ActivateService(self.services_cursor))` or
   `Msg::Settings(SettingsIntent::Activate(self.cursor))` — the component's own
   value, already known locally.
3. `Model::handle_service_request` (`shell_settings.rs:168-172`) does
   `self.app.services_cursor = cursor;` then calls
   `self.app.activate_service_entry()`, which re-reads `self.services_cursor`
   at `services_settings.rs:180` to index `SERVICE_ENTRIES`.
   `Model::handle_settings_intent` (`shell_settings.rs:236-239`) does the same
   for `settings_cursor` → `App::handle_settings_activate()` →
   `settings_cursor_to_key(self.settings_cursor)` at
   `render/components/settings.rs:40`.
4. `SettingsIntent::Back` (`shell_settings.rs:217-219`) additionally resets
   `self.app.services_cursor = 0` directly, with no component involvement.
5. Every tick, `settings_snapshot()` (`shell_settings.rs:121-123`) reads those
   same three `App` fields back out and pushes them into `SettingsSnapshot`,
   which the component clamps into its own fields in `set_content()` — this
   half of the loop is the sanctioned content push (D17: "Direct pushes of
   validated shell-owned content ... are not forbidden mirrors") and stays.

**What blocks deleting the three `App` fields today — verified, not assumed:**

- `App::activate_service_entry()` (`services_settings.rs:180`) and
  `App::handle_settings_activate()` (`render/components/settings.rs:40`) read
  `self.services_cursor` / `self.settings_cursor` directly — these are the
  handlers this change reshapes to take the value as an argument.
- `App::render_settings_panel` / `App::render_services_panel`
  (`render/components/settings.rs:166,284`) — the **legacy** painter (an `impl
  App` block in this file, distinct from the new `SettingsRenderModel`/
  `render_settings_content` painter used by the live component) — read
  `self.settings_cursor` at lines 40, 203; `self.settings_scroll` at lines
  268-280; `self.services_cursor` at line 310. Repo-wide grep for
  `render_settings_panel` and `render_services_panel` finds exactly one
  caller: `src/app/render/tests_settings.rs:16`. That module is wired in via
  `#[path = "tests_settings.rs"]` at `src/app/render/mod.rs:205`, so it is a
  live-but-test-only module, not dead code excluded from compilation — it must
  be deleted alongside the legacy painter, not left to bit-rot.
  `docs/architecture/interactive-surface-ledger.md:75` already marks this
  surface `migrated (2026-08-27)` with an "App-free render seam" claim; that
  claim is true for the *live* render path (the component's own
  `SettingsRenderModel`, fed from the component's own fields — see next point)
  but not yet true for these three interaction-state fields, which is exactly
  the gap #615 closes.
- `render/components/settings_component.rs:23,72` defines and reads
  `SettingsRenderModel.services_cursor`, but that struct is constructed at
  `components/settings.rs:356-364` from `self.cursor` / `self.services_cursor`
  / `self.scroll` — the **component's own** fields, not `App`. This is not a
  blocker; no change needed here.
- `services_settings.rs:51` (`open_services_settings`) clamps
  `self.services_cursor` when the Services destination opens. This is a
  same-value write within `App` and is unaffected — it stops mattering once
  the field is deleted, since the component initializes its own cursor to 0
  and clamps from the pushed `services.len()` in `set_content()`.

No other production reader of `settings_cursor`, `services_cursor`, or
`settings_scroll` exists outside `shell_settings.rs`,
`components/settings.rs`, and `app_struct.rs`.

## Goals / Non-Goals

**Goals:**
- Close the Settings/Services round trip: a component-owned cursor drives a
  shell-owned effect by being passed as a value, never by being written to and
  read back from `App`.
- Delete `App::settings_cursor`, `App::services_cursor`,
  `App::settings_scroll` once nothing reads them.
- Delete the now-fully-unblocked legacy painter
  (`render_settings_panel`/`render_services_panel`) and its sole test caller.
- Prove the pattern other #611 slices (#616 TV, #617 Queue, #618 Emby browser)
  will repeat: pass the resolved value, don't mirror the field.

**Non-Goals:**
- Any change to the sanctioned shell→component content push (service rows,
  setting values, setup draft) — those stay exactly as they are.
- Answering #617's follow-position-vs-user-cursor ownership question or #618's
  per-frame scroll write-back — those are separate, harder slices with their
  own scout handoffs, out of scope here.
- Any mouse behavior change (mouse remains accepted-broken for the alpha per
  D16; this surface's mouse hit geometry, if any, is untouched).
- Any daemon/ctrl/Service/playback behavior change.

## Decisions

**Reshape the two App-side handlers to take a resolved value, not re-read
`self`.** `App::activate_service_entry()` becomes
`activate_service_entry(&mut self, entry: ServiceEntry)` (or keeps a cursor
parameter and does the `SERVICE_ENTRIES.get()` lookup itself — implementer's
call, either removes the `self.services_cursor` read). `App::
handle_settings_activate()` becomes `handle_settings_activate(&mut self, key:
SettingKey)`, with `settings_cursor_to_key` called once at the call site in
`shell_settings.rs` instead of inside the handler. This is the direct
application of D14/D17: the component already knows its own value at the
point it emits the request; the fix is to carry that value through the call
instead of parking it in `App` first. This mirrors the existing exemplar
(`shell_home.rs:251`, cited by #611) — push resolved values forward, never
park component state in `App` for a handler to re-read.

**Delete the legacy painter as part of this slice, not a follow-up.** #615's
"ordering note" says field deletion requires the legacy painter to stop
reading the fields first, and to coordinate with #613 (sole-painter work) if
that painter is still live. Investigation above shows it is reachable only
from `tests_settings.rs`, which is itself dead weight now that the component
owns rendering (the ledger already marks this surface `migrated`). Since no
production code depends on it, deleting it here is a genuinely
"smallest and most self-contained" unit rather than out-of-scope teardown: it
is the one remaining reader blocking the field deletion this issue exists to
do, and leaving it in place until #613 lands would mean this slice cannot
reach its own "no `App` field carries the settings cursor" done-when
criterion. If #613's own scope turns out to depend on this same painter for
some other surface's sole-painter accounting, that is a documentation-only
concern (a ledger note), not a code dependency — checked in tasks.md.

**`SettingsIntent::Back`'s services-cursor reset becomes component-local.**
Today `shell_settings.rs:217-219` resets `self.app.services_cursor = 0`
directly when Back leaves the Services destination. Once the App field is
gone, the reset has to happen in the component. The component already knows
when it is transitioning out of Services (it is the source of the `Back`
intent), so the component resets its own `services_cursor = 0` at the point it
emits `SettingsIntent::Back`, rather than relying on the snapshot's
`destination_changed` clamp path (that path only *clamps*, i.e.
`.min(len-1)`, it does not zero — confirmed by reading
`components/settings.rs`'s `set_content()`, which never assigns `0`). This is
a new, small piece of component-local logic, not a reuse of existing behavior.

**No new capability; delta the existing `interactive-component-framework`
capability**, matching `fix-router-overlay-textentry`'s precedent of amending
that spec's existing "component owns presentation authority" requirement
rather than introducing a Settings-specific capability. The behavior being
locked down (no App-field round trip for a component-owned value) is general,
not Settings-specific, even though this change only touches the one surface.

## Risks / Trade-offs

- **Deleting `tests_settings.rs` reduces render-path coverage for the legacy
  panel.** Mitigation: that panel is unreachable from production code (no
  caller other than the deleted test), so the coverage it provided was already
  testing dead code. The live render path (`SettingsRenderModel` /
  `render_settings_content`, fed from the component's own state) is unaffected
  and keeps its own tests.
- **Reshaping `activate_service_entry`/`handle_settings_activate` signatures
  touches every call site.** Both currently have exactly one call site each
  (`shell_settings.rs`), confirmed above, so this is a two-call-site signature
  change, not a fan-out.
- **Two `#611` fields (`ActivateService`, `SettingsIntent::Activate`) both
  route through this reshape** — if a future call site is added that still
  wants to write `App`'s old fields, nothing prevents regression today beyond
  code review, since the fields are simply gone; that is the intended
  enforcement (deletion, not a lint).

## Migration Plan

No runtime migration — this is a compile-time-enforced internal refactor with
no persisted state, wire format, or user-visible behavior change. Land as one
PR; no rollback concerns beyond a normal revert.
