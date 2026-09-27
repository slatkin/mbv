# Design

## Context

Edges between `src/app/` modules, counted as `crate::app::<module>`
references in production code (tests excluded) on `main` at `74cd74c6e`.
Grouped `crate::app::{…}` imports and `super::` paths add more, but none of
them adds an edge that is not listed here.

- **`render/theme` is already a leaf.** It uses only `ratatui`.
  `infra/palette.rs` (22 lines) is a re-export facade over `render::theme`,
  and `render.rs` re-exports ~50 theme constants.
- **`render → App`:** eleven render files contain `impl App` blocks:
  `components/{queue, tv_wide, music_wide, detail, chrome_status,
  chrome_player_context, card, settings, visualizer, list_context}.rs` and
  `screens/album_cursor.rs`. `components/settings.rs` holds only settings
  *activation* (effects), and `screens/album_cursor.rs` holds cursor movement.
  None of this is painting. `render/fixtures.rs` builds `App`s for tests.
- **`render → components` (≈10 item families):** painters read
  component-owned row and paint types (`media_list::{MediaListRow,
  MediaSemanticState, MediaKind, ActiveProgress, MediaListTrailing,
  MediaListTitleReveal, RowGeometry, SelectedRowSurface, row_marquee_key,
  queue_row_background, queue_row_zebra_stripe}`, `list::tree_browser::{TreePaintRow,
  TreePaintRowKind, TreeAggregateMark, TreeTitleRole}`, `list::ThreeLineRole`,
  `settings::{ServiceRow, SettingsRow, SetupDraft}`,
  `library_playback_panel::TransportAvailability`,
  `library_panel::content::HeroImageState`, `help`). Two painters take a whole
  component: `render_wide_media_list_component(…, &WideMediaList, …)` and
  `render_three_line_flat_list(…, &ThreeLineFlatList)`.
- **`render → state`:** `SettingsDestination`, `OverlayRequest`,
  `FeedForm`/`FeedFormField`/`FeedsManageStage`, `MultiSelectKind`,
  `BrowseResting`, `AudiobookshelfBookBrowseState`, `SearchSidebar`,
  `ArtistKey`, `ArtistDetailProjection`, `normalize_list_pane_width`, and
  root re-exports `PanelFocus`, `PanelMode`, `TabSelection`, `SidebarId`,
  `NowPlayingStatus`, `PlaybackState`, `RemoteSlotState`, `SeriesDetail`.
- **`components → state` (≈30 items):** the list above, plus `QueueScope`,
  `ContextMenuTargets` and the other `context_menu` types, `ConfirmAction`/
  `ConfirmModal`, `audiobookshelf_browse::*`, `WatchedFilter`,
  `PanelTarget`/`SessionTargetKey`, `ArtistTrackGroup`,
  `provider_timestamp_secs`, `normalize_queue_column_width`, `AlbumSearchEntry`,
  and `SavePlaylistStage`.
- **`components → dispatch`:** `action::VOLUME_STEP` and
  `audiobookshelf::browse::audiobookshelf_book_queue_item`.
- **`components → app::tests`:** `make_item` (builds an `EmbyItem`) and
  `make_session` (builds an Emby `SessionInfo`), used by component tests.
- **`components/msg → components, state`:** `Msg` carries `SelectionOrigin`,
  `SelectionSummary`, `LibraryKey`, `TvTreeTarget`, `QueueScope`,
  `ContextMenuTargets`, `MultiSelectKind`, and `SessionTargetKey`.
- **Upward edges inside the state types themselves:** `types/browse.rs` names
  `render::{LetterFilter, LetterFilterKind, resolve_tv_content_mode}`
  (`screens/sort_filter`). `types/context_menu.rs` names
  `components::media_list::SelectionOrigin` and `msg::HomeRowTarget`.
  `list_pane_width.rs` names `wide_hero::{WIDE_HERO_MIN_PANE_WIDTH,
  WIDE_HERO_PANE_GAP}`. `music_grouping.rs`, `music_artist_detail.rs`,
  `panel_targets.rs`, `queue_column_width.rs`, and `playback_target.rs` each
  mix plain types with an `impl App` block.
- **`infra/images`:** `images.rs`, `protocol.rs`, `fetch.rs`, and `fetch/*`
  hold `impl App` blocks. `cache.rs`, the cache-key functions, `CachedImage`,
  `ImageFetchReq`, `ImageSource`, compositing, and `impl ImageCache` do not.
  `impl ImageCache` uses `RENDER_FILTER` (defined in `render/components/widgets.rs`)
  and the palette. `resize.rs` is App-free.
- **`infra/ui_util`:** plain functions. Only `service_state_color` touches
  the palette. **`infra/layout`:** constants plus geometry structs, and it names
  `render::arrangements::chrome::RootFrame`. `PAGE_SIZE` and `PREFETCH_AHEAD`
  are fetch-window constants that only the TUI uses.

## Goals / Non-Goals

**Goals:**

- The graph is acyclic and reads, bottom-up: `mbv-theme → mbv-images →
  mbv-ui-model → mbv-render → mbv-ui-msg → mbv-components → mbv`. Each crate
  may also use any Tier 1–4 crate. No new crate depends on the TUI crate.
- `mbv-render` and `mbv-components` do not name `App`, shell, dispatch,
  input, or state modules. The spec delta states this as a requirement.
- Old paths do not resolve, and no re-export shims are added.
- Every cut moves existing code to the place that owns it. The paint-input
  structs of D4 are the only new types.

**Non-Goals:**

- Removing the dependencies that `mbv-components` legitimately has on the
  provider DTO crates (`mbv-emby-model`, `mbv-queue`, `mbv-feed`,
  `mbv-audiobookshelf` row types). Presentation models carry these today.
- Splitting `state`/`dispatch`/`shell` further, or moving `input/`.
- Splitting a file just because it moved. The pre-push line check governs
  that.

## Decisions

### D1: Make every cut inside the TUI crate first, then extract with pure moves

This is the Tier 2 / Tier 3–4 D1 pattern. Group 1 makes all the cycle cuts
while `src/app/` is still one crate, so `cargo check -p mbv` proves each one.
Each cut also stages the future crate as an in-crate module whose boundary
`rg` can check: `src/app/ui_model/` and `src/app/ui_msg/` are created, and
`render/` and `components/` are cleaned up where they are. Extraction groups
then follow the Tier 1 shape: new manifest, `git mv`, path rewrite, delete
the old `mod` line. They run bottom-up.

### D2: Two shell↔component vocabulary crates, one either side of render

The types the shell pushes *into* painters and components (projection
payloads and shell-state enums) are needed by `mbv-render`. The `Msg` family
components send *out* is not. It also carries component identities
(`SelectionSummary`, `LibraryKey`, `TvTreeTarget`) that sit next to
render-level types. So:

- `mbv-ui-model`, below render, holds the moved state types, the pure halves
  of the mixed state files, `screens/sort_filter` (content preparation that
  `BrowseLevel` stores a `LetterFilter` from), `ui_util` minus
  `service_state_color`, `SelectionOrigin`, `HomeRowTarget`, `VOLUME_STEP`, and
  `audiobookshelf_book_queue_item`.
- `mbv-ui-msg`, above render, holds `Msg` and its request enums,
  `ComponentId`, `UserEvent`, and the component identity payloads `Msg`
  carries (`SelectionSummary`, `LibraryKey`/`LibraryKind`, `TvTreeTarget`).

Alternatives rejected:
- A single `mbv-ui-msg` below render (the issue's one name). Every `Msg` edit
  would then recompile the painters, and render would depend on `tuirealm`
  event types it never uses.
- Folding the model types into `mbv-render`. That makes the render crate the
  owner of `QueueScope`, `ContextMenuTargets`, and `PlaybackState`, which are
  shell concepts, not painting concepts.

### D3: `impl App` leaves render for `state/projection`, `state`, and `dispatch`

Each render `impl App` block moves, with the private helpers only it uses, to:

- `state/projection/<same file stem>.rs` for the model builders and the App-bound
  painting entry points (`queue`, `tv_wide`, `music_wide`, `detail`,
  `chrome_status`, `chrome_player_context`, `card`, `visualizer`,
  `list_context`). `state/projection.rs` declares them. This is "the shell
  projects owned presentation models" from AGENTS.md, given a home.
- `state/album_cursor.rs` for `screens/album_cursor.rs` (cursor movement).
- `dispatch/settings.rs` for `render/components/settings.rs` (settings
  activation).

What stays in render after the move is the free painting functions. A render
file left with nothing but code that only the moved block calls moves as a
whole (`visualizer.rs` is the known case). Tests inside a moved block, and
`render/fixtures.rs`, move to the TUI next to the code they exercise
(`render/fixtures.rs` → `src/app/tests/render_fixtures.rs`).

The alternative was one `src/app/projection/` top-level module. It was
rejected because these methods read and mutate `App` state exactly as the
rest of `state/` does. A new top-level module would add a sibling that
`state/` and `dispatch/` both reach into.

### D4: Painters take typed content, not component types

Row and paint types that painters read move from `components/` into
`render/components/` next to the painter that reads them (for example
`media_list::{MediaListRow, …}` → `render/components/media_list.rs`,
`tree_browser::{TreePaintRow, …}` → `render/components/tree_browser.rs`, and
`settings::{ServiceRow, SettingsRow, SetupDraft}` →
`render/components/settings_component.rs`). Components import them from
render, which is the direction the architecture diagram already draws.

`render_wide_media_list_component` and `render_three_line_flat_list` take a
whole component today. Each gets a `…PaintInput<'a>` struct in render that
holds borrows of exactly the fields the painter reads, and the component builds
it in `view()`. No painter output changes.

The alternative was to move those two painters into `components/`. That
would put painting code in the interactive tree, which the mbv-frontend skill
forbids (one painter per surface, and painters live in `render/components`).

### D5: Placement rule for everything else

For any item a cut has to move and that D2–D4 do not name: it goes to the
lowest crate in the D2 chain that can hold everything it references. A
state type that names a render item goes where that item goes: the
`wide_hero` pane-width constants move into `ui_model` together with
`normalize_list_pane_width`, or `normalize_list_pane_width` moves into
`render/arrangements/wide_hero.rs`. Take whichever needs no upward edge, and
prefer the one that keeps the function next to its constants.
`normalize_queue_column_width` follows the same rule into
`render/arrangements/queue.rs`. If no crate in the chain works without an
upward edge, stop and ask. Do not add a trait or generic to get around it.

### D6: Images split at the `impl App` line

App-free code becomes `mbv-images`: `cache.rs`, the cache-key functions and
constants, `CachedImage`, `ImageFetchReq`, `ImageSource`, `cover_fill_hero_box`,
`composite_landscape_logo`, the `impl ImageCache` half of `protocol.rs`,
`infra/resize.rs`, and `RENDER_FILTER` (the `ratatui_image` resize filter,
which moves here from `render/components/widgets.rs`). It depends on
`mbv-theme` for the loading-placeholder fill. The `impl App` code stays in the
TUI as `infra/image_fetch.rs` + `infra/image_fetch/`, made from `images.rs`'s
App blocks, the App half of `protocol.rs`, `fetch.rs`, and `fetch/*`.

### D7: Small leftovers

- `ui_util::service_state_color` → `render/components/widgets.rs`. It picks
  a palette colour, so it is a painting decision. The rest of `ui_util` goes
  to `ui_model::ui_util`, and the module name is kept so the path rewrite
  stays mechanical.
- `infra/layout.rs` → `mbv-render` as `layout`, except `PAGE_SIZE` and
  `PREFETCH_AHEAD`, which move to a new `src/app/infra/paging.rs`.
- `crate::app::palette` (facade) and `render.rs`'s theme re-exports are
  deleted. All callers use `mbv_theme::…`.
- Test helpers: `make_item` → `mbv_emby_model::test_support::make_item` and
  `make_session` → `mbv_emby::test_support::make_session`, each gated
  `#[cfg(any(test, feature = "test"))]`. The TUI's `app::tests` stops
  defining them, and its callers switch paths.

### D8: Crate roots and in-crate paths

`render.rs` → `crates/mbv-render/src/lib.rs` and `render/*` →
`crates/mbv-render/src/*`. The same applies to `components` →
`mbv-components`, `ui_model` → `mbv-ui-model`, `ui_msg` → `mbv-ui-msg`,
`render/theme` → `mbv-theme`, and `infra/images` → `mbv-images`. Inside each
crate, `crate::app::<module>::` rewrites to `crate::`. Visibility: in a moved
file `pub(in crate::app)` and `pub(in crate::app::<module>…)` first become
`pub(crate)`. They are widened to `pub` only where the compiler reports a
use from a higher crate. This keeps the pedantic `must_use_candidate` /
`missing_*_doc` churn to the items that actually cross.

### D9: Test gating follows Tier 3–4 D7

A crate whose code has `#[cfg(any(test, feature = "test"))]` items gets
`[features] test = […]`, forwarding to the lower crates' `test` features it
needs. Consumers enable it in `[dev-dependencies]`.

### D10: One change, commit-sized groups

Group 1's cuts are several commits, and each leaves `mbv` green. Every
extraction group is one commit. `mbv-components` (~31k lines) is one group,
because the path rewrite only compiles once the whole module has moved.

## Risks / Trade-offs

- **Tier 3–4 drift.** PR #826 is open. → Group 0 verifies that the paths
  it leaves behind exist, and stops if they do not.
- **Placement disputes.** D5 is a rule, not an item list, and a handful of
  items may fit two crates. → The rule picks the lowest crate that works, and
  asking is required only when none works.
- **Visibility widening.** Far more items cross than in Tier 3–4. → D8
  widens only what the compiler asks for. Lints are fixed at the source, never
  with `allow`.
- **Concurrent `src/app/` work.** Any in-flight TUI change collides with the
  move. → Rebase before each group. The groups are pure moves, so conflicts
  are path conflicts.
- **Paint-input structs (D4)** add two small types. → That is the price of
  making `render → components` impossible, and the painter bodies are
  unchanged apart from field access.

## Migration Plan

Nothing on disk, on the wire, or on screen changes. To roll back a group,
`git revert` its commit. The spec delta syncs to
`openspec/specs/interactive-component-framework/spec.md` when the change is
archived.
