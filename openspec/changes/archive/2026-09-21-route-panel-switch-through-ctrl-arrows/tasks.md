# Tasks

## 1. Verify the shipped behavior matches the new requirement

- [x] 1.1 Confirm `panel_left`/`panel_right`'s `default_chords` in
      `crates/mbv-core/src/keybinds.rs` are `["Ctrl+Left"]`/`["Ctrl+Right"]`,
      and `crates/mbv-core/src/keybinds_tests.rs`'s `EXPECTED_DEFAULTS` agrees.
- [x] 1.2 Confirm `src/app/tests_routing_matrix_focus.rs`'s
      `panel_focus_switches_on_the_ctrl_arrows_only` and
      `library_focus_tree_navigation_chords_stay_leaf_local` pass and cover
      both scenarios in the spec delta:
      `cargo nextest run -p mbv tests_routing_matrix_focus`.

## 2. Land the spec

- [x] 2.1 `openspec validate route-panel-switch-through-ctrl-arrows --strict`
      passes.
- [ ] 2.2 Sync the delta into
      `openspec/specs/interactive-component-framework/spec.md` and archive
      the change (`openspec archive route-panel-switch-through-ctrl-arrows`).
