## 1. Introduce the type

- [x] 1.1 Add `enum TabSelection { Home, Library(usize) }` (index into `libs`) in a small module (e.g. `src/app/types_tab_selection.rs`). Derive `Debug, Clone, Copy, PartialEq, Eq`.
- [x] 1.2 Add methods: `is_home(&self) -> bool`, `library_index(&self) -> Option<usize>`, and private `from_position(pos: usize) -> Self` / `to_position(&self) -> usize` (`0 = Home`, `1.. = Library(pos-1)`).
- [x] 1.3 Round-trip unit test: `from_position(t.to_position()) == t` for `Home` and several `Library(i)`.

## 2. Swap the field

- [x] 2.1 Change `library_tab: usize` → `tab: TabSelection` in `app_struct.rs` (default `Home` in `construct.rs`).
- [x] 2.2 Reimplement `set_library_tab`, `library_tab_next/prev`, `library_tab_count`, and the `render/mod.rs` pending→committed apply on `TabSelection` via `from_position`/`to_position`. Keep their public signatures.

## 3. Migrate read sites

- [x] 3.1 Replace `library_tab - 1` / `library_tab.checked_sub(1)` (46 sites) with `self.library_index()`.
- [x] 3.2 Replace `library_tab > 0` (29 sites) with `self.library_index().is_some()`.
- [x] 3.3 Replace `library_tab == 0` (19 sites) with `self.tab.is_home()`.
- [x] 3.4 Migrate tests that set `app.library_tab = 1` to the selection setter (or a test helper); do not expose the raw field.
- [x] 3.5 Let the compiler drive: after 2.1 every remaining old `usize` use is a type error — fix each by routing through a `TabSelection` method, not by re-adding arithmetic.

## 4. Verify

- [x] 4.1 `cargo test -p mbv-core` and the `src/` app tests green — behavior unchanged (tab select, cycling, routing, render).
- [x] 4.2 `cargo clippy --workspace --all-targets` green; no residual `library_tab` arithmetic remains (`rtk grep -rn "library_tab" src/app` shows only the type/helpers, not index math).
- [x] 4.3 `make check-code-file-lines` passes.
- [x] 4.4 Diff review: every changed line is a mechanical type migration; no reordering, no cycling change, no `Feeds` variant.
