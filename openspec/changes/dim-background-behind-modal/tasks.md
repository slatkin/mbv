## 1. Shared dim helper

- [ ] 1.1 Add `src/app/render/overlays/backdrop.rs` with a `render_backdrop_dim(&self, f: &mut Frame)` (or similarly named) method on `App` that darkens every cell's `Color::Rgb` fg/bg across `f.area()` in `f.buffer_mut()` by a fixed blend factor, leaving non-`Rgb` colors untouched.
- [ ] 1.2 Register the new `backdrop` module in `src/app/render/overlays/mod.rs`.

## 2. Wire dim call into each blocking modal

- [ ] 2.1 Call the dim helper at the start of `render_confirm_modal` in `src/app/render/overlays/confirm_modal.rs`, before the existing `Clear`/`Block` drawing.
- [ ] 2.2 Call the dim helper at the start of `render_save_playlist_dialog` in `src/app/render/overlays/playlists.rs`.
- [ ] 2.3 Call the dim helper at the start of `render_multiselect_popup` in `src/app/render/overlays/multiselect.rs`.
- [ ] 2.4 Call the dim helper at the start of `render_library_routes_popup` in `src/app/render/overlays/library_routes.rs`.

## 3. Verify

- [ ] 3.1 Run the app (via the `run` skill or `cargo run`) and visually confirm each of the four modals dims the background behind it and the modal itself stays at full brightness.
- [ ] 3.2 Confirm docked panels (sessions, playlists, help, settings) and the context menu render unchanged (no dim) when opened without a blocking modal.
- [ ] 3.3 `cargo fmt --all -- --check` and `cargo build` pass.
