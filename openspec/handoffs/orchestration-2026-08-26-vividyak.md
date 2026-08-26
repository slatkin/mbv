# Orchestrator Handoff — 2026-08-26 (VividYak session)

## Current State
- **Branch:** `feat/migrate-tui-to-tuirealm`
- **HEAD:** `fc85975b`
- **Progress:** 74/89 tasks complete (15 remaining)

## What Was Done This Session

### 5.3d.15 — Emby browser mount/content split
- **M1** (`8929248c`): Extracted `mount_emby_browser()` from `sync_emby_browser()`
- **M2** (`24e645b9`): Split content projection into event-driven `push_emby_browser_content()` at 12 writer sites; moved `set_wide_movies` to per-draw adapter in `render_emby_browser_component` (D18 step 1)

### 5.3d.16 — Emby browser raw-key fallthrough + mirror-pinning removal
- **Initial** (`6fa217fb`): Claimed all raw-key/mouse fallthrough (component consumes all input via `NoOp`), removed `initialized`/`last_mirrored_cursor`/`last_mirrored_scroll` fields, made component cursor authoritative
- **P1 fix** (`4e1e6f67`): Restored external cursor jump parity — reviewer found that removing mirror-pinning broke tab switch/go_back/inline search. Fixed by restoring cursor/scroll sync from context in `set_content`

### 5.3d.17 — Emby generic/Movies/home-video legacy underpaint removal
- **17a** (`578ecff0`): Component now handles wide Movies/home-video layout itself (hero card on left, pills on right, list rows in right pane). Shell passes full `movies_wide_area` when wide. Component exports `image_paint` for shell to paint via `App::paint_home_image`
- **17a P1 fix** (`e94a9fb5`): Reset `image_paint = None` in narrow branch to prevent stale hero images after wide→narrow resize
- **17b** (`afcaf659`): Removed legacy wide Movies branch from `render_list`, deleted `render_wide_movies_with_ctx` and `selected_wide_movie` (movies_wide.rs), deleted `movies_wide_tests.rs`. Preserved scroll write-back and geometry population in component path
- **17b P2 fix** (`fc85975b`): Fixed stale doc comment referencing deleted `movies_wide.rs`

## Scout Reports Generated

All scouts completed with `opencode-go/hy3` and saved to `openspec/handoffs/`:

- **5.3d.13** → `scout-abs-book.md` (16.6 KB)
  - ABS book typed-input surface, interaction readers, legacy render, mount/sync adapter
  - Key finding: book browser is half-converted (component exists, legacy renderer still paints)
  - Identified bounded units: A (delete legacy renderer), B (component owns interaction), C (typed Msg/ShellRequest)

- **5.3d.18** → `scout-tv-workspace.md` (6.9 KB)
  - TV workspace refinement with exact typed keyboard surface, writer seams, geometry contracts
  - Key finding: TV is one wave behind Emby (raw-key forward, mirror-pin, per-frame sync, legacy underpaint)
  - Identified bounded units: T1 (typed keyboard), T2 (drop mirror-pin), T3 (push at writers), T4 (underpaint removal), T5 (teardown), T6 (episode play/enqueue gap)

- **5.3d.19** → `scout-music-workspace-preliminary.md` (4.6 KB)
  - Music workspace completion with exact raw-key surface, projection writers, geometry, underpaint
  - Key finding: **BLOCKER** — geometry chicken-and-egg. Component view depends on `wide_music_area` set by legacy branch. Must compute geometry before component view (U2) before deleting legacy branch (U3)
  - Identified bounded units: U1 (mount/idempotent mirror), U2 (geometry pre-pass, BLOCKER), U3 (delete legacy branch), U4 (relocate fetch_album_tracks trigger), U5 (framework teardown)

- **5.3d.20** → `scout-inline-search.md` (3.2 KB)
  - Inline search mirror and raw endpoint
  - Key finding: inline search is already `migrated` — per-frame sync was deleted earlier. What remains is residual shell scaffolding and group-5 App teardown
  - Identified smallest units: U1 (drop inline_search_id field), U2 (merge redundant re-pushes), U3 (drop apply_inline_search_items), U4 (drop recursive pool branch), U5 (re-home `/` trigger), U6 (fix mouse left_area quirk)

## Remaining Tasks

### Podcast Teardown (5.3d.8–5.3d.11)
- **5.3d.8**: Complete podcast downstream-reader/cover-fetch handoff
- **5.3d.9**: Move podcast cover fetching to smallest shell/Model bridge
- **5.3d.10**: Delete podcast legacy underpaint after cover/layout work
- **5.3d.11**: Re-home remaining podcast interaction-field readers, delete obsolete mount/sync adapter

### ABS Book (5.3d.13)
- Scout report done. Bounded units identified in `scout-abs-book.md`. Next: implement Unit A (delete legacy renderer + gate legacy dispatch)

### TV Workspace (5.3d.18)
- Scout report done. Bounded units identified in `scout-tv-workspace.md`. Next: implement T1 (typed keyboard surface)

### Music Workspace (5.3d.19)
- Scout report done. Bounded units identified in `scout-music-workspace-preliminary.md`. Next: implement U1 (mount/idempotent mirror), then U2 (geometry pre-pass, BLOCKER for U3)

### Inline Search (5.3d.20)
- Scout report done. Already migrated. Bounded units identified in `scout-inline-search.md`. Next: implement U1 (drop inline_search_id field)

### Final Framework Teardown (5.3d.21–5.3d.24)
- **5.3d.21**: Re-inventory remaining CONTEXT_STACK, Msg::Legacy, LegacyTerminalEvent, LegacyInput, terminal reconstruction, sync_* interaction endpoints
- **5.3d.22**: Delete now-unreferenced per-surface CONTEXT_STACK handlers
- **5.3d.23**: Delete LegacyInput, Msg::Legacy, LegacyTerminalEvent, terminal reconstruction adapters
- **5.3d.24**: Verify no component-local interaction state is mirrored through App, no legacy renderer paints beneath migrated surface, no global mouse router/hit map remains

## Key Patterns Established

### Writer/Reviewer Cadence
- Use `opencode-go/hy3` for both workers and reviewers
- Route every writer commit to a reviewer before reporting done
- Reviewer checks for P1 regressions (external cursor jumps, stale state, geometry handoffs)
- Apply reviewer findings immediately with follow-up commits

### Component Conversion Pattern (Emby template)
1. **M1**: Extract mount lifecycle from per-frame sync
2. **M2**: Split content projection into event-driven push at writer sites
3. **I1**: Claim raw-key/mouse fallthrough (component consumes all input)
4. **I2**: Remove cursor/scroll mirror-pinning (component cursor authoritative)
5. **17a**: Component handles wide layout itself (if applicable)
6. **17b**: Remove legacy underpaint, preserve scroll write-back and geometry population

### Task Splitting
- Large tasks (like 5.3d.17) should be split into smaller units (17a, 17b) to avoid writer timeouts
- Each unit should be independently verifiable with cargo check/nextest/clippy/ast-grep

### Scout Reports
- Scouts are read-only and produce handoff notes in `openspec/handoffs/`
- Scouts identify bounded implementation units with exact symbol-level data
- Scouts can run in parallel with writers (different scopes)

## Verification Gates (per-task policy)
- `rtk cargo check -p mbv` — 0 errors
- `rtk cargo nextest run -p mbv <surface>` — all pass
- `rtk cargo nextest run -p mbv` — all pass (full suite)
- `rtk cargo clippy --workspace --all-targets` — 0 errors, no new warnings in touched files
- `rtk ast-grep scan` — no new findings in touched files

## Notes for Next Orchestrator

1. **Podcast teardown (5.3d.8–5.3d.11)** is the next cluster. These are sequential dependencies. Start with 5.3d.8 (scout downstream-reader/cover-fetch handoff).

2. **ABS book (5.3d.13)** has a scout report ready. The bounded units are identified. Start with Unit A (delete legacy renderer + gate legacy dispatch).

3. **TV workspace (5.3d.18)** has a scout report ready. Start with T1 (typed keyboard surface).

4. **Music workspace (5.3d.19)** has a BLOCKER identified (geometry chicken-and-egg). Must implement U2 (geometry pre-pass) before U3 (delete legacy branch).

5. **Inline search (5.3d.20)** is already migrated. The bounded units are residual shell scaffolding cleanup.

6. **Final framework teardown (5.3d.21–5.3d.24)** should only start after all surface rows (podcast, ABS book, TV, Music, inline search) are complete.

7. **tasks.md marking**: The orchestrator left pre-existing planning-rescope edits in tasks.md intentionally uncommitted. When marking tasks complete, commit only the code files, not tasks.md. The `- [x]` marking is present in the working tree for the orchestrator to commit separately if needed.

8. **Model preference**: Use `opencode-go/hy3` for both workers and reviewers (per user directive 2026-08-26).
