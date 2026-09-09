## 1. Characterize grouped-album ownership

- [ ] 1.1 Add or update focused component/media-list tests proving stable-target ordinary refresh, local clamp, explicit re-anchor, and `ViewportAnchor` handoff across grouped heading/spacer rows; verify the targeted tests pass.
- [ ] 1.2 Add or update shell-tick integration coverage at Wide and Normal/Narrow proving retained-control album hits claim painted rows only while parent pill and Music track-workspace behavior remains owned by Music; verify the targeted tests pass through `Application::tick()`.

## 2. Transfer album-list authority

- [ ] 2.1 Refactor `MusicWorkspaceComponent` so the active persistent album control solely owns live cursor and scroll, ordinary pushes preserve local selection, and only a breakpoint transition transfers one `ViewportAnchor`; verify component tests cover movement and handoff without inactive-control synchronization.
- [ ] 2.2 Refactor Grouped Music shell projection and Wide/Inline render paths so rendering only configures and paints controls, never reseeds album content/position or writes painter-derived position back; remove parent album row maps and compatibility point resolution; verify retained control geometry resolves album hits and Music-owned tracks/search/pills/images/effects/persistence remain unchanged.

## 3. Verify the bounded repair

- [ ] 3.1 Run relevant Grouped Music component, render, and shell-tick tests at supported breakpoints; verify the base frame does not underpaint the mounted Music surface.
- [ ] 3.2 Run `cargo fmt --all -- --check`, `cargo check -p mbv`, relevant `cargo nextest run -p mbv`, `cargo clippy --workspace --all-targets`, `ast-grep scan`, and `make check-code-file-lines`; record any unrelated failure without widening this change.
