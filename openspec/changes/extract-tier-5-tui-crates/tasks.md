# Tasks

Each numbered task in group 1, and each group from 2 on, is one commit that
leaves the workspace green. Groups run in order, because each extraction
depends on the crates below it.

**The gate** (run from the repo root after each commit's tasks):

```
cargo fmt
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
```

- Accept whatever `cargo fmt` reflows, and never revert it.
- Never add an `allow`/`expect` attribute to silence a lint. If a lint
  cannot be fixed at the source, stop and ask.
- Never add a `pub use` re-export for a moved path. The old path must not
  resolve.
- No painter output, key handling, mouse handling, or `Msg` payload may
  change. The existing tests pass unchanged, apart from their `use` paths.

**Every new crate's `Cargo.toml`** follows the shape of
`crates/mbv-text/Cargo.toml`: `name`, a one-line `description`, the seven
`*.workspace = true` package fields, and `[lints] workspace = true`. Path deps
are written `mbv-x = { path = "../mbv-x" }`. Add each crate to both
`[workspace] members` and `default-members` in the root `Cargo.toml`.

**Bulk path rewrites** use `sed -i` (or `ast-grep`) over `src/ crates/` for
the exact prefixes named in each task. `cargo check` output then drives the
rest. Split by hand any grouped `use crate::app::{…}` import that mixes moved
and unmoved names. Some paths are `super::`-relative and escape a moved
subtree (for example `use super::super::{palette, App};` in
`render/components/visualizer.rs`, or `use super::{palette, App};` in
`render.rs`). Fix these as the compiler reports them.

**Visibility in moved files** (design D8): first rewrite
`pub(in crate::app)` and `pub(in crate::app::<anything>)` to `pub(crate)`.
Then widen an item to `pub` only when the compiler reports a use of it from a
higher crate.

**Test gating** (design D9): when a `#[cfg(test)] pub(crate)` item becomes
unreachable from another crate's tests, change it to
`#[cfg(any(test, feature = "test"))] pub`. Add `test` to the owning crate's
`[features]`, and enable it in the caller's `[dev-dependencies]`.

**Naming pitfalls:**

- `src/app/components/` (Interactive Components → `mbv-components`) and
  `src/app/render/components/` (painters → `mbv-render`'s `components`
  module) are different trees with colliding file names (`queue.rs`,
  `settings.rs`, `media_list`, `help`, …). Every task names its tree.
- `render/components/settings.rs` holds settings *activation*, which goes to
  `dispatch/settings.rs`. `render/components/settings_component.rs` is the
  settings painter and stays in render.
- `mbv_emby_model` (DTOs) is not `mbv_emby` (the client, from Tier 3–4).
- `crate::app::palette` is a facade. The real code is `render/theme`.

## 0. Groundwork

- [x] 0.1 Confirm Tier 3–4 has merged. Run `gh pr view 826 --json state` and
  `git log --oneline -20`. Verify with all of:
  - PR #826 is `MERGED`;
  - `ls crates` shows `mbv-emby`, `mbv-audiobookshelf`, `mbv-player`, and
    `mbv-daemon`;
  - `rg 'mbv_core::(api|player|audiobookshelf)' src` is empty.

  If any check fails, stop and report which one.
- [x] 0.2 Rebase onto the latest `main` before each group. Verify:
  `git status` is clean and `git merge-base --is-ancestor origin/main HEAD`
  succeeds.
- [x] 0.3 Promote the TUI's UI dependencies to `[workspace.dependencies]`:
  `ratatui`, `crossterm`, `tuirealm`, `ratatui-image`, `image`,
  `unicode-width`, `textwrap`, and `tui-scrollbar`, each with its exact
  version/features line from the root `[dependencies]`. Rewrite the root
  entries as `<dep>.workspace = true`. Verify: gate passes, and
  `cargo tree -d -p mbv` shows no new duplicate versions of these crates.

## 1. Cycle cuts inside the TUI crate

Each task below is its own commit, and the full gate passes after each.

- [x] 1.1 Move the `impl App` blocks out of `src/app/render/` (design D3).
  For each file below, move the `impl App { … }` block, together with the
  private free functions and tests that only that block uses, to the target
  named here:
  - `render/components/{queue, tv_wide, music_wide, detail, chrome_status,
    chrome_player_context, card, list_context}.rs` →
    `src/app/state/projection/<same stem>.rs`;
  - `render/components/visualizer.rs` → `src/app/state/projection/visualizer.rs`,
    moved whole;
  - `render/screens/album_cursor.rs` → `src/app/state/album_cursor.rs`, moved
    whole;
  - `render/components/settings.rs` → `src/app/dispatch/settings.rs`, moved
    whole.

  Create `src/app/state/projection.rs` declaring the new modules, and delete
  the emptied `mod` lines in `render/components.rs` and `render/screens.rs`.
  Move `src/app/render/fixtures.rs` to `src/app/tests/render_fixtures.rs` and
  rewrite its callers. A render test that builds an `App` moves to the file
  its subject moved to. Verify: gate passes, and
  `rg '\bimpl App\b|crate::app::App\b|super::super::\{?[^}]*\bApp\b|make_(movie|music_group|queue)_app' src/app/render`
  is empty.
- [x] 1.2 Split `src/app/infra/images` at the `impl App` line (design D6).
  Create `src/app/infra/image_fetch.rs` + `image_fetch/`. Move into it both
  `impl App` blocks of `images.rs` with their private helpers, the
  `impl App` half of `images/protocol.rs`, `images/fetch.rs`, and
  `images/fetch/*`. Move `RENDER_FILTER` from
  `render/components/widgets.rs` into `infra/images.rs`. Move
  `infra/resize.rs` to `infra/images/resize.rs`. Verify: gate passes, and
  `rg 'crate::app::(state|dispatch|shell|components|render|input|tests)|\bApp\b|LibEvent|PAGE_SIZE' src/app/infra/images src/app/infra/images.rs`
  shows only `#[cfg(test)]` hits, or none.
- [x] 1.3 Small leftovers (design D7):
  - move `ui_util::service_state_color` into `render/components/widgets.rs`;
  - move `PAGE_SIZE` and `PREFETCH_AHEAD` from `infra/layout.rs` into a new
    `src/app/infra/paging.rs`;
  - move `make_item` into a new `crates/mbv-emby-model/src/test_support.rs`,
    gated `#[cfg(any(test, feature = "test"))]` with `[features] test = []`;
  - move `make_session` into `crates/mbv-emby/src/test_support.rs`, gated the
    same way;
  - delete both from `src/app/tests.rs`, and point every caller at the new
    paths (with the `test` feature in the root `[dev-dependencies]`).

  Verify: gate passes; `rg 'palette' src/app/infra/ui_util.rs` is empty; and
  `rg 'fn make_(item|session)\b' src` is empty.
- [x] 1.4 Stage `src/app/ui_model/` (design D2, D5). Create
  `src/app/ui_model.rs` and move into it:
  - the `state/types/*` files that `components/` or `render/` name (at least
    `playback`, `context_menu`, `confirm`, `audiobookshelf_browse`,
    `settings`, `feed_tab`, `feeds_manage`, `overlay`, `browse`, `feed`,
    `sidebar`, `library_tab`, `player_tab`, `tab_selection`, `daemon_lost`,
    `events`, and whatever they import);
  - the plain types and functions of the mixed files, splitting each at its
    `impl App` block, with the `impl App` half staying in `state/`:
    `music_grouping` (`ArtistKey`, `MusicGroupingState`),
    `music_artist_detail` (`ArtistDetailProjection`, `ArtistTrackGroup`, and
    the other plain types), `panel_targets` (`PanelTarget`, `SessionTargetKey`),
    `home_latest` (`provider_timestamp_secs` and the launch-window types),
    `playback_target` (`NowPlayingStatus`), `search_sidebar`
    (`SearchSidebar`), and `queue_owner::QueueOrigin`;
  - `render/screens/sort_filter.rs` and `infra/ui_util.rs` (the latter as
    `ui_model/ui_util.rs`);
  - `components::media_list::SelectionOrigin`, `components::msg::HomeRowTarget`,
    `dispatch::action::VOLUME_STEP`, and
    `dispatch::audiobookshelf::browse::audiobookshelf_book_queue_item`;
  - `normalize_list_pane_width` and `normalize_queue_column_width`, placed
    per design D5 (`render/arrangements/…` or `ui_model` together with the
    constants they read).

  Delete the `crate::app::` root re-exports of moved types in `src/app.rs`.
  Callers name `crate::app::ui_model::…` instead. Verify: gate passes, and
  `rg 'crate::app::(state|dispatch|shell|input|infra|components|render|tests)\b|\bApp\b|super::super' src/app/ui_model src/app/ui_model.rs`
  shows no production hits.
- [x] 1.5 Cut `render → components` (design D4). Move the paint-content
  types listed in design Context (`media_list` row/paint types,
  `tree_browser` paint types, `ThreeLineRole`,
  `ServiceRow`/`SettingsRow`/`SetupDraft`, `TransportAvailability`,
  `HeroImageState`, and whatever `help` item render names) from
  `src/app/components/…` into the `src/app/render/components/…` file that
  paints them. Add `WideMediaListPaintInput<'a>` and
  `ThreeLineFlatListPaintInput<'a>`, which borrow exactly the fields
  `render_wide_media_list_component` and `render_three_line_flat_list` read.
  Change those two painters to take them, and build them in the two
  components' `view()`. Verify: gate passes, and
  `rg 'crate::app::(components|state|dispatch|shell|input|tests)\b|\bApp\b' src/app/render src/app/render.rs`
  shows no production hits. The only `crate::app::` paths left in render are
  `ui_model`, `infra::images`, and `render` itself.
- [x] 1.6 Stage `src/app/ui_msg/` (design D2). Create `src/app/ui_msg.rs` and
  move into it `components/msg.rs` + `components/msg/`,
  `components/component_id.rs`, `components/user_event.rs`, and the
  identity payloads `Msg` names: `SelectionSummary` (from
  `components/media_list`), `LibraryKey`/`LibraryKind` (from
  `components/library_panel/owner.rs`), and `TvTreeTarget` (from
  `components/tv_tree_target.rs`). Verify: gate passes, and
  `rg 'crate::app::(components|state|dispatch|shell|input|infra|tests)\b|\bApp\b' src/app/ui_msg src/app/ui_msg.rs`
  shows no production hits.
- [x] 1.7 Check that `components/` names nothing above it. Fix any remaining
  hit by applying the design D5 rule. Verify: gate passes, and
  `rg 'crate::app::(state|dispatch|shell|input|tests|test_seams)\b|\bApp\b|crate::app::infra::(?!images)' --pcre2 src/app/components src/app/components.rs`
  shows no production hits.

## 2. `mbv-theme`

- [x] 2.1 Create `crates/mbv-theme`. Deps: `ratatui`. `git mv` the
  following:
  - `src/app/render/theme.rs` → `src/lib.rs`;
  - `src/app/render/theme/*` → `src/`.

  Rewrite `crate::app::render::theme::` → `crate::`. Verify:
  `cargo nextest run -p mbv-theme` passes.
- [x] 2.2 Delete `mod theme;` and the `pub(crate) use theme::{…}` re-export
  block from `render.rs`. Delete `src/app/infra/palette.rs` and its `mod`
  line, and remove `palette` from the `pub(crate) use self::infra::{…}` line
  in `src/app.rs`. Add `mbv-theme` to the root `[dependencies]`. Rewrite
  `crate::app::palette::`, `crate::app::infra::palette::`, and theme names
  imported from `crate::app::render::` → `mbv_theme::`. Rewrite
  `use crate::app::palette;` + `palette::X` → `use mbv_theme as palette;`
  only if the compiler forces it. Prefer direct `mbv_theme::X` imports.
  Verify: gate passes, and `rg 'app::palette|infra::palette|render::theme' src`
  is empty.

## 3. `mbv-images`

- [x] 3.1 Create `crates/mbv-images`. Deps: `mbv-theme`, `ratatui`,
  `ratatui-image`, `image`, `log`, plus others only if the compiler asks.
  `git mv` the following:
  - `src/app/infra/images.rs` → `src/lib.rs`;
  - `src/app/infra/images/*` → `src/`.

  Rewrite `crate::app::infra::images::` / `crate::app::images::` →
  `crate::`. Verify: `cargo nextest run -p mbv-images` passes, and
  `rg 'crate::app' crates/mbv-images` is empty.
- [x] 3.2 Delete the `images` `mod` line in `infra.rs` and the `images`
  entry in `src/app.rs`'s `pub(crate) use self::infra::{…}`. Add
  `mbv-images` to the root `[dependencies]`. Rewrite
  `crate::app::images::` and `crate::app::infra::images::` → `mbv_images::`.
  Verify: gate passes, and `rg 'app::images|infra::images' src` is empty.

## 4. `mbv-ui-model`

- [x] 4.1 Create `crates/mbv-ui-model`. Deps: `mbv-core` (for
  `ServiceState`/`SetupGeneration`), `mbv-emby-model`, `mbv-queue`,
  `mbv-feed`, `mbv-config`, and `mbv-audiobookshelf` as the compiler asks,
  plus `unicode-width`. `git mv` the following:
  - `src/app/ui_model.rs` → `src/lib.rs`;
  - `src/app/ui_model/*` → `src/`.

  Rewrite `crate::app::ui_model::` → `crate::`. Verify:
  `cargo nextest run -p mbv-ui-model` passes, and
  `rg 'crate::app' crates/mbv-ui-model` is empty.
- [x] 4.2 Delete `mod ui_model;` from `src/app.rs`. Add `mbv-ui-model` to the
  root `[dependencies]`. Rewrite `crate::app::ui_model::` → `mbv_ui_model::`.
  Verify: gate passes, and `rg 'app::ui_model' src` is empty.

## 5. `mbv-render`

- [x] 5.1 Create `crates/mbv-render`. Deps: `mbv-theme`, `mbv-images`,
  `mbv-ui-model`, `ratatui`, `ratatui-image`, `textwrap`, `unicode-width`,
  `tui-scrollbar`, plus the Tier 1–4 crates the compiler asks for. `git mv`
  the following:
  - `src/app/render.rs` → `src/lib.rs`;
  - `src/app/render/*` → `src/`;
  - `src/app/infra/layout.rs` → `src/layout.rs`, declared in `lib.rs`.

  Rewrite `crate::app::render::` → `crate::`, and `crate::app::layout::` /
  `crate::app::infra::layout::` → `crate::layout::`. Verify:
  `cargo nextest run -p mbv-render` passes, and
  `rg 'crate::app' crates/mbv-render` is empty.
- [x] 5.2 Delete `pub mod render;` from `src/app.rs`, the `layout` `mod` line
  in `infra.rs`, and the `layout` entry and the layout-constant re-exports in
  `src/app.rs`. Add `mbv-render` to the root `[dependencies]`. Rewrite
  `crate::app::render::` → `mbv_render::`, and `crate::app::layout::` /
  `crate::app::infra::layout::` plus the root layout constants
  (`LEFT_WIDTH_DEFAULT`, `TWO_COLUMN_THRESHOLD`, …) → `mbv_render::layout::`.
  Verify: gate passes, and `rg 'app::render\b|app::layout|infra::layout' src`
  is empty.

## 6. `mbv-ui-msg`

- [ ] 6.1 Create `crates/mbv-ui-msg`. Deps: `mbv-ui-model`, `mbv-render` (only
  if the compiler asks), `mbv-emby-model`, `mbv-queue`, `mbv-feed` (as the
  compiler asks), and `tuirealm`. `git mv` the following:
  - `src/app/ui_msg.rs` → `src/lib.rs`;
  - `src/app/ui_msg/*` → `src/`.

  Rewrite `crate::app::ui_msg::` → `crate::`. Verify:
  `cargo nextest run -p mbv-ui-msg` passes, and `rg 'crate::app' crates/mbv-ui-msg`
  is empty.
- [ ] 6.2 Delete `mod ui_msg;` from `src/app.rs`. Add `mbv-ui-msg` to the root
  `[dependencies]`. Rewrite `crate::app::ui_msg::` → `mbv_ui_msg::`. Verify:
  gate passes, and `rg 'app::ui_msg' src` is empty.

## 7. `mbv-components`

- [ ] 7.1 Create `crates/mbv-components`. Deps: `mbv-ui-msg`, `mbv-render`,
  `mbv-ui-model`, `mbv-images`, `mbv-theme`, `tuirealm`, `ratatui`,
  `ratatui-image`, `unicode-width`, and `mbv-text`, plus the Tier 1–4 crates
  the compiler asks for. Dev-deps: `rstest`, plus `mbv-emby-model` and
  `mbv-emby`, each with `test`. `git mv` the following:
  - `src/app/components.rs` → `src/lib.rs`;
  - `src/app/components/*` → `src/`.

  Rewrite `crate::app::components::` → `crate::`. Verify:
  `cargo nextest run -p mbv-components` passes, and
  `rg 'crate::app|mbv_core::|\bApp\b' crates/mbv-components` shows no code
  hits (doc comments that mention `App::…` are allowed).
- [ ] 7.2 Delete `pub mod components;` from `src/app.rs`. Add
  `mbv-components` to the root `[dependencies]`. Rewrite
  `crate::app::components::` → `mbv_components::` (~74 files). Verify: gate
  passes, and `rg 'app::components\b' src` is empty.

## 8. Crate graph

- [ ] 8.1 Check the graph. Verify all of:
  - `cargo tree -p mbv-components -e normal --prefix none | rg '^mbv '` is
    empty;
  - `cargo tree -p mbv-render -e normal` contains neither `mbv-components` nor
    `mbv-ui-msg`;
  - `cargo tree -p mbv-ui-model -e normal` contains none of `mbv-render`,
    `mbv-ui-msg`, or `mbv-components`;
  - `cargo tree -p mbv-images -e normal` contains only `mbv-theme` among the
    `mbv-*` crates;
  - `cargo tree -p mbv-theme -e normal` contains no `mbv-*` crate.

## 9. Docs and pre-push

- [ ] 9.1 Update `AGENTS.md`:
  - in the repository map, add one line each for `mbv-theme`, `mbv-images`,
    `mbv-ui-model`, `mbv-render`, `mbv-ui-msg`, and `mbv-components`;
  - repoint the `src/app/components/` and `src/app/render/` lines to
    `crates/mbv-components/` and `crates/mbv-render/`;
  - in "Interactive architecture", add one bullet: components and painters
    cannot name `App` because their crates sit below the TUI crate.

  Verify: `rg 'src/app/(components|render)' AGENTS.md` is empty, and all six
  crate names appear.
- [ ] 9.2 Repoint the 14 `src/app/(render|components)` paths in
  `.agents/skills/mbv-frontend/SKILL.md`, and the path mentions in
  `docs/invariants/07-colour-value-sharing-is-deliberate.md` and
  `docs/invariants/09-completed-frame-hit-claim.md`, to the new crate paths.
  Leave ADRs unchanged. Verify:
  `rg 'src/app/(render|components)' .agents docs/invariants` is empty.
- [ ] 9.3 Run `make check-code-file-lines` before pushing. Split any file it
  flags along a responsibility seam, following the `splitting-files` skill.
  Verify: the check passes.
- [ ] 9.4 Comment on issue #814. Summarise the Tier 5 crates, the graph
  (`mbv-theme → mbv-images → mbv-ui-model → mbv-render → mbv-ui-msg →
  mbv-components → mbv`), and the deviation from the issue: the single
  `mbv-ui-msg` became `mbv-ui-model` below render plus `mbv-ui-msg` above it
  (design D2). Verify: `gh issue view 814 --comments` shows the comment.
