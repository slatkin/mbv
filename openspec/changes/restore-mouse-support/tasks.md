## 1. Foundations (ADR + ledger seam)

- [ ] 1.1 Write `docs/adr/0024-mouse-events-through-component-subscriptions.md` recording D1–D3 (any-position subscription + mounted-parent hit-test, shell fold with fixed priority, mounted-parent gesture recognition) as Accepted; verify it cross-links ADR 0022/0023 and `openspec validate restore-mouse-support` still passes.
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

_This change owns the full mouse migration. It adds `HitRegions<Target>` (field, `view()` population, `resolve(point) -> Option<Target>`) to the already-landed `WideMediaList`/`InlineMediaBrowser`, then for every surface (Queue included) swaps the bespoke `*HitRegion` enum onto that control's `HitRegions<Target>`, deletes the enum, and wires the parent→child point-resolution delegation call. Nothing is deferred to another change. The tasks below migrate each mounted parent onto `MouseGestureState`, perform that per-surface swap, and wire parent-owned non-list chrome. This scope note also governs Phase 3._

- [ ] 3.1 Create `src/app/components/mouse/hit.rs` with `HitRegions<Tag>` (`clear`, `push(rect, tag)`, `resolve(point) -> Option<Tag>`, last-push-wins); unit-test overlap resolution and empty/out-of-bounds cases.
- [ ] 3.2 Create `src/app/components/mouse/gesture.rs` with `MouseGestureState` consuming raw `MouseEvent`s and emitting `Click`/`DoubleClick`/`RightClick`/`Scroll` (reserve `DragStart`/`DragMove`/`DragEnd`/`HoverEnter`/`HoverLeave`); unit-test the double-click window and wheel throttle in isolation.
- [ ] 3.2a Add a `HitRegions<Target>` field, `view()` population, and `resolve(point) -> Option<Target>` to `WideMediaList` and `InlineMediaBrowser`; unit-test point resolution against a populated view and the empty/out-of-bounds cases.
- [ ] 3.3 Migrate the mounted Browser parent onto `MouseGestureState` and migrate the surface's row hit-testing onto the embedded canonical control's `HitRegions<Target>`, delete the `*HitRegion` enum, and wire the parent→child delegation call; wire parent-owned non-list chrome (pills, wheel-for-chrome, right-click recognition + anchor) and parent click-to-focus; verify row click/select/activate/scroll and parent-owned control delivery.
- [ ] 3.4 Migrate the Home and Queue mounted parents onto `MouseGestureState` and migrate their row hit-testing onto the embedded canonical control's `HitRegions<Target>`, delete the `*HitRegion` enums (Queue's `QueueHitRegion` included), and wire the parent→child delegation call; wire parent-owned non-list chrome (including Queue scope buttons and wheel-for-chrome) and parent click-to-focus; verify row click/select/activate/scroll and parent-owned control delivery.
- [ ] 3.5 Migrate the TV and Music mounted parents onto `MouseGestureState`. Migrate TV's row hit-testing onto the embedded canonical control's `HitRegions<Target>` and delete the TV `*HitRegion` enums; migrate Music's Wide right-rail row hits onto `HitRegions<Target>` (Music has no `*HitRegion` enum to delete). Wire the parent→child delegation call, parent-owned pills and non-list chrome, and parent click-to-focus; verify row gestures and parent-owned control delivery. Music's narrow list and track-table row paths are completed in Phase 5 (task 6.1).
- [ ] 3.6 Delete the shell-side recognition glue (`note_browse_double_click`, `note_browse_scroll`, `App.last_click_time`/`last_click_pos`/`last_scroll_at` if now unused) and the "shell decides single vs double" contract text in `msg/hit_regions.rs`; delete each per-surface `*HitRegion` row enum as its surface is migrated onto `HitRegions<Target>` in the tasks above; verify `rtk cargo clippy --workspace --all-targets` reports no dead code and `rtk cargo check -p mbv` passes.
- [ ] 3.7 Phase-2 gate: full `rtk cargo nextest run -p mbv` green with zero characterization-buffer changes; confirm this phase added no observable behaviour. If review finds an observable pointer/rendered-UI change, treat it as a bug and rerun the affected tests and gate before phase acceptance.

## 4. Phase 3 — Main-surface parity + wheel

- [ ] 4.1 Route parent-owned non-list wheel/chrome behavior for the main panels through `MouseGestureState`. ABS and Feeds — which have no existing `*HitRegion` enum — get their list scrolling and row hits wired here onto the canonical control's `HitRegions<Target>`. Emby list row hits already resolve through the canonical control's `HitRegions<Target>` established in task 3.3; this change adds no duplicate list-row coordinate path.
- [ ] 4.2 Ensure parent click-to-focus and parent-owned controls are wired for main panels; ABS/Feeds list click/select/activate is wired onto the canonical control's `HitRegions<Target>` here (Emby's via task 3.3) — not duplicated.
- [ ] 4.3 Wire parent-owned right-click/context-menu recognition and preserve the click-position anchor contract; menu row identity comes from the canonical control's `HitRegions<Target>` (Emby established in 3.3, ABS/Feeds wired here) — no duplicate row-hit path is added.
- [ ] 4.4 Update the ledger Mouse column for the five main-panel rows with owner, supported gestures, and the test names that verify them.
- [ ] 4.5 Phase-3 gate: run `rtk cargo nextest run -p mbv`, clippy, and fmt; then live-review the observable pointer/rendered-UI behavior. Treat defects as bugs and rerun affected tests and the gate before phase acceptance.

## 5. Phase 4 — Overlays & popups

- [ ] 5.1 Add `Event::Mouse` handling (via the primitives) to `feeds_manage`, `library_routes`, `multiselect`, `save_playlist`, and `search_sidebar`: click-to-select and click-to-dismiss where a keyboard equivalent exists; verify per-component tests.
- [ ] 5.2 Confirm `confirm`, `daemon_lost`, and `remote_reanchor` blocking modals swallow all mouse events and never let a click reach obscured content; verify a `tick()`-level test that a click outside the modal produces no underlying message.
- [ ] 5.3 Verify `context_menu` and `selection_modal` mouse behaviour matches their specs (menu-click executes/closes, outside-click dismisses, wheel does not mutate the obscured view); run `rtk cargo nextest run -p mbv context_menu selection_modal`.
- [ ] 5.4 Update the ledger Mouse column for every overlay/popup/modal row.
- [ ] 5.5 Phase-4 gate: run `rtk cargo nextest run -p mbv`, clippy, and fmt; then live-review observable overlay/popup pointer behavior. Treat defects as bugs and rerun affected tests and the gate before phase acceptance.

## 6. Phase 5 — music_workspace completion + narrow browse

- [ ] 6.1 Migrate the Music mounted parent onto `MouseGestureState` and migrate its narrow list row regions and the track-table row path onto `HitRegions<Target>` (Music has no `*HitRegion` enum; its Wide right-rail row hits were wired in task 3.5), wiring the parent→child delegation call; complete parent-owned non-list hit geometry for narrow/wide chrome and group pills; verify row and chrome gestures.
- [ ] 6.2 Keep `browser_narrow` parent `MouseGestureState` gesture recognition and non-list controls; migrate its narrow list-row hit-testing onto the embedded control's `HitRegions<Target>` and delete the enum here.
- [ ] 6.3 Update the ledger Mouse column for the music-workspace and narrow-browse rows; confirm no row still reads `pending`.
- [ ] 6.4 Phase-5 gate: run `rtk cargo nextest run -p mbv`, clippy, and fmt; then live-review observable pointer/rendered-UI behavior. Treat defects as bugs and rerun affected tests and the gate before phase acceptance.

## 7. Phase 6 — Precedence proofs + close-out

- [ ] 7.1 Land the "simultaneous Queue + Library" precedence proof: both visible, no overlay, a click on each resolves to the painting component through the real `tick()` synchronisation order.
- [ ] 7.2 Land the "blocking overlay swallows mouse" precedence proof through `tick()`.
- [ ] 7.3 Land the "geometry cannot drift" precedence proof: a component resolves a click from the same `HitRegions` it populated during `view()`.
- [ ] 7.4 Verify the three modified capability deltas match the implementation: no code path uses a global hit map / global mouse router; the mouse-emitted `ShellRequest` variants have real handlers (no "mouse-only" no-op arms remain); `rtk ast-grep scan` clean.
- [ ] 7.5 Final close-out: run `rtk cargo nextest run -p mbv` full suite, `rtk cargo clippy --workspace --all-targets`, `rtk cargo fmt --check`, and `rtk make check-code-file-lines`; confirm every ledger row has a filled Mouse column and `openspec validate restore-mouse-support --strict` passes. Then complete live review and acceptance; treat any observable pointer/rendered-UI defect as a bug and rerun affected tests and gates before accepting the change.
