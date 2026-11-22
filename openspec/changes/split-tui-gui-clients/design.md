# Design: split-tui-gui-clients

## Context

See proposal.md (Why). The current state that shapes the approach:

- The root package `mbv` is a library and the `mbv` binary. Its `src/app/` holds the shared `App` (182 fields), dispatch (~23.5k lines), state (~11k lines), the keyboard router (`src/app/input/`, ~5k lines), the TUI shell (`src/app/shell/`, ~14.7k lines), and the GUI-only modules `gui_library`, `gui_settings`, `gui_playlists`, `gui_client_state` (~14k lines).
- `crates/mbv-gui` depends on the root `mbv` package. It uses `App`, `ClientTickHost`, the `Gui*Command` types, and the `test`-feature seams (`stub_app_with_queue_and_cmd_rx`).
- The GUI-only modules are `impl App` blocks. They read 38 non-GUI `App` fields directly, own 26 `gui_*` fields on `App`, and add GUI variants to the shared event enum (`GuiGroupAlbums`, `GuiEmbyRootItems`, `GuiShowsNew`).
- No GUI module names the TUI shell. Outside `shell/` and `input/`, these non-test files use ratatui or crossterm: `state/projection/{card,chrome_status,tv_wide,visualizer}.rs`, `infra/terminal.rs`, `infra/image_fetch.rs`, `infra/image_fetch/protocol.rs`, and `test_seams.rs` (`crossterm_key_to_neutral`, also tuirealm and `mbv-components`). Inside `input/`, `input.rs` builds a ratatui `Rect` from `terminal_width` and `terminal_height` for the tab-strip width. One shared file uses the shell: `dispatch/run_loop/teardown.rs` (`shell::Model`).
- Shared code and GUI-only code also import items from the TUI crates `mbv-render` and `mbv-components`: 27 non-test shared files and 13 GUI module files. Some items are toolkit-free shared logic that lives in a TUI crate (hero content builders, album plans, sort keys, book rows). Others are TUI terminal geometry that shared state reads (`MINI_VIEW_THRESHOLD`, `chrome_geometry`, `wide_library_panes`). So the GUI uses TUI code today through these crates. D13 places each item.
- `App::gui_client` is the one Client-kind fact. It gates more than the Mini rule: the terminal-width Mini view and the hidden empty queue column (`client_arranges_panels`), the visualizer and visual-slot hide setters (`visual_slot_controls`), the shown-queue rules in `state/named_queues.rs` and `state/queue_link.rs` (including the closed-queue fallback in `adopt_named_queue_list`), and the router gates `GuiClient` and `GuiQueueTarget` in `input/key_policy.rs`. D6 replaces it.
- The prefs file is one JSON object that `App::save_prefs` reads, changes, and writes back. It holds keys that only one Client reads (`gui_player_auto_hide`, `visual_slot_hidden`, `queue_column_width`, `list_pane_width`).
- Shared signatures carry terminal cells, for example the context-menu anchor `Option<(u16, u16)>` in `build_queue_selection_menu_at`.
- `mbv-theme` has an optional `ratatui` feature. `mbv-images` depends on `ratatui` and `ratatui-image` (crossterm feature).
- The Owner process is `mbv --__local-daemon` (`src/startup.rs`, `src/local_daemon.rs`). The GUI resolves the sibling `mbv` program to start it. The Owner's Pin swap, Unpin, and Open launch `/proc/<owner pid>/exe`, which is the TUI because the Owner and the TUI are one file today.
- The attach gate compares `app_version` and `wire_id` only (`BuildInfo::differs_for_gate`). `build_id` is display only.
- `ClientTickHost` already gives each Client its own drain hooks for the shared tick.

## Goals / Non-Goals

**Goals:**

- Cargo enforces the boundary: `mbv-client-core` cannot name a terminal or GUI toolkit, and neither Client crate can name the other. A break is a compile error, not a review finding.
- Each Client keeps its present behaviour. The TUI and the GUI attach to one Owner at the same time.
- An agent that works in one Client crate reads only that crate, the core, and the shared leaf crates.

**Non-Goals:**

- No new user feature, key, screen, or command.
- No change to the ctrl protocol or the wire schema. `wire_id` stays the same.
- No change to `mbvd`.
- No merge of the TUI Library and the GUI Library panel. They present different surfaces (the `client-interaction-core` rule "A surface that one Client presents alone" stands). Shared behaviour under them stays in the core.

## Decisions

### D1. Crate layout

```
  crates/mbv-tui  (bin "mbv")        crates/mbv-gui  (bin "mbv-gui")
        |   \                              /   |
        |    mbv-components, mbv-render,  /    |
        |    mbv-tui-support (TUI-only)  /     |
        v                               v      |
  crates/mbv-client-core  <-------------+      |
        |                                      |
        v                                      v
  shared leaf crates: mbv-surfaces, mbv-ui-model, mbv-ui-msg, mbv-keybinds,
  mbv-text, mbv-theme, mbv-images, mbv-desktop, and the domain and
  provider crates (queue, ctrl, config, emby, audiobookshelf, feed, cast,
  remote-player, player, visualizer, net, ws, ids, core)

  crates/mbv-owner (bin "mbv-owner") --> mbv-daemon, mbv-desktop, mbv-config, mbv-ctrl
  (no dependency on mbv-client-core, mbv-tui, or mbv-gui)
```

The root `Cargo.toml` becomes a virtual workspace (`[workspace]`, `[workspace.package]`, `[workspace.dependencies]`, `[workspace.lints]` only). `default-members` lists every crate except `mbv-gui`, as today, and adds `mbv-client-core`, `mbv-tui`, and `mbv-owner`.

Alternatives considered:

- Keep the root package as the TUI and only move the core out. Rejected: the root `src/` would still look like "the app", which is the confusion this change removes.
- Separate repositories. Rejected by the user: one repo.

### D2. Placement rule

For each function, type, field, and test:

1. Both Clients reach it (directly, or through the shared router and `Command` dispatch) → `mbv-client-core`.
2. Only one Client reaches it, and it is presentation or Client-local state (layout, terminal cells, window state, a surface that Client alone presents) → that Client's crate.
3. Only one Client reaches it, but it is Owner, Service, queue, or playback behaviour → `mbv-client-core`, as a public call. It is not UI code, and the other Client can call it later without a move.
4. A type both Clients need but that has no `App` dependency → the lowest shared leaf crate that fits (`mbv-ui-model`, `mbv-ui-msg`, `mbv-surfaces`).

Where a GUI-only and a TUI-only entry point wrap one shared function, the shared function stays in the core and each wrapper moves to its Client. Where two Clients have two copies of one rule, the copy in the core stays and the other is deleted.

### D3. Root `src/` placement

| Today | Goes to |
|---|---|
| `src/main.rs`, `src/startup.rs` (args, `-q`, `--toggle`, `-d`, `--pin`, TUI run) | `mbv-tui` |
| `src/app/shell/`, `src/app/shell.rs` | `mbv-tui` |
| `src/app/input/` (router, key policy, resolver, per-surface key modules) | `mbv-client-core` |
| `src/app/input/gui.rs`, `src/app/input/gui/` | `mbv-gui` |
| crossterm key translation in `src/app/input/` | `mbv-tui` |
| `src/app/{dispatch,state,infra}/` without ratatui or crossterm | `mbv-client-core` |
| `src/app/state/projection/{card,chrome_status,tv_wide,visualizer}.rs` | `mbv-tui` |
| `src/app/infra/terminal.rs`, `src/app/infra/image_fetch/protocol.rs`, ratatui parts of `infra/image_fetch.rs` | `mbv-tui` |
| `src/app/dispatch/run_loop/teardown.rs` (names `shell::Model`) | `mbv-tui` (the shared part of the teardown stays in the core as a call over `App`) |
| `src/app/gui_library`, `gui_settings`, `gui_playlists`, `gui_client_state` | `mbv-gui` |
| `src/app/{client_core,client_drains,waking_relay,app_stub}.rs` | `mbv-client-core` |
| `src/app/test_seams.rs`: `crossterm_key_to_neutral` and every other seam that names crossterm, tuirealm, or `mbv-components` | `mbv-tui`, with the shell-driving tests that use it |
| `src/app/test_seams.rs`: the toolkit-free seams | `mbv-client-core` (`test` feature) |
| `src/app/input.rs`: the tab-strip width from `terminal_width`, `terminal_height`, a ratatui `Rect`, and `chrome_geometry` | `mbv-tui`; the TUI pushes the resulting width to the core as a value |
| queue, status bar, and footer projections without ratatui | `mbv-client-core` |
| `src/owner_link.rs` + `owner_link/`, `src/client_kind.rs`, `src/config.rs` | `mbv-client-core` |
| `src/owner_restart.rs`: pure `follow_up` decision | `mbv-client-core`; the terminal prompt goes to `mbv-tui` |
| `src/local_daemon.rs`: `run_local_daemon_main`, swap launch, tray hooks | `mbv-owner` |
| `src/local_daemon.rs`: `spawn_detached` (a Client starts the Owner) | `mbv-client-core` (`owner_link`) |
| `src/single_instance.rs`: lock holder | `mbv-owner`; the Client-side probe goes to `mbv-client-core` |
| `src/pin.rs`: pinwin argv, `--pin`, `--in-panel`, panel identity | `mbv-tui` |
| `src/pin.rs`: `terminal_prefix`, `notify` | `mbv-desktop` (new `launch` module); the Owner and the TUI both call it |
| `src/launch.rs` (`launch_exe`) | `mbv-desktop` (`launch` module) |
| `src/app/tests/` | each test moves with the code it owns: ratatui and tick-integration tests to `mbv-tui`, core tests to `mbv-client-core` |

### D4. Each Client wraps `App` by composition

`App` lives in `mbv-client-core`. Each Client owns it:

- TUI: the shell `Model` already owns `App`. It also takes the TUI-only state that leaves `App`: `terminal_width`, `terminal_height`, the narrow-terminal Mini view state, and every other field that only the shell reads.
- GUI: a new `GuiCore { app: App, gui: GuiClientState }` in `mbv-gui` owns `App` and all GUI state. Of the 26 `gui_*` fields, 24 leave `App` for `GuiClientState`. The other two do not move: `gui_client` is replaced by `ClientCapabilities`, and `gui_shown_entry` stays in the core as `shown_entry` (D6). The `impl App` blocks of the GUI modules become `impl GuiCore` blocks.
- Prefs: the file format and every key name stay the same. The core keeps `load_prefs` and writes only the keys that the core owns. It gets one `pub fn save_client_pref(key: &str, value: serde_json::Value)` with the same read, change, and write. Each Client reads its own keys from `load_prefs` at start and writes them through `save_client_pref` when they change: the GUI owns `gui_player_auto_hide`, and the TUI owns every key whose field rule D2 moves to the shell `Model`. A key whose field stays in the core stays a core key.

`App` fields stay `pub(crate)` inside the core. A Client reads and changes `App` only through `pub` methods. The 38 non-GUI fields that the GUI modules read directly get a `pub` read method each, or the GUI code calls an existing core method that already does the work. No field becomes `pub`.

Alternatives considered:

- `App<X: ClientExt>` with a Client-specific slot. Rejected: it makes ~70k lines generic, and every signature changes.
- `Box<dyn Any>` client slot with downcasts. Rejected: it hides the boundary from the compiler.
- An extension trait on `App` in each Client crate. Rejected: it needs `pub` fields.

### D4a. The core calls no GUI function: a two-part seam

Today shared core code takes `Option<&mut GuiClientState>` and calls `gui_library` functions (the command dispatch, playback requests, the library event handlers, and feed loads). `GuiClientState` also holds about 15 `gui_library` types. A move row cannot compile, and its `rg` check cannot pass, while the core names these items. So two seams land before any file moves:

1. Commands. `App::dispatch` and `App::handle_playback_request` stop taking the GUI state. An arm that only the GUI acts on becomes a documented no-op in the core, as `PanelIntent::MoveQueueRow(_) => {}` is today. The GUI entry calls the core first, then does its own half. Every variant is listed; no wildcard arm.
2. Library events. The library event handlers take `Option<&mut dyn ClientLibraryStores>`, a core trait with `wants_feed_target(&self, app: &App, lib_idx, &FeedHomeVideoSelection) -> bool` and `defer(&mut self, ClientLibraryEffect)`. The effects are `RowArtSourcesChanged`, `FeedTargetLanded`, and `FeedTargetFailed`. The GUI applies each deferred effect after the event that raised it, before the next event. The TUI passes `None`, so it does nothing, as today.

Also part of the seam: the App half of the artist-name lookup stays in the core under the neutral name `ArtistNameLookup`, and its answer stays gated on client presence. `handle_feed_home_video_selection_failed` splits: the fetch-mark retirement and the log stay in the core, and the claim insert becomes `FeedTargetFailed`. The `gui_overlay` accessors move onto `GuiClientState`.

Move order: the `impl` blocks move first, callers first. The types and `GuiClientState` move last and together, because they cross the boundary together. A type or helper stays in the core while any core file still names it. A `pub` that a step widens is recorded in the row's task note and narrowed at the end. The eight `Gui*` key-policy bindings and `OverlayFocus::SettingsScreen` get neutral names in the last step.

Alternatives considered:

- A hook that runs GUI code inside the core handler. Rejected: `gui_queue_in_view_row_fetches` reaches most destinations.
- An async channel. Rejected: it changes the event order, and it cannot answer the feed-wanted query.
- Keep the types in the core. Rejected: it is against D3, D4, and the Goals.

### D5. Client-only events leave the shared event enum

The shared event enum loses `GuiGroupAlbums`, `GuiEmbyRootItems`, and `GuiShowsNew`. GUI fetch workers report on a channel that `GuiCore` owns. `ClientTickHost` gets one more hook, `drain_client_events(&mut self) -> bool`, which runs in the shared tick after `drain_lib_events`. The GUI implementation drains its channel. The TUI implementation is a documented no-op that returns `false`. The waker is the same `ClientWaker`, so a GUI worker result still wakes the tick.

### D6. Neutral shared signatures

- Context menus: the core builds the menu entries and the target. Each Client places the menu. The `anchor: Option<(u16, u16)>` parameter leaves the core. `build_queue_selection_menu_at` folds into `build_queue_selection_menu`, and the TUI keeps its pointer cell beside the returned menu.
- Any other terminal-cell or toolkit type that the compiler finds in a core signature after the move gets the same treatment: a neutral value in the core, the conversion in the Client.

### D6a. Client capabilities replace the Client-kind flag

The core names no Client. Where the Clients differ, the core reads a neutral fact. `App::gui_client`, `App::enable_gui_client`, `client_arranges_panels`, and `visual_slot_controls` are deleted. The core gets:

```rust
/// What this Client presents. Each Client builds its own value and passes it
/// to the core before the first step. The core never names a Client.
pub struct ClientCapabilities {
    /// The Client may present one panel only, and hides an empty queue
    /// column in its two-panel layout (TUI: true, GUI: false).
    pub terminal_panel_rules: bool,
    /// The Client paints the visualizer and the visual-slot hide control
    /// (TUI: true, GUI: false).
    pub visual_slot_controls: bool,
    /// The Client shows one queue entry at a time that the user picks: a
    /// Named queue or the peer's queue (TUI: false, GUI: true).
    pub shown_queue_entries: bool,
    /// The Client paints the transport chips, the seek-10/30 controls, the
    /// playback-target chooser, and moves queue rows by key (TUI: false,
    /// GUI: true).
    pub transport_chip_keys: bool,
}
```

Each existing `gui_client` reader maps to one fact. No behaviour changes:

| Behaviour today | After the split |
|---|---|
| Terminal-width Mini view (`is_mini_view`) | The TUI computes "one panel only" from its terminal width and `MINI_VIEW_THRESHOLD` on each resize and pushes it with `App::set_single_panel(bool)`. `is_mini_view` reads that fact and `terminal_panel_rules`. The GUI never pushes it, so it stays false. |
| Hidden empty queue column (`queue_column_hidden_empty`), `effective_panel_focus`, `effective_panel_mode` | Stay in the core and read `terminal_panel_rules` and the pushed single-panel fact. GUI code that reads them today calls the same `pub` methods. |
| Visualizer and visual-slot hide setters, `visual_slot_hide_in_effect` | Stay in the core and read `visual_slot_controls`. |
| The shown queue (`gui_shown_entry`): `show_peer_entry`, `show_local_current_entry`, `show_entry`, `show_named_queue`, `shown_queue_edit`, `adopt_named_queue_snapshot`, the closed-queue fallback in `adopt_named_queue_list`, and `queue_link_resolution` | Stay in the core. The field is renamed `shown_entry` and the gates read `shown_queue_entries`. The Owner-projection path keeps the fallback, so the `gui-queue-chooser` rule and ADR 0041 hold. The GUI queue chooser reads the entry through a `pub` read method. |
| Router gates `KeyPolicyGate::GuiClient` and `GuiQueueTarget` | Renamed `TransportChipKeys` and `TransportChipQueueTarget`. The router snapshot carries `ClientCapabilities` in place of `gui_client`, and the gates read `transport_chip_keys`. The actions stay in the shared registry, so a rebind works the same in both Clients. |
| `LaunchRestore::Done` set by `enable_gui_client` | The GUI calls a `pub fn skip_launch_restore(&mut self)` before its first step. |
| Hello claim `named-queues` (`ClientKind::Gui` on a local link) | Reads `shown_queue_entries`. `ClientKind` is deleted if nothing else reads it; otherwise its readers move to the capability they mean. |

Alternative considered: keep a `ClientKind { Tui, Gui }` enum in the core. Rejected: every `if kind == Gui` branch is GUI code in the shared core, which the `client-interaction-core` delta forbids.

### D6b. Breakpoints

The core reads only the shared breakpoint class (`mbv-surfaces`) and the pushed single-panel fact. The TUI derives both from its terminal size. The tab-strip width rule in `input.rs` (a ratatui `Rect` and `chrome_geometry` over `terminal_width`) moves to the TUI. The `tab_scroll` field follows rule D2.

### D7. Shared leaf crates lose their TUI parts

- `mbv-theme`: delete the `ratatui` feature and the `to_ratatui` method.
- `mbv-images`: the terminal image protocol code (ratatui-image, crossterm) moves out. Decode, cache, and processing stay.
- `mbv-components` and `mbv-render` sit below `mbv-tui` and need both removed pieces. So both go in a new TUI-only leaf crate `mbv-tui-support` (ratatui, ratatui-image): a `ToRatatui` extension trait for `mbv_theme` colour values, and the terminal image protocol. `mbv-tui`, `mbv-components`, and `mbv-render` depend on it. Nothing in the core or the GUI does.

### D8. The Owner program

`crates/mbv-owner` builds the `mbv-owner` binary. `main` installs the same allocator settings and fatal-signal handler as the `mbv` binary does today, then runs today's `run_local_daemon_main`. It takes `--log-level` as today's hidden mode does. It has no other argument. Run with no Owner lock free, it exits as today's hidden mode does.

A Client starts the Owner (`owner_link::start_owner`) from the sibling `mbv-owner` of its own executable, else from `mbv-owner` on `PATH` (the rule that the GUI's `owner_program` uses today, moved to the core and used by both Clients). The `owner_exe` option and `App::set_owner_exe` stay for tests and for the GUI's restart path.

The hidden `--__local-daemon` flag is removed from `mbv`.

### D9. The Owner launches TUI Clients of its own build

At start, `mbv-owner` opens the sibling `mbv` program file (`std::fs::File::open`, so `O_CLOEXEC`) and keeps the `File` for its lifetime, with the path and the `(dev, ino)` it opened. For Pin swap, Unpin, and Open:

- If the path still names the same `(dev, ino)`, launch the path.
- Else, launch `/proc/<owner pid>/fd/<n>`. This names the held inode from any process of the same user, also from the terminal emulator that Unpin starts. This is the same `/proc` access rule that `launch_exe` relies on today for `/proc/<pid>/exe`.
- If no sibling `mbv` existed at start, send the desktop notification that names the missing program and start nothing (`pin-swap` delta).

A TUI started this way keeps today's `launch_exe` behaviour for its own `--pin` relaunch.

Alternative considered: launch the sibling `mbv` path every time. Rejected: after a package upgrade it starts a newer TUI build, which the attach gate refuses.

### D10. Packaging and build

- `PKGBUILD`, `PKGBUILD-git`, the tarball step in `.github/workflows/build.yml`, and the `.deb` assets install `mbv-owner` beside `mbv` and `mbv-gui`.
- The `.deb` metadata moves from the root package to `crates/mbv-tui/Cargo.toml` with `name = "mbv"` in `[package.metadata.deb]`, and asset paths become relative to that crate. CI runs `cargo deb -p mbv-tui --no-build`.
- `Makefile` GUI targets build `-p mbv-tui -p mbv-owner -p mbv-gui`.
- `tools/release.sh` bumps the version in the root `[workspace.package]`, which it already edits. Check that its `sed` pattern still matches only that line once the root `[package]` is gone.
- The CI ast-grep rule `mbv-dispatch-no-wildcard-arm` changes its glob from `src/app/dispatch/**` to `crates/mbv-client-core/src/app/dispatch/**`.
- The unsafe-site audit comment in `build.yml` updates its file paths.

### D11. Dependencies

No new external dependency, and no version change. Each existing workspace dependency moves with its code at the version in `[workspace.dependencies]`:

- `ratatui` 0.30, `crossterm` 0.29, `tuirealm` 4.1, `ratatui-image` 11, `tui-scrollbar` 0.2.7: `mbv-tui`, `mbv-tui-support`, `mbv-components`, `mbv-render` only.
- `mimalloc` 0.1: `mbv-tui` and `mbv-owner` (both binaries keep the allocator).
- `nix` 0.31, `libc` 0.2: where the moved FFI sites go (`mbv-owner`, `mbv-tui`, `mbv-client-core`, `mbv-desktop`).
- All internal crates are path dependencies at the workspace version.

### D12. Docs for agents

- Root `AGENTS.md`: the workspace map and the shared core only. It points to each Client's guide.
- New `crates/mbv-tui/AGENTS.md` (TUI rules, the TuiRealm and component rules, the TUI tests matrix) and `crates/mbv-gui/AGENTS.md` (GUI rules). Text that applies to one Client moves out of the root guide into that Client's guide.
- New ADR `docs/adr/0042-clients-are-separate-crates-over-one-client-core.md`. It amends ADR 0035 and ADR 0039 (the GUI links the core, not the TUI) and records D4, D6a, D8, D9, and D13.
- `CONTEXT.md`: add *Owner program*; update *Client core*, *Owner process*, *Local process* to name their crate and program.
- Update `docs/architecture/tui-frontend.md`, `docs/architecture/gui-client.md`, `docs/architecture/owner-client-operation-split.md`, `docs/architecture/interactive-tui-component-map.md`, and `docs/recipes.md` site lists to the new paths.

### D13. Items in `mbv-render` and `mbv-components` that shared or GUI code uses

`mbv-render` and `mbv-components` stay TUI-only. Each item they hold that the core or the GUI uses goes by one of two rules.

**Rule A: toolkit-free shared logic and data move down to `mbv-ui-model`.** Both Clients import them from there, and `mbv-render` and `mbv-components` import them too.

| Item | From |
|---|---|
| `current_time_secs` | `mbv-render` `screens/feeds_model.rs` |
| `group_album_plan`, `sorted_group_album_order`, `initial_group_artist_sort_key`, `effective_sort_str` | `mbv-render` (`screens/album_plan.rs` and the files `lib.rs` re-exports them from) |
| `book_rows` | `mbv-render` `components/audiobookshelf_book.rs` |
| `HeroImageState` | `mbv-render` `components/tv_wide.rs` |
| `MUSIC_ALBUM_IMAGE_TYPES` | `mbv-render` `components/widgets.rs` |
| `IndicatorStyle`, `StatusIndicator` (data only) | `mbv-render` `components/indicators.rs` |
| `TransportAvailability` | `mbv-render` `components/chrome_player.rs` |
| `hero_content_queue`, `hero_content_feed`, `hero_content_emby`, `hero_content_abs_episode`, `hero_content_abs_book` | `mbv-components` `library_panel/hero.rs` |
| `HeroArtwork`, `ArtworkSource`, `ArtworkShape`, `HeroFacts`, `PanelHeroImagePaint` (data only) | `mbv-components` `library_panel/content.rs` |
| `premiere_number_label` | `mbv-components` `tv_content/episode_rows.rs` |
| `sanitize_url` | `mbv-components` `library_panel/overview_box.rs` |

Where an item's type holds a ratatui value, the item splits: the neutral data moves to `mbv-ui-model`, and the ratatui part stays in its TUI crate as a conversion. Painting code never moves down.

**Rule B: TUI terminal geometry stays in the TUI crates, and its readers move to `mbv-tui`.** If a core rule needs the result, the TUI pushes it to the core as a neutral value (as with the single-panel fact in D6a).

`MINI_VIEW_THRESHOLD`, `TABBAR_LEFT_RESERVE`, `LEFT_WIDTH_DEFAULT`, `rect_to_presentation`, `LIBRARY_PILL_THRESHOLD`, `chrome_geometry`, `ChromeGeometryInput`, `tab_strip_text_width`, `wide_library_panes`, `normalize_queue_column_width`, `wide_hero_fits`, `visible_tab_range`, `service_state_color`, `components::card`, `hero_artwork_box`, `MusicWideRenderCtx`, `LibraryListRenderCtx`.

An item that the compiler finds later goes by the same two rules. After the split, `cargo tree -p mbv-client-core` and `cargo tree -p mbv-gui` list neither `mbv-render` nor `mbv-components`.

## Risks / Trade-offs

- [The move touches most of the workspace, so the diff is very large] → Move in compile-green steps (tasks): first the leaf-crate cuts, then the core crate, then each Client. Each step builds and tests green before the next.
- [Making `App` reads `pub` widens the core API] → Add a read method only where a Client needs one. Never make a field `pub`.
- [Tests move with code and some depend on private items] → A test stays a `src/` test of the crate whose private items it uses. Public-API tests go in the crate's `tests/<name>.rs` binary. No visibility widens to keep a test.
- [The held `mbv` file pins an old inode on disk until the Owner exits] → Same cost as today, where the Owner's own running file pins its inode.
- [`/proc/<pid>/fd/<n>` needs the same-user `/proc` access that `/proc/<pid>/exe` needs today] → No new access rule. Hardened kernels that block it today already block the current path.
- [A dev build that runs `mbv-gui` alone finds no fresh `mbv-owner`] → The `Makefile` GUI targets build all three. The mismatch prompt still catches a stale sibling.
- [A move row names items that the core still uses, so its check cannot pass] → Each move row states its seam (D4a) before it names a move. A row's `rg` check is tested against the code before the row is dispatched.
- [Compile time] → `mbv-gui` stops compiling ratatui, crossterm, and TuiRealm. `mbv` stops compiling GUI modules.

## Migration Plan

- An Owner from an earlier release runs as `mbv --__local-daemon`. A Client from this release finds it, the gate sees a different `app_version`, and the mismatch prompt offers restart. Restart stops it and starts `mbv-owner`. No state migration: the Owner's persisted state, sockets, and lock path do not change.
- Rollback: install the earlier package. Its `mbv` finds the newer `mbv-owner` Owner and offers restart in the same way.
