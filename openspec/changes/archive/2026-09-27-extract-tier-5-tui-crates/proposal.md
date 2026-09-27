# Proposal

## Why

Issue #814 Tier 5. After Tiers 1–4 the TUI crate is still ~115k lines, and
any edit under `src/app/` recompiles all of it. Its modules also depend on
each other in cycles, so nothing enforces the rule in AGENTS.md that
Interactive Components never receive `App`, `Config`, Service clients or
channels. Today that rule is kept only by review. Once components and painters
live in crates below the TUI crate, breaking it is a compile error.

Measured on `main` at `74cd74c6e`. Tier 3–4 (`extract-tier-3-4-provider-runtime-crates`,
PR #826) had not merged yet, so this plan assumes the layout it leaves behind.

## What Changes

Six new crates, in dependency order, taken out of `src/app/`. There is no
change to behaviour, rendering, key bindings, mouse handling, persistence, or
the config format.

| New crate | Moved from (`src/app/`) | Size |
|---|---|---|
| `mbv-theme` | `render/theme.rs` + `render/theme/` (palette, surfaces, semantic roles) | ~0.6k |
| `mbv-images` | the parts of `infra/images` that do not touch `App` (cache keys, `ImageCache`, protocol state, `CachedImage`, `ImageFetchReq`, compositing), plus `infra/resize` | ~1.2k |
| `mbv-ui-model` | the presentation types the shell pushes to components and painters (the `state::types` families and the pure halves of `state/*` that components or render name), `render/screens/sort_filter`, `infra/ui_util` | ~3k |
| `mbv-render` | `render/` without its `impl App` blocks, plus the paint-content types painters read that components own today, plus `infra/layout` | ~9k |
| `mbv-ui-msg` | `components/msg`, `components/component_id`, `components/user_event`, and the component identity payloads `Msg` carries | ~1.2k |
| `mbv-components` | `components/` without the above | ~31k |

Final graph, bottom-up over the Tier 1–4 crates:
`mbv-theme → mbv-images → mbv-ui-model → mbv-render → mbv-ui-msg →
mbv-components → mbv`.

Cycle cuts, all done inside the TUI crate before any extraction:

- **`impl App` leaves `render/`.** Eleven render files hold `impl App`
  blocks (~1.8k lines). An inherent impl cannot live outside `App`'s crate.
  They move into the TUI: the model builders go to a new
  `src/app/state/projection/`, the album cursor movement goes to `state/`, and
  settings activation goes to `dispatch/`. The App-building render test
  fixtures move with them.
- **Painters stop naming component types.** The row/paint-content types that
  painters read (`MediaListRow`, `MediaSemanticState`, `TreePaintRow`,
  `ThreeLineRole`, `SettingsRow`, `TransportAvailability`, `HeroImageState`,
  …) move from `components/` into `render/`. A painter that takes a whole
  component (`&WideMediaList`, `&ThreeLineFlatList`) takes a borrowed paint
  input struct defined in `render/` instead.
- **Components stop naming shell state.** About 30 items from `state/`
  (`QueueScope`, `PlaybackState`, `ContextMenuTargets`, `MultiSelectKind`,
  `ArtistKey`, `ArtistDetailProjection`, `SessionTargetKey`, `SettingsDestination`,
  `FeedForm`, …) and 2 from `dispatch/` (`VOLUME_STEP`,
  `audiobookshelf_book_queue_item`) move into the future `mbv-ui-model`. Files
  that mix these types with `impl App` blocks are split, and the `impl App`
  half stays in the TUI.
- **Images split.** The `impl App` fetch/protocol code in `infra/images`
  stays in the TUI as `infra/image_fetch`. `RENDER_FILTER` (the image
  resize filter) moves into `mbv-images`.

Changes that follow from the split:

- **BREAKING (inside the workspace only):** every moved path changes, and no
  re-export shim is left behind. The `crate::app::palette` facade and
  `render.rs`'s re-export of the theme constants are deleted, and callers use
  `mbv_theme::` directly.
- `pub(in crate::app)` items that a new boundary has to cross become `pub`.
  The compiler decides which ones.
- The test helpers `make_item` and `make_session`, which component tests use,
  move to `test`-feature modules of `mbv-emby-model` and `mbv-emby`.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `interactive-component-framework`: the requirement "Interactive surfaces are
  TuiRealm components" names the painters' location as
  `src/app/render/components/`. It is updated to the new crate, and it now says
  that neither `mbv-components` nor `mbv-render` depends on the TUI crate, so a
  component or painter that names `App` does not compile.

## Impact

- **Prerequisite:** `extract-tier-3-4-provider-runtime-crates` is merged
  (PR #826). The paths named here (`mbv_emby::SessionInfo`,
  `mbv_core::service_runtime::ServiceState`) are the ones it leaves behind.
  Group 0 checks this and stops if it has not merged.
- **New:** six `crates/mbv-*/` directories. Workspace `members` and
  `default-members` each gain six entries.
- **Call sites** (files outside the moving module that name it today):
  `components` 74, `render` 90, `palette` 43, `ui_util` 53, `images` 10,
  `layout` 6. All of these edits are forced by the compiler.
- **Visibility widening:** components + render hold ~750 `pub(crate)` /
  `pub(in crate::app…)` items. Only those the TUI still names become `pub`.
- **Tests:** tests move with the code they test. No test is rewritten or
  added, apart from the paint-input structs the painters now take.
- **Docs:** the repository map in `AGENTS.md`, the paths in
  `.agents/skills/mbv-frontend/SKILL.md` (14 mentions), and
  `docs/invariants/07-…` and `09-…`. ADRs are historical and stay as they are.
- **Stays in the TUI crate:** `shell/`, `state/` (except what moves to
  `mbv-ui-model`), `dispatch/`, `input/` (still the only keyboard routing
  site), `infra/` (signals, terminal, render cadence, visualizer glue, image
  fetch), and `src/app/tests/`.
- **Out of scope:** splitting `dispatch`/`shell`/`state` into their own crates;
  any rename of a moved type; making painters generic over their input;
  benchmarking build times.
- Umbrella: issue #814, Tier 5.
