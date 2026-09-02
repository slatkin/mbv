## 1. Foundations (ADR + ledger seam)

- [x] 1.1 Write `docs/adr/0024-mouse-events-through-component-subscriptions.md` recording D1–D3 (any-position subscription + mounted-parent hit-test, shell fold with fixed priority, mounted-parent gesture recognition) as Accepted; verify it cross-links ADR 0022/0023 and `openspec validate restore-mouse-support` still passes.
- [x] 1.2 Replace the "Mouse ownership is out of scope" section of `docs/architecture/interactive-surface-ledger.md` with a "Mouse owner / gestures / verification" column stub on every row (value `pending` for all); verify the table still renders and `rtk make check-code-file-lines` passes for the doc.

## 2. Phase 1 — Delivery spine

- [ ] 2.1 Add `mouse_sub()` helper returning the any-position `EventClause::Mouse` + `SubClause::Always` subscription; unit-test that its clause forwards a `MouseEvent` at arbitrary coordinates.
- [ ] 2.2 Attach `mouse_sub()` at every interactive-component mount site in `src/app/shell.rs` and the lazy mount paths (`mount_home`, `mount_feeds`, `reconcile_destination_mounts`, overlay/popup mounts, `Playback`); verify with a test that a mounted-but-unfocused component receives an injected `Event::Mouse` through `tick()`.
- [ ] 2.3 Add the mouse-message fold to `src/app/shell_run.rs` beside the keyboard router fold: collect mouse-derived messages from a `tick()`, keep at most one by priority (topmost overlay/modal > active panel > other panel > chrome), discard all underlying mouse messages when a blocking overlay is mounted; unit-test each priority branch against a hand-built message list.
- [ ] 2.4 Retire `TerminalObserverEvent::Mouse`: remove the drop arm in `shell.rs`, the `root.rs` mapping, and the enum variant; verify `rtk cargo check -p mbv` passes and no `match` regresses to a wildcard.
- [ ] 2.5 Route the folded mouse message through the existing `handle_terminal_message` dispatch so the five landed surfaces + `PlaybackComponent` act from any focus state; verify by an integration test: focus Queue, click a Library row → Library focuses and the row selects.
- [ ] 2.6 Verify `PlaybackComponent` seekbar + transport clicks work while another component holds focus (integration test through `tick()` asserting the emitted `Msg::Playback`).
- [ ] 2.7 Phase-1 gate: `rtk cargo nextest run -p mbv`, `rtk cargo clippy --workspace --all-targets`, `rtk cargo fmt --check`; no new gestures introduced — the diff only changes delivery.

## 3. Phase 2 — Shared primitives

_This spine change re-homes the existing per-surface `*HitRegion` row hit-testing onto the new `MouseGestureState` recognizer and keeps those enums wired, so list-row click/select/activate/scroll stay functional and verifiable at this change's close-out. The canonical media-list slices (foundation / Home / Music / Queue) later swap each surface's `*HitRegion` enum for the embedded control's `HitRegions<Target>` and delete the enum; that swap and deletion, plus the parent→child point-resolution delegation call, are the only mouse work deferred, each gated on this spine landing first. The tasks below migrate each mounted parent onto `MouseGestureState` and wire parent-owned non-list chrome. This scope note also governs Phase 3._

- [x] 3.1 Create `src/app/components/mouse/hit.rs` with `HitRegions<Tag>` (`clear`, `push(rect, tag)`, `resolve(point) -> Option<Tag>`, last-push-wins); unit-test overlap resolution and empty/out-of-bounds cases.
- [x] 3.2 Create `src/app/components/mouse/gesture.rs` with `MouseGestureState` consuming raw `MouseEvent`s and emitting `Click`/`DoubleClick`/`RightClick`/`Scroll` (reserve `DragStart`/`DragMove`/`DragEnd`/`HoverEnter`/`HoverLeave`); unit-test the double-click window and wheel throttle in isolation.
- [ ] 3.3 Migrate the mounted Browser parent onto `MouseGestureState`: re-home its existing per-surface `*HitRegion` row hit-testing onto the recognizer and keep the enum wired; wire parent-owned non-list chrome (pills, wheel-for-chrome, right-click recognition + anchor) and parent click-to-focus; verify row click/select/activate/scroll and parent-owned control delivery. Deferred to the gated canonical media-list slice: swapping the `*HitRegion` enum for the embedded control's `HitRegions<Target>`, deleting the enum, and the parent→child delegation call.
- [ ] 3.4 Migrate the Home and Queue mounted parents onto `MouseGestureState`: re-home their existing per-surface row hit-testing onto the recognizer and keep the enums wired; wire parent-owned non-list chrome (including Queue scope buttons and wheel-for-chrome) and parent click-to-focus; verify row click/select/activate/scroll and parent-owned control delivery. Deferred to the gated slices: the enum → `HitRegions<Target>` swap, enum deletion, and the parent→child delegation call.
- [ ] 3.5 Migrate the TV and Music mounted parents onto `MouseGestureState`: re-home their existing per-surface row hit-testing onto the recognizer and keep the enums wired; wire parent-owned pills and non-list chrome and parent click-to-focus; verify row gestures and parent-owned control delivery. Deferred to the gated slices: the enum → `HitRegions<Target>` swap, enum deletion, and the parent→child delegation call.
- [ ] 3.6 Delete the shell-side recognition glue (`note_browse_double_click`, `note_browse_scroll`, `App.last_click_time`/`last_click_pos`/`last_scroll_at` if now unused) and the "shell decides single vs double" contract text in `msg/hit_regions.rs`; do NOT delete the per-surface `*HitRegion` row enums — they stay wired through this change and are removed by each canonical slice; verify `rtk cargo clippy --workspace --all-targets` reports no dead code and `rtk cargo check -p mbv` passes.
- [ ] 3.7 Phase-2 gate: full `rtk cargo nextest run -p mbv` green with zero characterization-buffer changes; confirm this phase added no observable behaviour. Per design D10 visual-first ordering, any observable pointer/rendered-UI change in this phase requires user live visual confirmation before a rendered-UI or characterization-buffer test is added or modified.

## 4. Phase 3 — Main-surface parity + wheel

- [ ] 4.1 Route parent-owned non-list wheel/chrome behavior for the main panels through `MouseGestureState`; the existing per-surface list-row wheel/hit path is retained and re-homed, not duplicated. Emby, ABS, and Feeds list scrolling and row hits keep working through the current `*HitRegion` path until their canonical media-list slice swaps it for the embedded control; this change adds no new or duplicate list-row coordinate path.
- [ ] 4.2 Ensure parent click-to-focus and parent-owned controls are wired for main panels; list click/select/activate keeps working through the retained per-surface row path, with the embedded-control seam swap deferred to each slice — not reimplemented or duplicated here.
- [ ] 4.3 Wire parent-owned right-click/context-menu recognition and preserve the click-position anchor contract; menu row identity comes from the retained per-surface `*HitRegion` path until each canonical slice swaps it for the embedded control — no new or duplicate row-hit path is added here.
- [ ] 4.4 Update the ledger Mouse column for the five main-panel rows with owner, supported gestures, and the test names that verify them.
- [ ] 4.5 Phase-3 gate: `rtk cargo nextest run -p mbv`, clippy, fmt. Per design D10 visual-first ordering, this phase's observable pointer/rendered-UI changes require user live visual confirmation before any rendered-UI or characterization-buffer test is added or modified for them.

## 5. Phase 4 — Overlays & popups

- [ ] 5.1 Add `Event::Mouse` handling (via the primitives) to `feeds_manage`, `library_routes`, `multiselect`, `save_playlist`, and `search_sidebar`: click-to-select and click-to-dismiss where a keyboard equivalent exists; verify per-component tests.
- [ ] 5.2 Confirm `confirm`, `daemon_lost`, and `remote_reanchor` blocking modals swallow all mouse events and never let a click reach obscured content; verify a `tick()`-level test that a click outside the modal produces no underlying message.
- [ ] 5.3 Verify `context_menu` and `selection_modal` mouse behaviour matches their specs (menu-click executes/closes, outside-click dismisses, wheel does not mutate the obscured view); run `rtk cargo nextest run -p mbv context_menu selection_modal`.
- [ ] 5.4 Update the ledger Mouse column for every overlay/popup/modal row.
- [ ] 5.5 Phase-4 gate: `rtk cargo nextest run -p mbv`, clippy, fmt. Per design D10 visual-first ordering, this phase's observable overlay/popup pointer changes require user live visual confirmation before any rendered-UI or characterization-buffer test is added or modified for them.

## 6. Phase 5 — music_workspace completion + narrow browse

- [ ] 6.1 Migrate the Music mounted parent onto `MouseGestureState`, re-homing its existing narrow/wide row hit-testing onto the recognizer and keeping it wired; complete parent-owned non-list hit geometry for narrow/wide chrome and group pills; verify row and chrome gestures. Deferred to the canonical media-list slices: swapping wide/narrow list row regions and the track-table row path for `HitRegions<Target>` and deleting those enums.
- [ ] 6.2 Keep `browser_narrow` parent `MouseGestureState` gesture recognition and non-list controls; its existing narrow list-row hit-testing is re-homed onto the recognizer and stays wired, with the embedded-control swap and enum deletion deferred to the canonical media-list slices.
- [ ] 6.3 Update the ledger Mouse column for the music-workspace and narrow-browse rows; confirm no row still reads `pending`.
- [ ] 6.4 Phase-5 gate: `rtk cargo nextest run -p mbv`, clippy, fmt. Per design D10 visual-first ordering, this phase's observable pointer/rendered-UI changes require user live visual confirmation before any rendered-UI or characterization-buffer test is added or modified for them.

## 7. Phase 6 — Precedence proofs + close-out

- [ ] 7.1 Land the "simultaneous Queue + Library" precedence proof: both visible, no overlay, a click on each resolves to the painting component through the real `tick()` synchronisation order.
- [ ] 7.2 Land the "blocking overlay swallows mouse" precedence proof through `tick()`.
- [ ] 7.3 Land the "geometry cannot drift" precedence proof: a component resolves a click from the same `HitRegions` it populated during `view()`.
- [ ] 7.4 Verify the three modified capability deltas match the implementation: no code path uses a global hit map / global mouse router; the mouse-emitted `ShellRequest` variants have real handlers (no "mouse-only" no-op arms remain); `rtk ast-grep scan` clean.
- [ ] 7.5 Final close-out: `rtk cargo nextest run -p mbv` full suite, `rtk cargo clippy --workspace --all-targets`, `rtk cargo fmt --check`, `rtk make check-code-file-lines`; confirm every ledger row has a filled Mouse column and `openspec validate restore-mouse-support --strict` passes. Per design D10 visual-first ordering, the precedence proofs (7.1–7.3) are non-visual and may precede confirmation; any observable pointer/rendered-UI change closed out here still requires user live visual confirmation before a rendered-UI or characterization-buffer test is added or modified for it.
