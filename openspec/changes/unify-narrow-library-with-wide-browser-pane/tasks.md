## 1. Extract the shared browser-pane painter

- [ ] 1.1 In `src/app/components/library_panel/wide.rs`, extract the Wide browser pane's paint block (Selector row, optional List controls row, list-box `fill_surface`, list view, `WideSkeletonGeometry` role rects) into one `paint_browser_pane(f, pane: WideHeroBrowserPane, content, list_focused, hovered_selector, hits, windows)`; have `render_wide_skeleton` call it unchanged. Verify `cargo nextest run -p mbv library_panel` and the Wide characterization suites still pass with no expected-pixel edits.
- [ ] 1.2 Confirm the extraction changed no Wide behaviour by running the surface-conformance and Wide-hero tests: `cargo nextest run -p mbv surface_conformance wide_hero`.

## 2. Make the non-Wide panel the browser pane

- [ ] 2.1 Rewrite `render_narrow_skeleton` in `src/app/components/library_panel/narrow.rs` to build the pane with `pill_bar_areas(area)` and delegate to `paint_browser_pane`; delete the bespoke Selector/controls/list/`set_paint_policy`/`set_geometry` body. Verify `narrow_tests.rs` passes (the `list_area.y == controls.bottom() + PANE_PAD_Y` assertion must still hold) and no `PANE_PAD_X`-specific narrow branch remains.
- [ ] 2.2 Return `WideSkeletonGeometry` from the non-Wide path, delete `NarrowSkeletonGeometry`, update the `mod.rs` re-export and `panel.rs`'s `narrow_geometry` field type. Verify `cargo check -p mbv` and `cargo nextest run -p mbv library_panel`.
- [ ] 2.3 Confirm the non-Wide list now paints with the `Wide` policy (no body/scrollbar override) and add/adjust a component test asserting the non-Wide list box fill and stripe pair in both focus states. Verify `cargo nextest run -p mbv panel_list narrow`.

## 3. Retire the non-Wide paint seams

- [ ] 3.1 Delete `PanelListPaintPolicy::Narrow` (`components/library_panel/content.rs`, `panel_list.rs`) and its arm. Verify `cargo nextest run -p mbv library_panel`.
- [ ] 3.2 Delete `WideMediaListPaintPolicy::{body, scrollbar}` with `with_body`/`with_scrollbar`/`body_bg`/`scrollbar_bg` (`components/media_list/mod.rs`) and the `body_bg`/`scrollbar_bg` parameters with the body-fill block and scrollbar override in `render/components/media_list/wide.rs`; update the `render_wide_media_list*` test call sites. Verify `cargo nextest run -p mbv media_list`.

## 4. One fixed column backdrop

- [ ] 4.1 Make `Model::library_body_fill` (`src/app/shell_library_panel.rs`) return the `LibraryColumn` fixed backdrop for every geometry and focus state; drop the non-Wide branch and update its doc comment. Verify `cargo nextest run -p mbv shell_library library_panel`.
- [ ] 4.2 Point the non-Wide Selector spacer row at `Surface::PillRowGap` (the Wide pane's own identity) and confirm the status band's padding rows still take `library_body_fill`. Verify `cargo nextest run -p mbv status_bar library_panel`.
- [ ] 4.3 Delete `Surface::NarrowLibraryBody`: its variant, table row, `surface_resolve` pinned fill, and the conformance-coverage residual entry in `src/app/render/tests_surface_conformance.rs`. Verify `openspec validate --all` and `cargo nextest run -p mbv surface`.
- [ ] 4.4 Add a surface-conformance or characterization test asserting a focused non-Wide (and Mini) frame paints `#2d353b` for the panel placement, the Selector spacer, the status band padding rows and the list scrollbar column, identical to the unfocused frame. Verify with `cargo nextest run -p mbv surface_conformance`.

## 5. Preserve the Library Hero overlay gate

- [ ] 5.1 Keep `narrow_geometry.is_some()` as the non-Wide gate in `panel.rs:701` (and `menu_geometry`/`list_rect`/`test_painted_layout`) after the type change; verify by running the existing non-Wide overlay-open tests: `cargo nextest run -p mbv hero_overlay overlay`.
- [ ] 5.2 Add a tick-integration test through the shell sync pass asserting a double-click in the non-Wide browser still opens the Library Hero overlay, and that a Wide double-click does not take that path. Verify `cargo nextest run -p mbv tests_tick_integration`.
- [ ] 5.3 Add a test asserting the overlay's own pixels are unchanged by this change (sheet surface, workspace box pair) in non-Wide geometry. Verify `cargo nextest run -p mbv library_hero_overlay hero_composition`.

## 6. Non-Wide interaction geometry

- [ ] 6.1 Cover the intended saved-geometry shift: a click in the list box's outer two columns and a context-menu open in non-Wide geometry resolve against the Wide pane's rects. Verify `cargo nextest run -p mbv context_menu library_panel`.
- [ ] 6.2 Confirm the non-Wide `list_rect`, selected-row rect and menu anchor agree with the painted inset (ADR 0024: geometry resolved is geometry painted). Verify `cargo nextest run -p mbv library_panel`.

## 7. Docs and gates

- [ ] 7.1 Amend `docs/adr/0028-geometry-scoped-surface-identity.md` Consequences to record that `NarrowLibraryBody` plus `PanelListPaintPolicy::Narrow` were retired once the non-Wide library became the Wide browser pane, so the non-Wide palette no longer exists. Verify the ADR reads coherently against `add-non-wide-library-palette`'s archive.
- [ ] 7.2 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run -p mbv` clean.
- [ ] 7.3 Manual live check: a focused and unfocused Narrow/Mini library shows the list box on a flat `#2d353b` backdrop, identical to the Wide right column, and the Library Hero overlay still opens, focuses and dismisses as before.