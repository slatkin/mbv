# Handoff — 5.3d *Album cursor prep* (blocked)

**Status:** Bookkeeping-only progress this session. The top-level `5.3c`
checkbox is marked `[x]` in `tasks.md` (all seven named child units were
already verified `[x]`). The *Album cursor prep* child (`5.3d` first bullet)
is **not** implemented; its `[ ]` is left unchanged. The unit is blocked by an
unresolved narrow-mode ownership question that the task scope forbids me from
deciding unilaterally.

**Prior verified commits preserved:** `b9d3cd68` (Context menu),
`95b7936` (Settings popups). Working tree was clean at start; only
`tasks.md` is modified (5.3c checkbox).

---

## 1. What the unit asks for

From `tasks.md` §5.3d (and the spawning prompt):

1. Settle the narrow-mode question — mount `MusicWorkspaceComponent` in
   narrow, **or** prove the narrow path cannot reach a `Some`.
2. Move `render/screens/album_cursor.rs`'s three `pub(in crate::app)` entry
   points (`move_music_group_display_cursor`, `jump_music_group_display_cursor`,
   `page_grouped_album_cursor`) **into** `MusicWorkspaceComponent`.
3. Behaviour-neutral, compiles standalone, deletes no field, and does **not**
   perform the separate Album-track-focus teardown.

## 2. Narrow-mode evidence (call paths traced)

**The component is wide-only.** `shell_music_workspace.rs:16` gates mount on
`!self.app.layout.main.is_wide_music_active()`. `is_wide_music_active()`
(`layout.rs:184`) is true only when `wide_music_right_area` has nonzero
width/height, which only the wide Music renderer sets. So in narrow mode
(`wide_music_right_area` is the default zero rect) **the component is never
mounted** — the narrow path cannot reach a `Some` (a mounted
`MusicWorkspaceComponent`). Confirmed by `input_music_track_focus_tests.rs:25`
(`assert!(!app.layout.main.is_wide_music_active())`) and
`tests_music_characterization.rs:112`.

**But the three functions are still invoked in narrow** through `App` key
handlers that do **not** gate on wide:

- `move_music_group_display_cursor` ← `lib_cursor_actions.rs:194`
  (`move_lib_cursor_inner`). No wide guard; only
  `is_viewing_album_folders`. Reached in narrow.
- `jump_music_group_display_cursor` ← `lib_cursor_actions.rs:262`
  (`jump_lib_cursor`). No wide guard; only `is_music_group_view`. Reached in
  narrow.
- `page_grouped_album_cursor` ← `input_lib_keys.rs:166`/`172`
  (PageUp/PageDown). No wide guard; gates on `effective_panel_focus`,
  `album_track_focus.is_none()`, `is_viewing_album_folders`. Reached in narrow.

Concrete proof: `input_music_track_focus_tests.rs:229`
`up_down_at_group_boundary_moves_between_groups_skipping_headers` builds a
music-group app via `make_music_album_list_app` (default zero-area layout →
narrow) and asserts `handle_key(Down)`/`handle_key(Up)` move the cursor through
the grouped-album plan — i.e. `move_music_group_display_cursor` runs and takes
effect in narrow today.

**Conclusion:** "Narrow cannot reach a `Some`" is TRUE for the *component*, but
FALSE for the *functions*: narrow still drives them via `App::handle_key`.
Mounting the component in narrow is excluded by the task scope ("do not expand
into mounting in narrow"). Therefore a literal "move into the component as the
sole implementation" breaks narrow unless the `App` side keeps an equivalent
path — which is duplication / not a single-source move, and is the exact
wide/narrow entanglement that `5.3a` already flagged as the reason these three
functions were *folded into 5.3d* rather than teardown-claimed earlier.

## 3. Why it is blocked (not lazily punted)

The three functions read/write `App` authority directly:

- `self.libs[lib_idx].nav_stack.last(_mut)().cursor`
- `self.libs[lib_idx].album_track_focus = None` (3 sites: lines 98, 147, 206;
  plus the gate at 166)
- `self.layout.main.left_area.height`, `self.current_library_columns(lib_idx)`
- `self.tab.emby_library_index()`, `self.effective_panel_focus()`,
  `self.is_viewing_album_folders`, `self.is_music_group_view`
- `self.group_album_info`, `self.build_grouped_album_display_plan`,
  `self.list_image_fetches_allowed`, `self.mark_library_navigation`,
  `self.maybe_fetch_next_page`, `self.last_nav_at`

`MusicWorkspaceComponent` (`components/music_workspace.rs`) holds **none** of
this. Under the mirror-first contract (design D14) the component owns a
*local mirror* (`album_cursor`, `album_scroll`, `track_cursor`) hydrated from
`MusicWideRenderCtx` via `set_content`; it forwards keys to `App` and emits
`Msg::Legacy`. It does **not** take `&mut App` — the interactive-component
boundary (AGENTS.md + `rules/interactive-component-boundary/*.yml`) rejects
`App` as a type inside a component. No existing component passes `&mut App` to
a method, so there is no precedent and the boundary rule would have to be
relaxed (out of scope for this unit).

The realistic single-source moves both require a decision the scope withholds:

- **Path A — mount in narrow.** Drop the `is_wide_music_active()` gate in
  `music_workspace_component_id`. Then the component exists in both modes and
  can own the methods. This is explicitly forbidden ("do not expand into
  mounting in narrow").
- **Path B — keep `App` authoritative for narrow, relocate logic into the
  component module, no narrow mount.** Move the three bodies + helpers into
  `music_workspace.rs` as a cohesive unit (free `fn`s taking `&mut App`, or
  `impl MusicWorkspaceComponent` methods taking `&mut App` transiently), and
  keep `App`'s three `pub(in crate::app)` methods as delegators. This satisfies
  "move into `MusicWorkspaceComponent`" only loosely (module, not struct-owning
  state), still couples `App` into the component's code, and leaves `App`
  authoritative — i.e. it is a relocation, not the ownership transfer the
  eventual teardown needs. It also does not make the component the cursor
  owner (that needs deleting `nav_stack.cursor`/the mirror, which this unit
  forbids).

Neither path is safely completable within the stated constraints without the
parent resolving the narrow-mode ownership decision. That decision is the
explicit first half of this child task; it was left open ("mount … or prove …")
and scope removes the "mount" option, so the only remaining resolution is to
confirm the narrow `App` fallback is the intended permanent shape for stage 1.

## 4. Exact edits required once the decision is made

Assumes Path B (recommended; no narrow mount):

1. In `src/app/components/music_workspace.rs`, add the three functions +
   their private helpers (`grouped_album_navigation_targets`,
   `catalog_album_navigation_targets`, `music_group_navigation`,
   `grouped_cursor_target`), relocated verbatim from
   `src/app/render/screens/album_cursor.rs`. They take `&mut App` (transient;
   not stored in the struct).
2. In `album_cursor.rs`, replace each `impl App` body with a one-line
   delegator to the relocated function. Field reads/writes (`album_track_focus
   = None`, `nav_stack.cursor`) stay exactly as-is — **no field deleted**.
3. Keep `album_cursor.rs` mounted in `render/screens/mod.rs` (the file stays);
   only its bodies move. Or, if relocating the whole file, re-export from
   `render/mod.rs` so `pub(in crate::app)` visibility is preserved.
4. `MusicWorkspaceComponent`'s own `album_cursor`/`track_cursor` mirrors are
   untouched; this unit does **not** change wide/narrow render or cursor
   behaviour.

Verification (per task):

- `rtk cargo check -p mbv`
- `rtk cargo nextest run -p mbv` (no narrow-specific selector exists for this
  surface; the grouped-album cursor tests in `input_music_track_*.rs` and
  `input_library_scope_routing_tests.rs` cover both modes).
- `rtk cargo fmt --all -- --check`
- `rtk make check-code-file-lines`

## 5. What was NOT touched (preserved)

- All 5.3c child units and their commits.
- `LibraryTab.album_track_focus` — not deleted (separate *Album track focus*
  unit). The three `= None` resets and the line-166 gate stay in place.
- No `input_mouse*.rs`, no global hit-testing, no `CONTEXT_STACK`/`AppLayout`/
  `LegacyInput` removal, no other 5.3d unit.
- Only `tasks.md` line 201 (`5.3c` checkbox) was edited this session.

## 6. Recommended next step for the parent

Confirm Path B is acceptable (relocate the function bodies into the
`music_workspace` module with `App` delegators; keep `App` authoritative for
narrow; do not mount in narrow). If yes, the edits in §4 are mechanical and
behaviour-preserving — hand back to an agent with this handoff. If the intent
was instead for the component to become the sole owner in stage 1, that
requires narrow mounting (Path A) and is out of scope for this unit as
written.
