## 1. Foundations (ADR + ledger seam)

- [ ] 1.1 Write `docs/adr/0024-mouse-events-through-component-subscriptions.md` recording D1–D3 (any-position subscription + component hit-test, shell fold with fixed priority, per-component gesture recognition) as Accepted; verify it cross-links ADR 0022/0023 and `openspec validate restore-mouse-support` still passes.
- [ ] 1.2 Replace the "Mouse ownership is out of scope" section of `docs/architecture/interactive-surface-ledger.md` with a "Mouse owner / gestures / verification" column stub on every row (value `pending` for all); verify the table still renders and `rtk make check-code-file-lines` passes for the doc.

## 2. Phase 1 — Delivery spine

- [ ] 2.1 Add `mouse_sub()` helper returning the any-position `EventClause::Mouse` + `SubClause::Always` subscription; unit-test that its clause forwards a `MouseEvent` at arbitrary coordinates.
- [ ] 2.2 Attach `mouse_sub()` at every interactive-component mount site in `src/app/shell.rs` and the lazy mount paths (`mount_home`, `mount_feeds`, `reconcile_destination_mounts`, overlay/popup mounts, `Playback`); verify with a test that a mounted-but-unfocused component receives an injected `Event::Mouse` through `tick()`.
- [ ] 2.3 Add the mouse-message fold to `src/app/shell_run.rs` beside the keyboard router fold: collect mouse-derived messages from a `tick()`, keep at most one by priority (topmost overlay/modal > active panel > other panel > chrome), discard all underlying mouse messages when a blocking overlay is mounted; unit-test each priority branch against a hand-built message list.
- [ ] 2.4 Retire `TerminalObserverEvent::Mouse`: remove the drop arm in `shell.rs`, the `root.rs` mapping, and the enum variant; verify `rtk cargo check -p mbv` passes and no `match` regresses to a wildcard.
- [ ] 2.5 Route the folded mouse message through the existing `handle_terminal_message` dispatch so the five landed surfaces + `PlaybackComponent` act from any focus state; verify by an integration test: focus Queue, click a Library row → Library focuses and the row selects.
- [ ] 2.6 Verify `PlaybackComponent` seekbar + transport clicks work while another component holds focus (integration test through `tick()` asserting the emitted `Msg::Playback`).
- [ ] 2.7 Phase-1 gate: `rtk cargo nextest run -p mbv`, `rtk cargo clippy --workspace --all-targets`, `rtk cargo fmt --check`; no new gestures introduced — the diff only changes delivery.

## 3. Phase 2 — Shared primitives

- [ ] 3.1 Create `src/app/components/mouse/hit.rs` with `HitRegions<Tag>` (`clear`, `push(rect, tag)`, `resolve(point) -> Option<Tag>`, last-push-wins); unit-test overlap resolution and empty/out-of-bounds cases.
- [ ] 3.2 Create `src/app/components/mouse/gesture.rs` with `MouseGestureState` consuming raw `MouseEvent`s and emitting `Click`/`DoubleClick`/`RightClick`/`Scroll` (reserve `DragStart`/`DragMove`/`DragEnd`/`HoverEnter`/`HoverLeave`); unit-test the double-click window and wheel throttle in isolation.
- [ ] 3.3 Migrate `BrowserComponent` onto `HitRegions<BrowserHitRegion>` + `MouseGestureState`; verify `rtk cargo nextest run -p mbv emby_browser` stays green with characterization buffers unchanged.
- [ ] 3.4 Migrate `HomeComponent` and `QueueComponent` onto the primitives; verify `rtk cargo nextest run -p mbv home queue` green.
- [ ] 3.5 Migrate `TvWorkspaceComponent` and the partial `MusicWorkspaceComponent` mouse paths onto the primitives; verify `rtk cargo nextest run -p mbv tv_workspace music_workspace` green.
- [ ] 3.6 Delete the shell-side recognition glue (`note_browse_double_click`, `note_browse_scroll`, `App.last_click_time`/`last_click_pos`/`last_scroll_at` if now unused) and the "shell decides single vs double" contract text in `msg/hit_regions.rs`; verify `rtk cargo clippy --workspace --all-targets` reports no dead code and `rtk cargo check -p mbv` passes.
- [ ] 3.7 Phase-2 gate: full `rtk cargo nextest run -p mbv` green with zero characterization-buffer changes; confirm this phase added no observable behaviour.

## 4. Phase 3 — Main-surface parity + wheel

- [ ] 4.1 Replace the stubbed `handle_mouse_scroll_browse` with real per-component wheel routing for Emby, ABS, and Feeds browse lists, mirroring `Model::handle_home_scroll`'s throttle/readiness gates; verify each list scrolls under the wheel via component tests.
- [ ] 4.2 Ensure click-to-focus, click-to-select, and double-click-to-activate are wired for every main panel (`browser`, `home`, `queue`, `tv_workspace`, `music_workspace`); verify per-surface integration tests through `tick()`.
- [ ] 4.3 Wire right-click → context menu on every main panel that paints selectable rows, anchored at the click position; verify against the `context-menu` "Right-click parity across migrated surfaces" scenario.
- [ ] 4.4 Update the ledger Mouse column for the five main-panel rows with owner, supported gestures, and the test names that verify them.
- [ ] 4.5 Phase-3 gate: `rtk cargo nextest run -p mbv`, clippy, fmt.

## 5. Phase 4 — Overlays & popups

- [ ] 5.1 Add `Event::Mouse` handling (via the primitives) to `feeds_manage`, `library_routes`, `multiselect`, `save_playlist`, and `search_sidebar`: click-to-select and click-to-dismiss where a keyboard equivalent exists; verify per-component tests.
- [ ] 5.2 Confirm `confirm`, `daemon_lost`, and `remote_reanchor` blocking modals swallow all mouse events and never let a click reach obscured content; verify a `tick()`-level test that a click outside the modal produces no underlying message.
- [ ] 5.3 Verify `context_menu` and `selection_modal` mouse behaviour matches their specs (menu-click executes/closes, outside-click dismisses, wheel does not mutate the obscured view); run `rtk cargo nextest run -p mbv context_menu selection_modal`.
- [ ] 5.4 Update the ledger Mouse column for every overlay/popup/modal row.
- [ ] 5.5 Phase-4 gate: `rtk cargo nextest run -p mbv`, clippy, fmt.

## 6. Phase 5 — music_workspace completion + narrow browse

- [ ] 6.1 Complete `MusicWorkspaceComponent` hit geometry for the narrow branch and the wide right-rail track table (the D16-partial areas); verify component TestBackend tests cover album rows, track rows, and group pills under the pointer.
- [ ] 6.2 Add row hit-testing to `browser_narrow.rs` so narrow generic/Movies/home-video/TV/Music bodies resolve clicks; verify narrow-width integration tests.
- [ ] 6.3 Update the ledger Mouse column for the music-workspace and narrow-browse rows; confirm no row still reads `pending`.
- [ ] 6.4 Phase-5 gate: `rtk cargo nextest run -p mbv`, clippy, fmt.

## 7. Phase 6 — Precedence proofs + close-out

- [ ] 7.1 Land the "simultaneous Queue + Library" precedence proof: both visible, no overlay, a click on each resolves to the painting component through the real `tick()` synchronisation order.
- [ ] 7.2 Land the "blocking overlay swallows mouse" precedence proof through `tick()`.
- [ ] 7.3 Land the "geometry cannot drift" precedence proof: a component resolves a click from the same `HitRegions` it populated during `view()`.
- [ ] 7.4 Verify the three modified capability deltas match the implementation: no code path uses a global hit map / global mouse router; the mouse-emitted `ShellRequest` variants have real handlers (no "mouse-only" no-op arms remain); `rtk ast-grep scan` clean.
- [ ] 7.5 Final close-out: `rtk cargo nextest run -p mbv` full suite, `rtk cargo clippy --workspace --all-targets`, `rtk cargo fmt --check`, `rtk make check-code-file-lines`; confirm every ledger row has a filled Mouse column and `openspec validate restore-mouse-support --strict` passes.
