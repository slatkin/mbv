# Fix Home Media-List Ownership Design

## Context

See `proposal.md` for motivation. Home already holds persistent
`WideMediaList<String>` and `InlineMediaBrowser<String>` fields
(`src/app/components/home.rs:55-56`) whose `ListCore` owns cursor/scroll,
and `project_active_section` projects only the active section into both
controls. The remaining violations are: (a) both controls move in lockstep
(`move_local_cursor`, `claim_row`, `select_section` drive both), so the
inactive control is a synchronized second authority rather than a parked
handoff target; (b) painting uses the legacy free-function seam with
explicit rects (`render_home_content` takes Inline as `&`, never persists
narrow scroll) instead of the retained-result `set_geometry` /
`set_paint_policy` / child `Component::view` / point-only `resolve_point`
seam Queue uses; (c) render returns `resolved_section`, which
`HomeComponent::view` writes back into `self.section`; (d) the shell keeps
a live `continue_cursor` mirror (`HomeContinueCursor` wheel arm writes it,
CW effects and the context menu read it back) plus a dead per-pill
`latest[i].3` cursor cache. The existing canonical-list contract instead
makes the active persistent control the live authority, with the shell
providing content and explicit re-anchor requests.

## Goals / Non-Goals

**Goals:**

- Give the active persistent Home control exclusive live cursor/scroll
  ownership; only the active control moves on input.
- Migrate both controls onto the retained-result seam and persist narrow
  scroll exactly like Wide.
- Remove the `resolved_section` writeback, the live `continue_cursor`
  mirror, the dead per-pill cache, and parent compatibility point
  resolution.
- Preserve a stable selected target over ordinary content refresh and
  hand off one `ViewportAnchor` only when the presentation changes.
- Make Home pointer resolution use the painting control's retained
  current-frame geometry.
- Retain Home authority for sections, pills, hero, images, effects,
  persistence, and typed intent translation.

**Non-Goals:**

- Change Queue, Grouped Music, any other campaign destination, or
  non-hero two-column catalogs.
- Redesign pills painting/positioning, hero content or image pixel paint,
  `pref_key`/`restore_section` persistence semantics, provider
  workspaces, CW business logic, or `merge_home_sections` ordering logic
  (only its dead cursor-preservation sub-behavior goes).
- Fork shared arrangements (`wide_hero.rs` etc.), `ListCore`, or painter
  primitives; add a router, a second mounted identity/subscription/focus,
  or pass `App`/Service/`Config`/`PlayerProxy` into components.

## Decisions

### D1. Only the active control moves; the inactive control is a parked handoff target

`move_local_cursor`, `claim_row`, and wheel handling drive the active
control only. The inactive control keeps its last content and is brought
current solely through the existing `view()` breakpoint handoff, which
takes one `ViewportAnchor` from the outgoing control and applies it to
the incoming one before flipping `self.wide`. Discrete section changes
keep parking at the first row with no per-section cache.

Alternative: keep lockstep and treat the inactive control as a warm
standby. Rejected because two live authorities permit drift and let the
shell or render path read position from whichever copy is convenient.

### D2. Rendering configures geometry and paint policy, then the control paints itself

Both Home render paths set the parent-established claim/row-flow
rectangles and the closed semantic paint policy on the control, then
invoke the control's `Component::view`. Wide keeps its existing
`set_scroll` persistence path; Inline moves from the `&` free function
(which never persists scroll) onto the retained-result adapter that
calls `finish_view`, so narrow scroll persists across frames. Parent row
maps and explicit-rect point resolution are removed; row hits resolve
through a point-only call against the painting control's retained
current-frame geometry. Parent pill geometry remains parent-owned
chrome.

Alternative: keep the free-function painters and add a narrow scroll
writeback beside them. Rejected because a second geometry authority can
drift from the current painted frame.

### D3. Section clamping lives in the component, not in render output

`render_home_content` no longer returns `resolved_section`, and
`HomeComponent::view` no longer writes render output back into
`self.section`. Clamping happens inside the component (at content
projection or before paint), and render receives a pre-clamped section.

Alternative: keep the writeback and treat it as a harmless clamp echo.
Rejected because any render-to-component state write is a second
authority over component-local state.

### D4. CW effects and the context menu resolve from the component's selected target

The `HomeContinueCursor` wheel arm no longer writes an App-wide mirror;
the wheel moves the active control's selection like any other list
input. `home_cw_item`, `home_flat_target` consumers, and the
context-menu Home arms resolve from the component's selected stable
target (passed as an explicit target in the typed effect, like every
other destination's effects) instead of reading `continue_cursor` back
from `Model::home_content`. The dead `latest[i].3` per-pill cursor
storage and the `merge_home_sections` cursor-preservation sub-behavior
are deleted; pill ordering and canonical splicing stay untouched.

Alternative: keep `continue_cursor` as a shell-resting copy updated from
the component. Rejected because a live-updated copy the shell reads back
for effects is exactly the mirror the contract forbids; resting state
that nobody reads is dead storage.

### D5. Home workspace authority remains intact

Section identity (`pref_key`/`restore_section`), pills, hero content and
image paint, provider workspaces, CW business logic, persistence, and
typed request translation remain owned by Home and the shell. This
repair changes only canonical list mechanics.

Alternative: migrate adjacent Home state while touching the component.
Rejected as unrelated scope expansion.

## Risks / Trade-offs

- [Wheel input previously wrote the mirror; removing it changes the CW effect source] → cover wheel-driven CW play/enqueue/context-menu through shell-tick tests resolving from the component selection.
- [Removing compatibility point resolution leaves a stale hit path] → cover mounted tick/subscription pointer handling at Wide and Normal/Narrow breakpoints, including hero-area clicks.
- [Constructor churn from deleting the `latest` tuple element] → update constructors mechanically; shared ordering logic is untouched.
- [Refresh regresses selection] → characterize stable-target preservation, clamp, and explicit re-anchor separately.

## Migration Plan

1. Characterize current visible output and control behavior at Wide and
   Normal/Narrow breakpoints.
2. Move Home live position and handoff to the active persistent
   control; remove lockstep, render reseeding/writeback, the shell
   mirror, the dead cache, and compatibility geometry.
3. Verify focused component, shell-tick pointer integration, and
   required project gates.
4. If needed, revert the bounded source change; no persisted data
   migration is involved.
