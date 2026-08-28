## Context

See proposal.md - Why. This is a D17-governed teardown slice (stage 4,
"remove interaction-state pins and obsolete `App` readers only after all
remaining consumers are re-homed" — `openspec/changes/migrate-tui-to-tuirealm/design.md`).
D17's parity rule applies: current observable behaviour, including current
effect targets, is authoritative. Where the component and the legacy/App
path would resolve a request differently, that is a blocking discovery
result to record, not license to pick the cleaner interpretation.

Three `App` methods are re-read after `mirror_tv_workspace_cursor` writes
`BrowseLevel.cursor`:

- `App::activate_selected_series(lib_idx)` → `selected_series_item(lib_idx)`
  → `library_list_render_ctx(lib_idx, false).cursor` → indexes `ctx.items`.
  This **does** depend on the mirrored cursor.
- `App::go_back(lib_idx)` (`src/app/actions_navigation.rs:217`) pops
  `nav_stack` unconditionally when `len() > 1` (subject to the synthetic-group
  root guard) and restores the *parent* level's cursor by looking up the
  popped level's own `parent_id` in the parent's `items` — it never reads the
  popped/current level's `.cursor` field itself. This does **not** depend on
  the mirrored cursor.
- `App::cycle_letter_pill(lib_idx, delta)` (`src/app/music_actions.rs:218`)
  reads `letter_filter` off the current level, not `.cursor`. It does **not**
  depend on the mirrored cursor.

This matches the Emby browser precedent in `shell_browser.rs:17-110`:
`BrowserBack` calls `self.app.go_back(lib_idx)` and
`BrowserCycleLetterPill` calls `self.app.cycle_letter_pill(lib_idx, delta)`
directly, with no cursor mirror ahead of either call — because neither method
needs one. Only `BrowserActivate`/`BrowserPlay`/etc. take an explicit `item`
argument, because activation is the one effect that actually consumes the
selected item.

## Goals / Non-Goals

**Goals:**
- Delete `mirror_tv_workspace_cursor` and both of its call sites.
- Leave zero TV request path that writes a component-owned cursor into `App`.
- Preserve current observable behavior of `TvMoveRows`, `TvMoveColumn`,
  `TvJumpCursor`, `TvActivate`, `TvBack`, `TvCycleLetterPill` exactly, per
  D17.

**Non-Goals:**
- Season/episode cursor ownership (`shell_tv_workspace.rs:44-45`) — already
  component-local, untouched.
- Mouse activation (`mouse_gestures.rs:166,234`) — accepted-broken for the
  alpha (D16); its cursor-resolving call to
  `activate_selected_series(lib_idx)` is left as-is.
- Redesigning `go_back` or `cycle_letter_pill` beyond removing the now-dead
  mirror call ahead of them.
- The ABS podcast `episode_selection` App fallback
  (`shell_audiobookshelf_podcast.rs:405-406`) — explicitly called out in #611
  as a trailing cleanup, not part of this slice.

## Decisions

**D1 — `TvActivate` becomes item-targeted; `go_back` and `cycle_letter_pill`
calls are left as bare `App` calls with no new item argument.**

Only `activate_selected_series` reads the mirrored cursor, so only its call
site needs a resolved-item payload. Add
`App::activate_selected_series_item(&mut self, item: &EmbyItem) -> bool` (or
equivalent name chosen at implementation time) that performs the same
`is_wide_tv_active()` branch (`enter_series_selection` /
`open_series_selection_modal`) as the existing method, minus the
`selected_series_item(lib_idx)` lookup — the item arrives already resolved
from `TvWorkspaceComponent::selected_item_id()` (mirroring the shape
`Browser*` request handlers use for `select_item`/`play_or_activate_lib_item`).
The existing `activate_selected_series(lib_idx)` stays, unchanged, for
`mouse_gestures.rs`.

`ShellRequest::TvActivate` gains `{ item: EmbyItem }`. The component resolves
the item at the same point it currently transitions `self.pane` on
`Key::Enter` (`tv_workspace.rs:246`), using the same lookup
`selected_item_id()` already performs, cloning the full `EmbyItem` rather
than re-deriving it later from an id.

Alternative considered: keep `TvActivate` bare and have the shell resolve the
item from the *component* (not `App`) at the `handle_tv_request` call site,
via the existing `tv_workspace_component_id()` + downcast pattern already
used by `tv_episode_activation_selection`. Rejected: it re-adds a
shell-reads-back-into-component step for a value the component already has
in hand at emission time, and diverges from the `Browser*` precedent, which
resolves and attaches the item at the component boundary before the `Msg`
crosses it.

**D2 — `go_back` and `cycle_letter_pill` get no new parameter.**

Per the Context analysis, neither reads the cursor the mirror was
maintaining. `go_back`'s parent-cursor restoration is shell-owned navigation
memory (keyed by `parent_id`, populated when the level was pushed, not by the
component's live selection) — a sanctioned push, not a mirror, exactly as
D17/#616 anticipates. Removing the mirror call ahead of these two is
therefore a no-op for their behavior. This must be confirmed with
characterization test coverage (see tasks.md) before deletion, per D17's
"prove the active production path first" rule — if a hidden dependency on
`.cursor` surfaces during implementation, stop and record it as a blocking
discovery result rather than silently reworking either method.

**D3 — `mirror_tv_workspace_cursor` and both call sites are deleted only
after D1 lands**, since `TvActivate`'s effect is the only remaining reader.

## Risks / Trade-offs

- [Risk] `selected_series_item(lib_idx)` may apply guards
  (`collection_type != "tvshows"`, `item.item_type != "Series"`) that the
  component-resolved item does not re-derive identically, causing a silent
  divergence between the item-targeted and cursor-resolved paths.
  → Mitigation: the item-targeted method keeps the `is_wide_tv_active()`
  branch dispatch identical; `TvWorkspaceComponent` only mounts for
  `collection_type == "tvshows"` (`tv_workspace_component_id()` guard in
  `shell_tv_workspace.rs:78-92`) and its list only contains `Series` items in
  the series pane, so the type guards are structurally satisfied at the
  emission point rather than needing to be re-checked. Confirm with a test
  asserting `TvActivate` on a non-series row is unreachable via the
  component's own row filtering.
- [Risk] Removing the mirror changes what `App.libs[lib_idx].nav_stack.last().cursor`
  reads as during the request, which other legacy/mouse code paths executing
  interleaved with a TV request could observe.
  → Mitigation: TV requests only fire while the wide TV workspace component is
  mounted and focused (`sync_tv_workspace`'s mount gate), during which the
  legacy TV key-handling path is not reachable (single keyboard resolution
  site, ADR 0023); mouse remains accepted-broken and out of scope (D16).
- [Risk] Characterization tests for D2 could be more invasive to write than
  the mirror removal itself, if `go_back`/`cycle_letter_pill` turn out to
  have TV-only branches not exercised by the existing Browser tests.
  → Mitigation: both methods are shared with the Browser path, which already
  calls them with no mirror ahead of the call; reuse that existing evidence
  and add TV-specific assertions only for the nav_stack cursor observation in
  `shell_tv_workspace.rs`'s own test module.

## Migration Plan

Single-slice, no runtime migration or data format change. Land as one
sequence: (1) add the item-targeted `activate_selected_series` entry point
and thread it through `TvActivate`'s payload and emission site; (2)
characterize `go_back`/`cycle_letter_pill` against the current mirrored
cursor, then remove the mirror calls ahead of them; (3) delete
`mirror_tv_workspace_cursor` and its now-unused declaration once all three
call sites are clear; (4) extend the two named tests
(`shell_tv_workspace.rs:200`, `:229`). Rollback is a plain revert — no
persisted state or protocol is touched.
