# Tasks

Read `design.md` before starting any group. Naming pitfall: the old variant `Iris` (`#a7c080`)
becomes slot `Green`, and the old variant `Green` (`#93b259`) becomes slot `GreenDeep`. Always
map by hex (design D1), never by name. Every group ends with `cargo fmt` and a commit.

## 1. Theme core: slots, theme, roles, surfaces

- [ ] 1.1 In `crates/mbv-theme/src/`, replace `palette.rs` with `slot.rs` holding the crate-private `Slot` enum (19 variants, design D1, with a `#[cfg(test)] ALL` list), the crate-private `Theme` value with `const DEFAULT` holding today's exact hexes and `const fn get(&self, Slot) -> Color`, and `pub(crate) fn active() -> &'static Theme` returning `&Theme::DEFAULT` (design D2). Verify: `cargo check -p mbv-theme`.
- [ ] 1.2 Add the public `Role` enum (design D3): one variant per surviving role constant, named by converting the constant to `UpperCamelCase` (40 variants: every current `pub const ...: Color` except the 15 fill roles in design D5), each mapped to the slot of its current `Palette` variant by hex. Add `Role::color()`, crate-private `Role::color_in(&Theme)`, a `#[cfg(test)] ALL` list, and `HERO_META_ROLES: [Role; 3]`. Keep `bar_role_fg`. Verify: `cargo check -p mbv-theme`.
- [ ] 1.3 Rewrite the Surface table (design D4): `surface.rs` keeps `declare_surfaces!` and `Surface`; `surface_table.rs` becomes one `const fn slots(Surface) -> (Slot, Slot)` match giving (resting, focused) per row exactly as in the D4 table; `surface_resolve.rs` keeps `surface_colors`'s signature and resolves through `active()`. Add the new rows (`SelectedRow`, `ListStripe`, `WorkspaceStripe`, `CreditsStripe`, `PopupBorder`, `ModalButton`, `TransportRow`, `HintChip0..2`) and `pub const HINT_CHIPS: [Surface; 3]`. Remove `ContextMenuSelectedRow` and switch `crates/mbv-render/src/components/context_menu.rs` to `Surface::SelectedRow`. Delete `Level`, `FocusSource`, `Row`, `soft`, `RESTING_DEVIATIONS`, `Surface::level()`, the row helper functions and the resolver's `debug_assert`. Verify: `cargo check -p mbv-theme -p mbv-render`.
- [ ] 1.4 Keep the workspace building: in `lib.rs`, redefine every old `pub const NAME: Color` (all 55, fill roles included) as a transitional alias `= Theme::DEFAULT.get(Slot::X)` with the same value, and `HINT_PILL_FILLS` likewise. Mark the block with a one-line comment saying task 7.1 deletes it. Verify: `cargo check --workspace --all-targets` passes with no consumer edits beyond 1.3's `context_menu.rs`.
- [ ] 1.5 Add one test in `crates/mbv-theme` owning the spec scenario "A theme is swapped": build a test `Theme` with a distinct value per slot, and assert that `Role::color_in` returns that theme's value for one role, and that resolving a focus-reactive surface against it returns the expected (resting, focused) pair. Verify: `cargo nextest run -p mbv-theme`.
- [ ] 1.6 Rewrite `palette_json.rs` to iterate `Slot::ALL`, `Role::ALL`, `HERO_META_ROLES`, `HINT_CHIPS` and `Surface::ALL` with no source-text parsing (design D6). Rename `docs/palette.json`'s keys (`variants` -> `slots`, role `variant` -> `slot`, surfaces drop `level`/`soft`) and its prose names (constant case -> `UpperCamelCase`) by hand, combining the prose of merged identities. Update `docs/palette.html` to read the renamed keys. Verify: `cargo nextest run -p mbv-theme` regenerates the file, and comparing the old and new `docs/palette.json` (git diff) shows every surviving role's hex and every pre-existing surface's resting/focused hex unchanged, and each new surface's hexes equal the replaced role's (design D7.1).
- [ ] 1.7 Update the docs that describe the old model: rewrite `docs/invariants/07-colour-value-sharing-is-deliberate.md` for the slot model (two roles on one slot are still independent concepts, and collapsing different concepts is still the failure it records; one concept now takes one identity, per the `ui-design-language` delta), update the `crates/mbv-theme/` row and the "What the compiler does not catch" paragraph in `docs/architecture/tui-frontend.md`, and change `Palette::Ink` to slot `BgDim` in `CONTEXT.md`'s Wide hero sheet entry. Verify: `rg -n "Palette::|RESTING_DEVIATIONS|focused_fill" docs/invariants docs/architecture CONTEXT.md` finds nothing.
- [ ] 1.8 Run `cargo fmt`, `cargo clippy -p mbv-theme --all-targets -- -D warnings`, and commit group 1.

## 2. Fill folds in mbv-render

- [ ] 2.1 Selection bar (design D5): `media_list/wide.rs` `selected_row_surface_color` returns `surface_colors(Surface::SelectedRow, false).fill`. `tree_browser.rs`, `playlists.rs` and `three_line_flat_list.rs` use the same instead of `SELECTED_ROW_BG`. In `media_list/row.rs`, delete the colour comparison: `selected_row_foreground` always returns `Role::SelectedRowFg.color()` and drops its `selected_bg` parameter. Verify: `cargo nextest run -p mbv-render`.
- [ ] 2.2 List stripes: `playlists.rs`, `settings_component.rs` and `three_line_flat_list.rs` paint `Surface::ListStripe` instead of the playlist, settings and sessions stripe roles. Verify: `cargo nextest run -p mbv-render`.
- [ ] 2.3 Change `ZebraStripe` in `media_list.rs` from a `Color` pair to a `Surface`, with `zebra_bg()` returning `surface_colors(surface, self.palette_focused).fill`, and `queue_row_zebra_stripe()` returning `Surface::QueueColumn` (design D5). Verify: `cargo check -p mbv-render` (mbv-components callers are fixed in 3.2).
- [ ] 2.4 Remaining fills: `modal_frame.rs:65` -> `Surface::PopupBorder`; `confirm_modal.rs:108` -> `Surface::ModalButton`; `chrome_player/title/queue_band.rs` (row background and seek gauge background) -> `Surface::TransportRow`; `widgets.rs` hint chips -> `HINT_CHIPS`. Rename any test assertion in these files to the replacing surface without changing its expected value. Verify: `cargo nextest run -p mbv-render`, and `rg -n "SELECTED_ROW_BG|_STRIPE_BG|SURFACE_(RESTING|BACKDROP|CHROME)|HINT_PILL_FILLS" crates/mbv-render` finds nothing.
- [ ] 2.5 Run `cargo fmt` and commit group 2.

## 3. Fill folds in mbv-components

- [ ] 3.1 `library_panel/hero_composition.rs:190`: replace the hand-rolled focus switch with `surface_colors(Surface::MainContentBox, box_focused).fill`. `library_panel/overview_box.rs:311`: paint `Surface::CreditsStripe`. Verify: `cargo check -p mbv-components`.
- [ ] 3.2 `library_panel/panel_list.rs`: the `WideWorkspace` policy uses `ZebraStripe` over `Surface::WorkspaceStripe` in both focus states (design D5 explains the equivalence). Rename its tests' expected colours to `surface_colors(Surface::WorkspaceStripe, ..)` and `Surface::SelectedRow`, along with `queue/tests.rs`, keeping every expected value. Verify: `cargo nextest run -p mbv-components`, and `rg -n "SELECTED_ROW_BG|WORKSPACE_FOCUSED|SURFACE_RESTING|HERO_CREDITS_STRIPE" crates/mbv-components` finds nothing.
- [ ] 3.3 Run `cargo fmt` and commit group 3.

## 4. Role call sites in mbv-render, part A

- [ ] 4.1 Convert every `palette::ROLE_NAME` reference to `palette::Role::RoleName.color()` (or `Role::RoleName.color()` with a `use`) in `crates/mbv-render/src/components/`: `chrome_player.rs`, `chrome_player/title/*.rs`, `chrome.rs`, `chrome_status_bar.rs`, `chrome_tabs.rs`, `chrome_tabs/tests.rs`, `indicators.rs`, `media_list/row.rs`, `media_list/wide.rs`, `tree_browser.rs`, `widgets.rs`. Test assertions keep their expected roles. Verify: `cargo nextest run -p mbv-render`, and `rg -n "palette::[A-Z][A-Z_]{2,}" <those files>` finds nothing.
- [ ] 4.2 Run `cargo fmt` and commit group 4.

## 5. Role call sites in mbv-render, part B

- [ ] 5.1 Same conversion for every remaining `crates/mbv-render` file with a role reference (`confirm_modal.rs`, `context_menu.rs`, `daemon_lost_modal.rs`, `feeds_manage.rs`, `help.rs`, `hero.rs`, `library_routes.rs`, `modal_frame.rs`, `multiselect.rs`, `playlists.rs`, `queue_playback.rs`, `queue.rs`, `search_sidebar.rs`, `sessions.rs`, `settings_component.rs`, `three_line_flat_list.rs`, plus `benches/`). Verify: `cargo nextest run -p mbv-render`, `cargo check -p mbv-render --benches`, and `rg -n "palette::[A-Z][A-Z_]{2,}|mbv_theme::[A-Z][A-Z_]{2,}" crates/mbv-render` finds nothing.
- [ ] 5.2 Run `cargo fmt` and commit group 5.

## 6. Role call sites in mbv-components and the mbv binary

- [ ] 6.1 Same conversion in `crates/mbv-components` (`hero_header/title_meta.rs`, `overview_box.rs`, `panel_list.rs`, `library_playback_panel.rs`, `sessions.rs`, `status_bar_panel.rs` and their tests), including `HERO_META_ROLES` callers resolving with `.color()`. Verify: `cargo nextest run -p mbv-components`, and `rg -n "palette::[A-Z][A-Z_]{2,}" crates/mbv-components` finds nothing.
- [ ] 6.2 Same conversion in `src/` (`state/projection/chrome_status.rs`, `state/projection/visualizer.rs`, `shell/playback.rs`, `shell/overlays/sidebars.rs`, `infra/image_fetch/protocol.rs`, `tests/panel_focus.rs`, `tests/tick_integration/queue_playback.rs`). Verify: `cargo nextest run -p mbv`, and `rg -n "palette::[A-Z][A-Z_]{2,}" src` finds nothing.
- [ ] 6.3 Run `cargo fmt` and commit group 6.

## 7. Remove the transitional aliases

- [ ] 7.1 Delete the transitional alias block from `crates/mbv-theme/src/lib.rs` (task 1.4) and update the crate doc comment to describe slots, theme, roles and surfaces. Verify: `cargo clippy --workspace --all-targets -- -D warnings` passes, which proves no consumer still names an old constant.
- [ ] 7.2 Integration check: run `cargo nextest run --workspace` and `cargo fmt --all -- --check`. Confirm `docs/palette.json` did not change after the run, so the theme is the same as at 1.6. Commit.

## Workflow follow-up

- Sync the `ui-design-system` and `ui-design-language` deltas into `openspec/specs/` and archive the change once it is reviewed.
- The theming change (theme selection, `Color::White` specials, pinwin accent default, mpv Lua colours) builds on `active()` and is proposed separately.
