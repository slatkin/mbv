# Row 9.2 — Live Review Record

- Date: 2026-09-10
- Reviewer: user (live terminal), all checklist areas; orchestrated via focused review of the fix commit.
- Scope: live Wide, Normal, Grid, and provider-workspace surfaces — one-painter ownership, preserved visuals, current-frame hit geometry, selector chrome, responsive viewport offset.

## Result: PASS (after one defect found and fixed)

### Defect found and fixed
- **Queue mini view exploded at the smallest breakpoint** (pill bars misplaced, broken geometry, placeholder images — library content painted over the queue frame).
- Root cause: PRE-EXISTING leak (reproduced at `22105ecb~1`, pre-queue-migration): `render_emby_browser_component` and narrow `render_music_workspace_component` paint at `LayoutMain::left_area`, which in queue-only mode is the queue's own column; `render_library` is skipped so no republication occurs.
- Fix: `2c89bc01` — `Model::library_panel_visible()` gate (keyed on `effective_panel_mode()`, same signal as the base frame) on both paint sites + mouse eligibility; narrow grouped-Music identical leak fixed; mini-breakpoint buffer characterization test added (verified FAIL pre-fix); narrow-browser test setup retargeted off the leak.
- User re-verified live: mini view Queue and narrow Music render correctly.

### Live checks confirmed by the user
- Browser generic two-column Grid navigation/cells/buckets; selected-cell treatment and year metadata slot acceptable.
- Movies/homevideos Wide↔Inline responsive offset; Home sections + hero wheel; PageUp/PageDown 5-row stride.
- Feeds group/watched chrome, Wide/Inline; Queue panel modes fixed-row, scope pills, drag reorder.
- Grouped Music albums/track-pane focus (highlight preserved on focus clear); TV series/episode panes, Normal routing through Browser.
- ABS Podcast episode pane clicks; Book chapter focus + seek; chapter duration slot always projected (accepted visible change).
- One painter per surface at every breakpoint; no ghosting/double-drawn rows.

### Verification
- Focused correction review of `2c89bc01`: PASS (gate correctness, consistency, test retarget).
- Gates at fix commit: nextest -p mbv 1529 passed; workspace 2120; clippy clean; ast-grep test 12/12 + scan clean; inventory 11 flows pass.
