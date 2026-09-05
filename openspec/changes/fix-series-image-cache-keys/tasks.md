## 1. Centralize the Series artwork cache key

- [x] 1.1 Add `series_image_cache_key(item_id, image_types)` beside the other cache-key constructors in `src/app/images.rs` and verify a unit test pins both the `Primary` and `Thumb,Primary,Backdrop,Logo` spellings.
- [x] 1.2 Adopt the helper at the paint site (`paint_home_image`'s `Series` arm in `src/app/render/components/home_hero_emby.rs`) with no behavior change, and verify `cargo nextest run -p mbv` stays green for the hero paint tests.

## 2. Point the shell prefetch and loading state at the painted keys

- [x] 2.1 Export one canonical Thumb-first chain constant from `src/app/render/components/hero_model.rs` (used by the `artwork_for` landscape mapping, no new `&["Thumb", ...]` literal anywhere) and update `push_tv_workspace_content` in `src/app/shell_tv_workspace.rs` to prefetch with that constant under the helper key and to derive `image_loading` from the same key; verify with a regression test that pushes Wide content, consumes its `HomeImagePaint`, calls `paint_home_image`, and asserts the same cache key stays reserved with no additional active/pending fetch, plus `push_tv_workspace_projects_uncached_and_cached_series_image_state` passes with its fixture key updated to the helper spelling.
- [x] 2.2 Update the narrow `image_loading` lookup in `src/app/render/components/list_narrow.rs` to the helper key with `&["Primary"]`, matching what `detail_series_view.rs` paints; verify a narrow Series selection shows the placeholder while uncached and the painted entry once cached.
- [x] 2.3 Widen the `tv_image_changed` gate in `src/app/shell_run.rs` from the `ends_with(":ser_primary")` match to the `:ser:` family; verify a completion under the Thumb-first key re-pushes TV workspace content while other image keys do not.

## 3. Prove no stale key reference remains and the full gates pass

- [ ] 3.1 Remove the stale `ser_primary` fixture key in `src/app/shell_tv_workspace_tests.rs`, search the tree for remaining `ser_primary` / inline `ser:` key formats outside the helper and its tests, and verify none remain.
- [ ] 3.2 Run `cargo fmt`, `cargo check -p mbv`, `cargo nextest run -p mbv`, `cargo clippy --workspace --all-targets`, `make check-code-file-lines`, and `ast-grep scan`; verify all are clean and the mbv-frontend completion checklist (render boundary, narrow-width behavior, component boundary, buffer tests) holds.
- [ ] 3.3 Run `openspec validate "fix-series-image-cache-keys" --strict` and verify the change validates.
