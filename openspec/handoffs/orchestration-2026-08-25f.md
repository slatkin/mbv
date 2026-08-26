# Orchestration handoff — migrate-tui-to-tuirealm, 2026-08-25 (sixth)

**Outgoing orchestrator: FastJaguar**

Supersedes `orchestration-2026-08-25e.md` where they conflict. Read that file
for the earlier campaign history and sizing lessons; this handoff is the
current resume point.

## Resume state

- Worktree: `/home/slatkin/Dev/mbv/.worktrees/migrate-tui-to-tuirealm`
- Branch: `feat/migrate-tui-to-tuirealm`
- Accepted HEAD: `b21653dc4e91c02dfeddd0a64b37bb7f1984603d`
  (`docs: qualify Home migration status`)
- No implementation or review agent is in flight.
- OpenSpec change: `migrate-tui-to-tuirealm`, nested row
  `5.3d > Teardown — framework removal`.
- Parent 5.3d, nested `Mirrors and framework`, 5.5, and 5.6 remain unchecked.
  That is correct.

The worktree has intentional pre-existing tracked deletions under
`.agents/skills/ast-grep*` and untracked `.pi/` plus orchestration handoffs.
The user said to leave the skill deletions deleted. Never use `git add -A`;
stage only the exact files for a unit. Do not commit `.pi/` or handoff files.

## Current delegation/review policy

Use the bounded implementation cadence:

1. One exact nested `tasks.md` row plus one coherent ownership family.
2. Give the worker an exact clean start SHA.
3. One new commit; do not amend and do not push.
4. Verify the reported commit with `git show <sha>`; do not rerun checks the
   worker already reported.
5. One review per commit. The orchestrator makes the final acceptance call.
6. If review rejects, dispatch only the smallest correction, create one new
   commit, and review that correction.

Model preference saved by the user:

- implementation/scoping workers: exact registry ID
  `openrouter/deepseek/deepseek-v4-flash-0731`
- reviewers: exact registry ID `openai-codex/gpt-5.6-luna`
- avoid `opencode-go`; the user is near its limits

Do not modify `~/.pi/agent/pi-messenger.json`; this preference is for
orchestration calls unless the user separately requests config changes.

Campaign constraints still apply:

- Serial one-writer only in the shared worktree.
- Normally 3–6 production files; one data-flow/ownership family per unit.
- Do not combine effect keys, state deletion, legacy renderer deletion,
  framework teardown, and documentation.
- No behavior-preservation tests for 5.3d; adapt existing tests only. A narrow
  regression test is allowed only for a defect introduced by the migration.
- Use `rtk` for checks.
- Do **not** run `make check-code-file-lines` until the final PR/5.6 gate. The
  scout suggested it from `tasks.md`, but the campaign-specific maintainer
  instruction defers it.
- No push or PR unless the user explicitly asks.

## Home phase accepted

Home content/state/preference ownership is complete:

- `b3fbf5b0` — re-home Home content from App to shell/Model
- `5c3e8a9e` — record the Home content re-home
- `4f98154f` — re-home Home section preference/pending restore marker
- `1459fd64` — record that ownership move
- `a742166b` — gate the test-only `home_section_pref` helper
- `b21653dc` — correct the ledger status after review

Reported verification for the 2c code: `cargo check` 0 errors, full mbv nextest
1152 passed, clippy 0 errors, fmt clean, ast-grep retained the 69 pre-existing
diagnostics (including pre-existing `src/app/render/screens/root.rs:65`).

Luna's first review accepted the code but rejected the docs because Home still
falls through to the no-op legacy `App::handle_key` catch-all and
`LegacyInput`/`CONTEXT_STACK` teardown remains. `b21653dc` moved the Home ledger
row back to `component` and qualified the scoping note. Luna's follow-up review
returned ACCEPT / OK. Do not mark Home `migrated` until framework fallback is
deleted.

## Next bounded family: Audiobookshelf podcast mirror, Phase A

A read-only scout completed from accepted HEAD `b21653dc`. Its full local output
is `/tmp/mbv-543d-abs-podcast-scout-full.md`; the banked findings are below.

### Current mirror

`Model::sync_audiobookshelf_podcast` in
`src/app/shell_audiobookshelf_podcast.rs` currently does two independent jobs:

1. mount/activate/unmount reconciliation for the active ABS podcast tab;
2. per-frame projection of `App::audiobookshelf_browse[index]`, focus, and image
   preference into `AudiobookshelfPodcastComponent::set_content`.

Phase A removes only job 2 from the frame loop and replaces it with targeted
pushes at real writer seams. Mount lifecycle stays. This follows the prior
`sync_home` push-first ordering.

### Phase A scope

Implement only the push seam:

- Add a focused Model helper such as
  `push_audiobookshelf_podcast_content(index)` in
  `src/app/shell_audiobookshelf_podcast.rs`.
- It must downcast the mounted component and call the existing `set_content`
  with exactly the current snapshot,
  `effective_panel_focus() == PanelFocus::Library`, and
  `app.images_enabled()`.
- Retain mount/activate/unmount behavior, but remove the per-frame content
  projection and the `self.sync_audiobookshelf_podcast()` frame-loop call.
- Push on active podcast tab entry/activation so the new component receives its
  first snapshot.
- Push after the existing podcast key/effect dispatch choke point.
- Push after asynchronous writers that alter the active podcast snapshot:
  catalog/page/detail completions, progress reconciliation, refresh/reset, and
  saved-position restoration. Prefer existing drain/completion choke points
  over adding a call to every low-level mutation.
- Recount actual production readers/writers before editing; if a required seam
  forces more than roughly 8 production files, stop and report the confirmed
  coupling instead of expanding silently.
- Adapt the existing shell podcast test to drive mount plus the push helper.

Parity requirements:

- Preserve `AudiobookshelfPodcastComponent::set_content` verbatim. Its
  selected-show/filter/episode-selection/scroll preservation is the current
  parity authority.
- Preserve focused-state and image-preference expressions exactly.
- The component must be mounted before its initial push.
- Keep `App::audiobookshelf_browse` and all content/cache/effect state.
- Keep the existing key-forward/effect path in Phase A.
- Keep the legacy podcast renderer, cover fetch, `LegacyInput`,
  `CONTEXT_STACK`, `Msg::Legacy`, and `ShellRequest::AudiobookshelfPodcastKey`.
- Do not touch the sibling ABS book surface.
- Keep the Audiobookshelf podcast ledger row `component`.
- No docs or checkbox changes in Phase A.

Likely production seams to inspect before patching:

- `src/app/shell_audiobookshelf_podcast.rs`
- `src/app/shell.rs`
- `src/app/run_loop_drains.rs`
- `src/app/lib_event_actions.rs`
- `src/app/library_position_state.rs`
- `src/app/audiobookshelf_service_actions.rs`
- `src/app/app_audiobookshelf_service_completion.rs`
- `src/app/audiobookshelf_browse_actions.rs`

This is an inspection list, not permission to edit all files. Use the narrowest
existing post-drain/post-dispatch seams.

Required Phase A checks:

- `rtk cargo check -p mbv`
- `rtk cargo nextest run -p mbv abs_podcast`
- `rtk cargo clippy --workspace --all-targets`
- `rtk ast-grep scan` (69 pre-existing diagnostics are the baseline; no new
  touched-file findings)
- `rtk cargo fmt --all -- --check`

Do not run the line-cap gate yet. Suggested commit message:
`5.3d: push Audiobookshelf podcast content at writers`.

### Suggested next implementation prompt

Implement only the exact nested OpenSpec tasks.md row 5.3d > Teardown —
framework removal, bounded family Audiobookshelf podcast per-frame content
mirror Phase A, starting from clean HEAD b21653dc. Replace only the per-frame
content projection in Model::sync_audiobookshelf_podcast with a targeted Model
push helper invoked at the real active-tab, key/effect, async completion,
progress, refresh/reset, and saved-position writer choke points. Preserve the
mount lifecycle and AudiobookshelfPodcastComponent::set_content semantics
exactly. Keep App::audiobookshelf_browse, the key-forward/effect path, the
legacy renderer and cover fetch, LegacyInput/CONTEXT_STACK/Msg::Legacy,
ShellRequest::AudiobookshelfPodcastKey, the ABS book surface, docs, and all
checkboxes unchanged. Recount writers first and stop if the unit cannot stay
within roughly eight production files. Adapt existing tests only; do not add
behavior-preservation tests. Verify with rtk cargo check -p mbv, focused
abs_podcast nextest, workspace clippy, ast-grep against the 69-diagnostic
baseline, and cargo fmt --check; do not run check-code-file-lines. Commit as
one new commit named 5.3d: push Audiobookshelf podcast content at writers; do
not amend or push; report the SHA, exact files, writer seams covered, checks,
and residual risks. Do not touch the intentional deleted .agents/skills files,
untracked .pi/, or openspec/handoffs.

## Phase B — explicitly not next yet

After Phase A is accepted and reviewed, separately scope the real podcast
interaction teardown:

- interaction state moving out of App: `selected_id`, `episode_filter`,
  `episode_selection`, `scroll`, and visible-episode selection derived from
  them;
- content/cache/effect state staying App-owned: library, shows, totals/pages,
  loading/error, episodes/detail cache/loading, and progress;
- typed component messages replacing the podcast key forward;
- deletion of podcast App interaction handlers and legacy renderer reads;
- removal of `Msg::Legacy`/terminal reconstruction only from the podcast
  component when safe.

Phase B has one confirmed design dependency: podcast cover fetching currently
lives in the legacy renderer. Decide on a thin shell/Model cover-fetch bridge
before asking an implementer to delete that renderer. Do not fold this decision
or Phase B into Phase A.

## Remaining order after podcast

1. Audiobookshelf podcast Phase A, review, then separately scoped Phase B.
2. Audiobookshelf book mirror, independently scoped.
3. Emby browser, TV workspace, and Music workspace families; the shared
   `BrowseLevel` cursor coupling remains a known blocker requiring design.
4. `CONTEXT_STACK` and remaining `handle_key_*` endpoints.
5. `LegacyInput`, `Msg::Legacy`, `LegacyTerminalEvent`, and reconstruction
   adapters.
6. Final ledger/task updates and 5.6 verification gate.

— FastJaguar
