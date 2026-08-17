# Handoff: centralize-ui-design-language

## Status

Phases 0-6 complete, committed, and pushed (`worktree-centralize-ui-design-language`,
commit `dc41e34b`). Phases 7-10 remain. `tasks.md` has detailed completion notes for
every finished task — read it first, it's the actual source of truth for what was done
and why. This file is just the resume pointer.

## Where you are

Worktree: `/home/slatkin/Dev/mbv/.claude/worktrees/centralize-ui-design-language`
Branch: `worktree-centralize-ui-design-language`, tracking `origin/worktree-centralize-ui-design-language`.

Working tree is clean as of this handoff. Next task is **7.1** (move feeds onto
hero-on-top).

## Read before resuming

1. `openspec/changes/centralize-ui-design-language/tasks.md` — full task list with
   completion notes through phase 6.
2. `openspec/changes/centralize-ui-design-language/design.md` — the six-decision design,
   especially decision 2 (components paint, arrangements compose), decision 4 (hero-on-top
   vs hero-on-left source screens), decision 6 (domain content is data, not a declaration).
3. `src/app/render/hero.rs` — the component catalogue built in phases 3-5: `top_hero_layout`/
   `hero_block_shell` (hero-on-top), `hero_on_left_panes`/`hero_on_left_right_pane`/
   `paint_hero_on_left_text` (hero-on-left), `paint_hero_content`/`HeroContent` (hero-on-top's
   text/image painter).

## Standing constraints (still in force)

- **No delegation to subagents on this project.** Write code directly. This has been
  requested repeatedly and explicitly.
- **Pause after each phase** for human review before starting the next.
- **Run `cargo nextest run -p mbv`** after each phase; also `rtk make check-code-file-lines`.
- **Never push without being asked.** Commit freely; push only on explicit instruction
  (this session's push was explicitly requested).
- Use the throwaway, gitignored `src/app/render/capture_harness.rs` (run via
  `rtk cargo nextest run -p mbv capture_harness --run-ignored all`, output in
  `target/ui-captures/`) to verify rendering. For no-visible-change phases, diff against
  `target/ui-captures-baseline/` and require byte-identical output. For phases with an
  intended visible change (phase 7 explicitly — feeds/home videos gain a wide arrangement
  that didn't exist before), inspect the captures visually instead.
- Phase 10.1 deletes the capture harness and captures — don't delete them before then.

## Known gotcha (already fixed, but worth knowing)

`capture_harness.rs`'s `WIDE_WIDTH` constant used to be sized against raw terminal width,
not the actual content area reaching each screen (which is ~44 columns narrower after the
queue sidebar/gap/tab padding). It was fixed this phase to `TWO_COLUMN_THRESHOLD + 64`,
which was empirically verified to land the content area past the 82-column breakpoint. If
you add a new capture width or content assertion, sanity-check it renders the arrangement
you think it does (eyeball the output) rather than trusting the constant alone — the old
bug meant every "wide" capture in phases 3-5 was silently exercising the narrow arrangement
instead, and only visual inspection caught it.

## Remaining phases

- **7. New wide arrangements: feeds and home videos** — unlike phases 3-6, this is
  genuinely new behavior (these screens have no prior wide arrangement), so 7.3 calls for
  visual review, not a byte-diff.
- **8. Unified mouse hit targets** — touches `LayoutMain`, `input_mouse.rs`,
  `input_mouse_panels.rs`, `lib_cursor_actions.rs`. 8.5 requires manual click verification
  on all eight screens in both arrangements — flag this to the user rather than assuming
  it's done from static analysis alone.
- **9. Chrome component files** — 9.1 finishes the `render_main` decomposition and
  finally removes the two breakpoint tests deferred from task 2.4 (`render/mod.rs:378`,
  `:526`) now that hero-on-top/hero-on-left own that behavior.
- **10. Close-out** — deletes the capture harness, runs clippy + full nextest (`-p mbv -p
  mbv-core`), checks file-line caps, updates `CONTEXT.md`, and archives the change
  (merges deltas into `openspec/specs/`).
