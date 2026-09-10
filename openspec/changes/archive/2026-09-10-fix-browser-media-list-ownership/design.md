# Fix Browser Media-List Ownership Design

## Context

See `proposal.md` for motivation. `BrowserComponent`
(`src/app/components/browser/mod.rs:52-108`) already holds persistent
`wide_list: WideMediaList<usize>` and
`inline_browser: InlineMediaBrowser<usize>` fields, and `BrowserContent`
(`src/app/components/browser/content.rs`) is position-free by
construction. The remaining violations are: (a) the component keeps
parent-level `cursor: usize, scroll: usize` fields that mirror the
controls — `navigation.rs` movement writes them back, `apply_position`
(`mod.rs:202-216`) and `set_content` (`mod.rs:~175-195`) seed them, and
the `view()` tail (`mod.rs:~690-710`) writes rendered scroll back into
`self.scroll`; (b) the shell keeps a live resting mirror —
`shell_browser.rs:130-153` writes `level.set_resting_cursor(index)` plus
`save_default_library_position` on every `BrowserCursorIndex` move, and
`shell_run.rs:60-76`, `push_emby_browser_content` (`shell_browser.rs:278-340`),
`render_emby_browser_component` (`shell_browser.rs:376-440`), and
`persist_emby_browser_scroll` (`shell_browser.rs:170-182`) read the
mirror back; (c) `resolve_row_target` / `resolve_left_cursor`
(`mod.rs:515-560`) resolve through parent-published compat maps
(`left_row_map` / `left_item_rows` written by `paint.rs` and
`render/components/list_narrow.rs:150-175`) with a `claim_list_point`
side effect that moves `self.cursor`; (d) the anchor path falls back to
parent fields — `view()` writes `self.cursor/self.scroll` when
`apply_active_viewport_anchor` fails, `set_content` resolves the
preserved anchor into `self.cursor`, and `viewport_anchor()`
(`mod.rs:351-364`) falls back to `context[cursor]`. Queue is the
concrete ownership precedent (umbrella D4); the Home repair (`23291256`)
is the point-resolution precedent (`resolve_current_point` /
`current_detail_rect`). The wheel path is live (control moves, emits
`BrowserCursorIndex`); no dead `BrowserScroll` variant exists at HEAD.
The two-column gate (`list_narrow.rs:hero_presentation`,
`navigation.rs:columns()`, `render/components/widgets.rs:565-601`) is
clean isolation that must be preserved (row 4.2).

## Goals / Non-Goals

**Goals:**

- Give the active persistent Browser control exclusive live
  cursor/scroll ownership for Movies and the Emby homevideos feed view
  at Wide and Normal/Narrow.
- Reduce the shell projection to resting/restore state written from
  component-resolved values, with explicit re-anchor as the only seed.
- Resolve pointer hits from the painting control's retained
  current-frame geometry; stop publishing compat maps on canonical
  paints.
- Transfer one `ViewportAnchor` control-to-control only at responsive
  handoff; ordinary refresh preserves/clamps local state.
- Prove non-hero two-column catalog isolation with test evidence.

**Non-Goals:**

- Change provider behavior per screen (sort/filter/pagination/fetch),
  workspaces, images/posters, playback effects, persistence schema,
  Inline Search internals, TV/Music/Feeds/ABS destinations, or non-hero
  two-column visuals.
- Fork shared arrangements (`wide_hero_presentation`,
  `pill_bar_areas`, `wide_library_panes`), `ListCore`/painter
  primitives, or the compat shims other destinations still use.
- Add a router, a second mounted identity/subscription/focus, or pass
  `App`/Service/`Config`/`PlayerProxy` into components.

## Decisions

### D1. Controls own live Browser position; the shell retains only resting/restore state

`wide_list` and `inline_browser` remain persistent component fields.
The parent `cursor`/`scroll` fields are deleted along with every
write-back: `navigation.rs` movement stops echoing position,
`apply_position`/`set_content` stop seeding parent fields, and the
`view()` tail stops writing rendered scroll back. Content pushes call
the controls' stable-target-preserving content APIs
(`ListCore::set_content` retention is not forked). The shell's
`BrowserCursorIndex`/Row arms stop writing resting cursor/scroll on
movement; resting state is written only from component-resolved values
at discrete navigation points (drill-in, `BrowserBack`, explicit
re-anchor) per the AGENTS.md resolved-value rule, and extras/poster
prefetch seed from the control's selected target. Identity-gated push
seeds (`note_browse_identity` + `apply_position`) become explicit
re-anchor requests.

Alternative: retain the resting mirror as a synchronized live copy.
Rejected because a live-updated copy the shell reads back for render
and effects is exactly the mirror the contract forbids; it permits
render-time adoption and App recompute of control-owned movement.

### D2. Pointer resolution uses retained control geometry only

`resolve_row_target` switches to point-only
`resolve_current_point` / `current_selected_target` plus
`current_detail_rect` against the painting control's retained
current-frame geometry, following the Home repair (`home.rs:564-575`).
`resolve_left_cursor`'s compat-map reads are removed for canonical
paths. Canonical paints (`paint.rs` wide path,
`list_narrow.rs` hero/inline path) stop publishing `left_row_map` /
`left_item_rows` / `left_sorted_indices`; the legacy non-hero branch
and the shims other destinations still use stay intact.
`claim_list_point` no longer moves parent state as a side effect.

Alternative: keep the compat maps for the non-hero grid fallback.
Rejected because a second geometry authority can drift from the
current painted frame; the grid keeps its own screen-owned geometry
under D5 instead.

### D3. Responsive transfer uses the shared anchor protocol, control-to-control only

A Wide/Narrow transition captures one `ViewportAnchor<String>` from
the outgoing control (`selected_row_offset`) and applies it to the
incoming persistent control before flipping, reusing the existing
`view()` flip block and `apply_viewport_anchor` seam. All parent-field
fallbacks are dropped: no `self.cursor/self.scroll` write when the
anchor apply fails, no preserved-anchor resolution into `self.cursor`,
no `context[cursor]` fallback in `viewport_anchor()`. Ordinary refresh
never adopts shell position; it preserves/clamps local control state.

Alternative: reconstruct the incoming viewport from parent
cursor/scroll arithmetic. Rejected because that creates a
destination-specific second viewport authority.

### D4. Wheel/click follows the Home repair pattern

The wheel arm keeps moving the control (the current live path in
`mod.rs:449-466`, throttled in `components/mouse/gesture.rs`) and
emitting the component-resolved index; the shell keeps only focus plus
nav-effect side calls and stops writing resting cursor/scroll on the
move. Click arms in `mouse_gestures.rs` keep focus and persistence
side calls only. No `BrowserScroll` variant work is needed — none
exists at HEAD.

Alternative: delete or stub the wheel path as dead input. Rejected
because the path is live and matches Home's kept-and-pinned wheel
(`home.rs:489-494`); the defect is the shell write-back, not the
input.

### D5. Non-hero two-column catalogs stay isolated and provably untouched

The isolation gates — `hero_presentation` in `list_narrow.rs`,
`columns()` in `navigation.rs`, the `widgets.rs:565-601` EmbyLibrary
arm that reserves `left_area` and returns — are preserved. Canonical
controls neither read nor overwrite grid state: non-hero Generic never
routes through either control, and canonical paths never read
`left_item_rows` from the grid. Tests record this isolation as
evidence (row 4.2): two-column Generic paints produce zero
canonical-control output and leave canonical state untouched.

Alternative: route the non-hero grid through the canonical controls
for uniformity. Rejected because the campaign explicitly preserves
non-hero two-column catalogs; conversion is out of scope.

### D6. Browser workspace authority remains intact

Provider content, workspaces, pills, images, effects, persistence, and
typed request translation remain owned by the Browser destination and
the shell. This repair changes only canonical list mechanics (position
authority, push/anchor discipline, hit geometry, isolation evidence).

Alternative: migrate adjacent Browser state while touching the
component. Rejected as unrelated scope expansion.

## Risks / Trade-offs

- [Shell extras/poster prefetch currently read `browser.cursor()`; re-sourcing from the control target changes the seed] → cover extras and prefetch through shell-tick tests asserting the painted target, not an App recompute.
- [Removing compat-map publishes leaves stale hit paths on canonical surfaces] → cover mounted tick/subscription pointer handling at Wide and Normal/Narrow, including blank/header no-op hits.
- [Anchor offset differs across structural rows] → exercise Wide↔Narrow round-trips preserving target plus offset in component and tick tests.
- [Two-column Generic shares the legacy painter entry] → characterize grid isolation separately so canonical map removal cannot regress the grid.

## Migration Plan

1. Characterize current visible output and control behavior at Wide
   and Normal/Narrow, including two-column isolation evidence.
2. Move Browser live position and handoff to the persistent controls;
   remove the parent mirror, shell live writes, compat-map
   publish/resolve on canonical paths, and anchor fallbacks.
3. Verify focused component, shell-tick pointer integration at both
   breakpoints, one-painter ownership, and required project gates.
4. If needed, revert the bounded source change; no persisted data
   migration is involved.
