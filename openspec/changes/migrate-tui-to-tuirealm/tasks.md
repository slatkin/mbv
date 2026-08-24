Each surface-conversion task below bundles a standard bundle unless noted:
create the component under `src/app/components/`, move its state/input/render off
`App` (delete, don't mirror), reproducing the surface's current cursors, pills,
panes, hero behaviour, focus targets, and keys exactly as the source defines them
today (design §Governing Principle — there is no target design to invent), add
local update/output tests + an App-free
`TestBackend` render test + one shell-routing test, flip its
`docs/architecture/interactive-surface-ledger.md` row to `migrated` with its
verification record, and verify with the named narrow `rtk cargo nextest`
selector plus a clean `rtk ast-grep scan`. Every checkpoint commit must be
behaviour-preserving; none except group 5 is a completion.

## 1. Foundation (runs the app on TuiRealm without behaviour change)

- [x] 1.1 Add `tuirealm = "4.1"` (default features already include `crossterm` and `derive`); verify `rtk cargo check -p mbv` succeeds and `Cargo.lock` resolves tuirealm 4.1 on the existing ratatui 0.30/crossterm 0.29.
- [x] 1.2 Declare `rust-version = "1.88"` in `[workspace.package]` **and** add `rust-version.workspace = true` to each member (`mbv`, `mbv-core`, `mbvd`) — a bare `[workspace.package]` entry is not inherited automatically; verify `rtk cargo check --workspace` passes and CI uses a ≥1.88 toolchain.
- [x] 1.3 Add `src/app/components/` with the `ComponentId`, `Msg`, and `UserEvent` enums from design D3–D5 (surface variants may start empty); verify `rtk cargo check -p mbv`.
- [x] 1.4 Introduce the shell `Model` holding `App` and the TuiRealm `Application<ComponentId, Msg, UserEvent>`; verify it builds and the binary still launches.
- [x] 1.5 Convert `App::run` to drive `application.tick(PollStrategy::Once(..))` and mark the frame dirty when `tick` reports a processed event (reuse the existing `had_events` → `wants_terminal_render` path). The Model keeps `App` and draws the current legacy UI and runs existing handlers **directly**; a temporary message-only `LegacyInput` component (owns no `App`) only translates terminal events into a typed legacy message the Model consumes. Verify the app boots, the first frame still precedes Remote Service startup (ADR 0018), and existing input/render characterization tests pass unchanged.
- [x] 1.6 Map each run-loop receiver (`src/app/mod.rs:412-517`: startup, player, library, Search, session, cast, shared-data, feed, image, websocket, ABS socket) to either a shell-owned adapter (default) or a TuiRealm `Port`, each injecting a `UserEvent` token; the owned model is validated in the shell by the existing generation/revision/session/image-key guards and then written into the target via `get_component_mut`+downcast. Prefer shell-owned adapters for receivers that are replaced at runtime (player, websocket, ABS socket, setup), since `restart_listener` is the only runtime port mechanism and it replaces the whole listener. Verify async-completion behaviour and stale-completion discards are unchanged by characterization.
- [x] 1.7 Add the `key_policy` ordered precedence table mirroring the current `CONTEXT_STACK` order and wire global/parent bindings as TuiRealm subscriptions with mutually-exclusive `SubClause` guards derived from it; verify against the existing ADR 0002 input-precedence tests.
- [x] 1.8 Route mouse via `EventClause::Any` subscriptions on visible top-level regions, each filtering `Event::Mouse(column,row)` against its own painted geometry and guarded `Not(IsMounted(overlay))` under blocking overlays (no shell hit-router — `Application` has no per-component event delivery); during CP1 `LegacyInput` forwards the mouse event and the Model runs the existing legacy mouse path. Verify mouse behaviour is unchanged by characterization.
- [x] 1.9 Add enforcement scaffolding: `rules/interactive-component-boundary/*.yml` (reject `impl App`, `App` as type, Service-client/`PlayerProxy` deps, `mpsc` ownership) each with one accepted + one rejected fixture, register the dir in `sgconfig.yml`, and add `.github/workflows/architecture-boundaries.yml` job `interactive-component-boundary` pinning `ast-grep` 0.44.1; verify `rtk ast-grep scan` passes and fixtures demonstrate accept/reject.

## 2. Low-risk leaf surfaces

- [x] 2.1 Convert Help sidebar (local scroll, destination-derived content); verify `rtk cargo nextest run -p mbv help` + `rtk ast-grep scan`.
- [x] 2.2 Convert Confirm modal (shared yes/no); verify `rtk cargo nextest run -p mbv confirm_modal` + scan.
- [x] 2.3 Convert Daemon-lost modal (process-lifecycle effects stay shell-owned); verify `rtk cargo nextest run -p mbv daemon_lost` + scan.
- [x] 2.4 Convert Remote-reanchor popup (reconciliation stays shell-owned); verify `rtk cargo nextest run -p mbv remote_reanchor` + scan.
- [x] 2.5 Convert Context menu (exclusive top-priority overlay with anchor geometry); verify `rtk cargo nextest run -p mbv context_menu` + scan.

## 3. Medium-risk surfaces

- [x] 3.1 Extract the Search render seam: expose `render_panel_shell*`, `render_sidebar_scrollbar`, `panel_row_text_width`, `render_panel_row` as typed render-component functions (output-preserving, no `impl App`); verify existing Search buffer characterization is unchanged.
- [x] 3.2 Convert the global Search sidebar as an ordinary row (component-owned 300 ms debounce driven by `UserEvent::Clock`; preserve the `global-search-sidebar` behaviour contract; do NOT fix its known bugs); verify `rtk cargo nextest run -p mbv search_sidebar` + scan.
- 3.3 Convert inline library Search — part of the §3.5-chain below; do not schedule standalone.
- [ ] 3.4 Convert Home (cross-Service rows and hero presentation); verify `rtk cargo nextest run -p mbv home` + scan.
- 3.5 Convert the Emby generic/Movies/home-video browser — part of the §3.5-chain below; do not schedule standalone.
- [ ] 3.6 Convert Feeds (grouping, selector, list, inline hero); verify `rtk cargo nextest run -p mbv feeds` + scan.
- [x] 3.7 Convert Sessions sidebar (merged Emby/Cast targets, fixed-stride geometry); verify `rtk cargo nextest run -p mbv sessions` + scan.
- [ ] 3.8 Convert Selection modal (filters, source-specific behaviour, explicit row/selector targets); verify `rtk cargo nextest run -p mbv selection_modal` + scan.
- [ ] 3.9 Convert Playback prompts (skip-intro/next-up; Player effects stay shell-owned); verify `rtk cargo nextest run -p mbv playback_prompt` + scan.
- [ ] 3.10 Convert Settings nested popups — Multiselect, Library-routes, Feed-management — as `Popup` children; verify `rtk cargo nextest run -p mbv settings_popup` + scan.

### §3.5-chain — shared Emby browser render seam (sequential, dependency order overrides phase)

`3.3`, `3.5`, `4.2`, `4.3`, and `4.4` all read or write the same render
functions (`render/components/list.rs`, `tv_wide.rs`, `movies_wide.rs`,
`music_wide.rs`) — see `scoping-3.3-3.5.md` "Correction (2026-08-24, session
3)" for the full trace. The medium/high-risk phase split scattered this one
dependency chain across group 3 and group 4 with nothing showing the real
order; that split is retired for these five. Execute them in this order only
— do not start a later step before the one above it has landed, and do not
pull any of them out to run alongside the phase-4 items below:

1. [ ] 3.5 Convert the Emby generic/Movies/home-video browser (owns the
   shared render-seam extraction every downstream step here builds on);
   verify `rtk cargo nextest run -p mbv emby_browser` + scan.
2. [ ] 4.2 Convert the TV workspace (two focusable panes, season/episode
   child targets; built on 3.5's seam); verify `rtk cargo nextest run -p mbv
   tv_workspace` + scan.
3. [ ] 4.3 Convert the grouped Music workspace (album/track focus coupling,
   track targets; built on 3.5's seam); verify `rtk cargo nextest run -p mbv
   music_workspace` + scan.
4. [ ] 4.4 Convert inline album-track interaction (child state machine of
   4.3's Music workspace); verify `rtk cargo nextest run -p mbv album_track`
   + scan.
5. [ ] 3.3 Convert inline library Search (`LibSearch`, child of one Emby
   browser, distinct from global Search) — downstream of all four steps
   above, since `render_search_box`'s results list renders through each of
   their wide renderers; verify `rtk cargo nextest run -p mbv
   inline_library_search` + scan.

## 4. High-risk surfaces

- [ ] 4.1 Convert Queue (cursor/scroll/scope move to the component; canonical queue stays in the Player owner, referenced by opaque `QueueSlotId`); verify `rtk cargo nextest run -p mbv queue` + scan.
- 4.2, 4.3, 4.4 moved into the §3.5-chain above (section 3) — not independently schedulable here.
- [ ] 4.5 Convert the Audiobookshelf podcast browser (show/episode workspace, selector targets); verify `rtk cargo nextest run -p mbv abs_podcast` + scan.
- [ ] 4.6 Convert the Audiobookshelf book browser (browser/chapter workspace, replacement geometry); verify `rtk cargo nextest run -p mbv abs_book` + scan.
- [ ] 4.7 Convert Playlists sidebar with component-owned variable-row `hit_test` (removes the duplicated mouse-path geometry in `input_mouse_panels.rs`); verify `rtk cargo nextest run -p mbv playlists` + scan.
- [ ] 4.8 Convert the Save-playlist dialog (child of the Playlists workflow); verify `rtk cargo nextest run -p mbv save_playlist` + scan.
- [ ] 4.9 Convert the Settings sidebar and setup forms (Service effects stay shell-owned via `Msg::Service`); verify `rtk cargo nextest run -p mbv settings` + scan.
- [ ] 4.10 Convert Playback chrome and global controls (Player authority stays outside; reduced playback-status projection only); verify `rtk cargo nextest run -p mbv playback_chrome` + scan.

## 5. Root, overlay routing, and completion gate

- [ ] 5.1 Convert the Library parent (active destination, Panel focus/mode, child routing); verify `rtk cargo nextest run -p mbv library_parent` + scan.
- [ ] 5.2 Convert Root UI + overlay-stack routing using TuiRealm's native LIFO focus stack (open = `active`, dismiss = `umount` → auto-`blur`/restore; no shell-owned focus stack), keeping only overlay z-order in the owning component; verify `rtk cargo nextest run -p mbv root_ui` + scan.
- [ ] 5.3 Remove `LegacyInput`, `CONTEXT_STACK` interaction dispatch, `AppLayout`, duplicated mouse-coordinate paths, and all temporary adapters/state mirrors; verify `rtk cargo check -p mbv` and that no `impl App` interaction handler or component-local `App` field remains for a migrated surface.
- [ ] 5.4 Confirm every mouse path reads component-owned geometry (no global hit map); verify the six precedence/mouse proofs (blocking-overlay swallow, parent/global precedence, simultaneous Queue+Library mouse, overlay blocks underlying mutation, deterministic focus restoration, geometry cannot drift) as tests.
- [ ] 5.5 Flip all remaining `docs/architecture/interactive-surface-ledger.md` rows to `migrated` with verification records; verify no `legacy` row remains.
- [ ] 5.6 Final gate: `rtk cargo check -p mbv`, `rtk cargo nextest run -p mbv`, `rtk cargo clippy --workspace --all-targets`, `rtk ast-grep scan`, and `rtk make check-code-file-lines` all pass; confirm no parallel legacy interaction framework remains and the shell Model holds only shell/runtime authority plus the TuiRealm `Application`.
