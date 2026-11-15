# Tasks

## 1. SectionRef and the four signals

- [x] 1.1 Add `SectionRef<T>` in `crates/mbv-gui/src/window/sections/shared.rs` (`mod shared;` in `sections.rs`): `Rc<T>` newtype, `new`, `Clone`, `Deref` only, `PartialEq` by `Rc::ptr_eq`, `Debug`. Verify: `cargo check -p mbv-gui`.
- [x] 1.2 Change the `library_list`, `library_detail`, `queue`, and `settings` signals in `Sections` to `SectionRef` (design D2). Make `write` build one `SectionRef` per written section; wrap `fresh_queue()`. Verify: `cargo check -p mbv-gui` compiles `sections.rs` (readers may still fail).
- [x] 1.3 Add one test in `window/sections/tests.rs`: after a batch that names only another section, the `queue` handle is `ptr_eq` to the handle held before it; a batch with a new `queue` revision gives a handle that is not `ptr_eq`. Contract: the spec requirement "A repaint copies no section content". Verify: `cargo nextest run -p mbv-gui sections`.

## 2. Readers outside components

- [x] 2.1 Fix the `peek()` and `read()` readers in `window/app.rs`, `window/library/**`, and `window/settings/state.rs` (use `as_deref()` where an `Option<&T>` is needed). Verify: `cargo check -p mbv-gui` shows no error in these paths.

## 3. Library components

- [x] 3.1 `library_panel.rs`: borrow through `as_deref()` for `library_pair`; pass the detail handle to the Hero painters; remove `detail.clone()` and `list.rows.clone()` (design D3). Keep `tabs` and `pills` cloned. Change `LibraryStatusBar` (`library_status_bar.rs`) so its `list` and `detail` props take `Option<SectionRef<..>>`, and pass the handles at its call site in `library_panel.rs`. Verify: `cargo check -p mbv-gui`, and `rg "as_deref\(\)\.cloned\(\)" crates/mbv-gui/src/window/components` returns no section read.
- [x] 3.2 The seven Hero components (`library_hero`, `library_home_hero`, `library_shows_hero`, `library_emby_hero`, `library_audiobooks_hero`, `library_podcasts_hero`, `library_feeds_hero`): take `SectionRef<GuiLibraryDetail>` (Feeds also the list handle) and narrow inside the body. Verify: `cargo nextest run -p mbv-gui library_` passes.
- [x] 3.3 The destination lists (`library_list`, `library_shows_list`, `library_emby_list`, `library_audiobooks_list`, `library_feeds_list`, `library_podcasts_list`, `library_home_list`): narrow with `as_deref()` on the handle clones; no deep clone of rows. Fix their `tests.rs` fixtures with `SectionRef::new`. Verify: `cargo nextest run -p mbv-gui library_` passes.

## 4. Queue and Settings

- [x] 4.1 `queue_panel.rs`: the render body and the `use_effect` clone the `SectionRef`. Fix `queue_panel/tests/**` fixtures. Verify: `cargo nextest run -p mbv-gui queue_panel`.
- [x] 4.2 `settings_screen.rs`: `SettingsSectionView` holds a `SectionRef<GuiSettingsFrame>`; fix the `settings_*` test fixtures. Verify: `cargo nextest run -p mbv-gui settings`.

## 5. Docs and gates

- [x] 5.1 Add one sentence to `docs/architecture/gui-client.md` (section-store paragraph): the four large sections are held as `SectionRef`, one copy per update. Verify: the sentence exists and names `SectionRef`.
- [x] 5.2 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run -p mbv-gui`, then `make check-code-file-lines`. Verify: all four pass; fix any red gate.
