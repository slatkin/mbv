# Design: add-tauri-gui-client

## Context

See `proposal.md` for the reason and `specs/gui-client` for the requirements.

The current state that shapes the approach (after `extract-client-core`, PR #984):

- The root `mbv` package has a library target. It exports `App`, `ClientCore`, `ClientTickHost`, `ClientWaker`, `CLIENT_IDLE_CADENCE`, the slice projections (`QueueSurfaceProjection`, `QueuePlaybackProjection`, `QueueFooterProjection`, `StatusBarProjection`), the GUI routing entry (`GuiInput`, `GuiSurfaceLeaf`, `App::route_gui_input`, `App::route_gui_pointer`), and the Owner steps (`find_owner`, `start_owner`, `attach_owner` with its `owner_exe` override, `declare_gui_client`).
- `mbv-surfaces` holds the Surface state of the slice, the focus stack, `HitMap`, the pointer gesture policy, and the shared breakpoint classes.
- Every Client honours the `client-interaction-core` spec: behaviour lives once, neutral input, capacity in items, and a per-frame semantic hit map. A Client that presents Library or hero placement also honours the shared breakpoint classes; the GUI canvas slice derives none.
- Tauri 2 owns the main thread and its event loop (tao, then GTK on Linux). Its command handlers and event emitters run under tokio. On Linux the webview is WebKitGTK 4.1, which draws text through fontconfig and FreeType.

## Goals / Non-Goals

**Goals:**

- A GUI Client that presents the player panel of the design canvas (D15), with the keyboard behaviour of the TUI, through the shared Client core.
- GUI text in the bundled IBM Plex Sans and JetBrains Mono faces, rasterized by the webview through FreeType and fontconfig; Lucide icons.
- A thin front end: it paints projections and reports input, capacity, and hit regions. It holds no interaction rules.

**Non-Goals:**

- Surfaces outside the canvas, the visualizer, `--pin`, tray, GUI-specific MPRIS work, packaging. The proposal lists them.
- Any change to ctrl, the Owner process, mpv, or the TUI.
- A web or remote UI. The webview loads only bundled assets.

## Decisions

### D1. Crate and front-end layout

```text
crates/mbv-gui/            Tauri 2 binary `mbv-gui`; links the `mbv` library
  src/main.rs              Tauri builder, command registration, core thread start
  src/core_thread.rs       the Client core loop (D3)
  src/bridge/              serde DTOs for projections and input (D4), key translator (D6)
  ui/                      Svelte 5 + Vite front end (pnpm), built into Tauri's frontendDist
```

`mbv-gui` is a workspace member and not in `default-members`, so the default build and nextest skip it. CI clippy and fmt cover it through `--workspace`.

*Alternative:* SvelteKit with `adapter-static`. Rejected: the GUI is one window with no routes, so SvelteKit adds a router and a build adapter for nothing. shadcn-svelte supports plain Vite.

### D2. Versions

Every dependency starts at its latest stable release, checked on 2026-11-06: `tauri` 2.12.5 (MSRV 1.90) and `tauri-build` 2.7.4; `@tauri-apps/api` and `@tauri-apps/cli` 2.12.4; `svelte` 5.57.3; `vite` 8.3.6 with `@sveltejs/vite-plugin-svelte` 7.3.1; `tailwindcss` 4.3.4; `shadcn-svelte` 1.8.0 (on `bits-ui` 2.20.6); `@lucide/svelte` 1.70.0; `vitest` 5.1.0 with `@testing-library/svelte` 5.4.2; `svelte-check` 4.7.8; `@fontsource/ibm-plex-sans` and `@fontsource/jetbrains-mono` 5.3.0; pnpm 12. The peer ranges of these releases fit one another (checked 2026-11-06).

One exception: `typescript` is 6.0.3, the latest 6.x, not the latest release 7.0.2. `svelte-check` 4.7.8 accepts only `typescript` `^5.0.0 || ^6.0.0`, and the type check is a gate (task 1.3). Move to 7.x when a `svelte-check` release accepts it.

The scaffold uses exactly these versions. A peer conflict that appears at scaffold time is a design gap: the implementer reports it and does not pick a version.

### D3. The Client core runs on its own thread

The Tauri main thread runs the window event loop. One dedicated, synchronous **core thread** creates the `App`, attaches to the Owner, and runs `ClientCore::tick` in a loop. Tauri command handlers never touch the `App`. They push a `CoreInput` value into a `std::sync::mpsc` channel to the core thread. The core thread waits on that channel with a timeout of `CLIENT_IDLE_CADENCE`. The installed `ClientWaker` sends a wake value into the same channel, so Owner events and timers wake it as they wake the TUI.

After each tick that changed something, the core thread builds a `GuiFrame` (D4) and emits it to the window with `AppHandle::emit`, which is safe to call from any thread. tokio stays inside Tauri. The core thread and the `mbv` library never see it.

The core thread also emits the current frame, changed or not, after every `report_layout`. The front end sends `report_layout` when it mounts. So the first frame, and the frame after a reload of the window content, never wait for a state change.

*Alternative:* hold the `App` in Tauri managed state behind a `Mutex` and tick from a timer. Rejected: command handlers on tokio workers would then run `App` code, and the tick would depend on an async timer.

### D4. The bridge contract

Rust to front end, one event `mbv://frame` that carries a `GuiFrame`:

- the focus (surface and Panel);
- the queue rows inside the current viewport with their stable `QueueSlotId`, cursor, marks, the active slot, watched and partial-progress state, and the right-column value of each row in both modes (length and finish clock);
- the queue summary: remaining count, queue finish clock, the right-column mode, and the filter text;
- the playback projection: title and parent line, season and episode badge, artwork, playback state, elapsed and remaining time and the finish clock, chapters with intro and credits marks, the trickplay source, and the transport state;
- the playback target and the cast choices;
- the track chips: the video stream's resolved label, and the audio and subtitle tracks each with its choices and the chosen one;
- the open menu or popover and its items, with key hints from the keybind registry;
- the collapse and queue-fold toggles;
- the footer line: the queue source, the session marker, the prefix-armed state, and the toasts;
- the theme (D7) when it changes.

Each projection is a serde DTO in `mbv-gui/src/bridge/`, built from the library projections. Where a library projection lacks a field, the change adds it to the projection first (the source-of-truth type), not to the DTO alone. The front end keeps only the last frame. The footer MAY show the toasts in a conventional GUI form that TUI geometry does not constrain.

#### D4a. Data sources

The source of every datum the frame carries. "Held" means the Owner, the `App`, or the Client's Surface state supplies it today, and the task only projects it. "New core state" means a datum the change introduces on the Client core; it needs no outside source. "GAP" means nothing held it when this table was written; each gap row records the source and the user's ruling.

| Datum | Source | Status |
| --- | --- | --- |
| Focus (surface, Panel) | `FocusStack` (`mbv-surfaces`, GUI core) for the surface; `App`'s `PanelFocus`/`PanelMode` (`mbv-ui-model::settings`, read in `RouterSnapshot`) for the Panel | Held |
| Queue rows in viewport | `QueueSurface` (`mbv-surfaces`) adopting `QueueSurfaceProjection` (`src/app/queue_surface.rs`): `QueueSlot.slot_id`, `QueueSlot.item` | Held |
| Cursor, marks, viewport | `QueueSurface`/`MediaList` state (`mbv-surfaces`), projected by the frame | Held |
| Active slot | `PlaybackState.active_idx` resolved to `QueueSurfaceProjection`'s active target | Held |
| Watched state per row | `QueueItem::played()` (`mbv-queue/src/items.rs`) | Held |
| Partial progress per row | `QueueItem::playback_position_ticks()` against `runtime_ticks()`; the active row's live value from `QueueSurfaceProjection.patch` | Held |
| Right column, "Length" mode | `QueueItem::duration()`/`runtime_ticks()` per slot, formatted with the `mbv-ui-model` duration helpers | Held |
| Right column, "Ends at" mode; queue finish clock; scrubber finish clock | Computed: wall clock + `PlayerStatus.position_ticks`/`runtime_ticks` + the remaining `QueueSlot` runtimes. New projection arithmetic, no new datum | Held (derived) |
| Queue remaining count | Derived: slots after the active index | Held (derived) |
| Right-column mode | New Client-core toggle (the TUI has no such mode) | New core state |
| Queue filter text | New Client-core text state; it filters the projected rows locally, never the canonical queue | New core state |
| Title, parent line | `App::playback_title_parts` → `PlaybackTitleParts` (`mbv-queue/src/items.rs`): `title` and `context` | Held |
| Season and episode badge | `EmbyItem.item_type == "Episode"` with `parent_index_number` (season) and `index_number` (episode) (`mbv-emby-model/src/lib.rs`) | Held |
| Artwork | `App::queue_card_artwork` (`src/app/queue_card_artwork.rs`) → display-encoded JPEG; Emby and Audiobookshelf covers through `src/app/infra/image_fetch` | Held (fill path below) |
| Playback state, elapsed, remaining | `QueuePlaybackProjection.playback` (`PlaybackState` over `PlayerStatus.position_ticks`/`runtime_ticks`/`paused`) | Held |
| Transport state | `QueuePlaybackProjection.availability` (`TransportAvailability`) plus `PlaybackState.paused` | Held |
| Chapters (scrubber boundaries, hover name) | Emby `Chapters[]` (`Name`, `StartPositionTicks`) on the item DTO, fetched by `ItemId`; Audiobookshelf `GET /api/items/{id}?expanded=1` `media.chapters` through `AudiobookshelfClient::book_detail` (already in `mbv-audiobookshelf`) | GAP — ruled: approved as a Client-side fetch by the active `ItemId`, decoration only |
| Intro and credits marks | `GET /MediaSegments/{itemId}` (segment `Type` Intro/Outro with tick bounds); requires a server that has detection data. The player plumbing exists (`PlaybackRun.intro_start`/`intro_end`, `IntroState`, `PlayerEvent::IntroStarted`, `PlayerCommand::SkipIntroDismiss`) but is seeded with 0 since the dead Chapter-API fetch was removed (546dba398) | GAP — ruled: approved as a Client-side fetch by the active `ItemId`, decoration only |
| Trickplay frames | Emby `GET /Videos/{itemId}/Trickplay` (`TrickplayInfo` tile grids) and `GET /Videos/{itemId}/Trickplay/{width}/{index}.jpg` for the tiles, fetched on the Rust side and carried as bytes per D11 | GAP — ruled: approved as a Client-side fetch by the active `ItemId`, decoration only |
| Playback target | `App::playback_host_label_and_remote` (`src/app/state/projection/chrome_status.rs`): the attached output's name, the remote route/label, else `mbv_emby::device_name()` | Held |
| Cast and session choices | `App.sessions` (Emby `/Sessions`, `SessionEvent::Loaded`) merged with `App.cast_receivers` (`spawn_cast_discovery` → `mbv_cast::discovery::browse_cast_receivers`) by `build_panel_targets` (`src/app/state/panel_targets.rs`), gated on `supports_playback_output` | Held (fill path below) |
| Audio chip: choices, chosen | `PlayerStatus.audio_tracks` (mpv id + label), `audio_id`, `audio_lang`; `PlayerCommand::SetAudio` switches | Held |
| Subtitle chip: choices, chosen | `PlayerStatus.sub_tracks` (mpv id + label + forced), `sub_id`, `sub_lang`, `sub_track_stream_indexes`; `PlayerCommand::SetSub` switches, `cycle_sub` cycles | Held |
| Video chip: label | `PlayerStatus.video_height` → `short_resolution_label`, `audio_codec`, `video_is_image` (the `IndicatorData` resolution of `LocalPlaybackTarget::indicator_data`) | Held |
| Video chip: choice list | Nothing holds it: mpv's `track-list` video entries are not projected into `PlayerStatus`, and most items carry exactly one video stream | GAP — ruled: the chip shows the resolved label only and opens no popover |
| Open menu or popover, items | Row-menu semantics on `ContextMenu`/`ContextMenuEntry`/`ContextAction` (`src/app/dispatch/context_menu/menu.rs`): the canvas's Play now, Mark watched, Go to episode, Remove map to `PlayQueue`, `MarkPlayed*`, `GoToLibrary`, `RemoveFromQueue`; "Play next" is a new `ContextAction`-level action. The open popover (a track list, the menu) is new Client-core state | Held + new core state |
| Menu item key hints | `mbv-keybinds`' `Keybinds`: the configured chords per action id, default chords from the registry | Held |
| Collapse (`z`) and queue-fold (`q`) toggles | New Client-core state; the spec assigns it to the Client core | New core state |
| Footer: queue source, owner, autosave, dirty | `QueueFooterProjection` pills over `playback_queue().source()` and `App::queue_owner_label` | Held |
| Footer: session marker ("woims ●") | `App::queue_owner_label`/`playback_host_label_and_remote`: `mbv_emby::device_name()` (the host name) for the local Owner, the peer route or remote label otherwise; the dot marks the live Owner | Held |
| Footer: prefix armed, toasts | `StatusBarProjection` (`App.prefix_armed`, `toast_presentation`) | Held |
| Theme | `mbv-theme` `Role::color` and `surface_colors` → `GuiTheme` | Held |

**Non-Emby kinds.** The badge is an Emby-episode datum only: `AudiobookshelfQueueItem`, `FeedEntry`, and `TwitchItem` carry no season or episode ordinal, so their artwork shows no badge. The parent line is `PlaybackTitleParts.context` per kind: an Emby episode shows `series_name`, an Emby audio item the track artist then the album artist, a Feed entry the configured subscription's name (`App::feed_subscription_display_name` against `config.feeds`), an Audiobookshelf episode `show_title`, and an Audiobookshelf book or Twitch item no line. Chapters exist only for Audiobookshelf (`media.chapters`, through the existing `book_detail` call); Feed and Twitch items carry none and the scrubber shows no boundaries. The track chips are kind-agnostic: they read `PlayerStatus`, which reports mpv's track list for whatever file is loaded, so an audio-only item shows no video chip and an item with no subtitles shows the subtitle chip as Off.

**Remote outputs.** The chips' source is the local player's status, so while playback rides a Cast receiver they have no data, as the TUI's indicator cluster is absent (`status_indicators` is `None` for remote output). An Emby Session output can supply its lists from `SessionInfo.media_info` (`SessionMediaInfo.audio_streams`/`subtitle_streams`/`video_label`), already in `App.sessions`.

**Fill path (ruled: approved).** `App.sessions`, `App.cast_receivers`, the Emby artwork fetch, and the approved Emby metadata fetches (chapters, marks, trickplay) all need an Emby client, and `App::emby_client` reads `emby_runtime.client`, which `spawn_startup_services` fills — a shell step the GUI core does not run. Ruled: the GUI core runs a minimal fill of `emby_runtime.client` and the session and cast discovery it feeds, not the library bootstrap; D9 records the decision. Audiobookshelf fetches need no service runtime: `AudiobookshelfClient::new(server_url)` plus the configured credential suffices, as the cover fetch in `src/app/infra/image_fetch/protocol.rs` does.

**Key-chord collision (ruled).** The canvas chords collide with the TUI defaults: `z` is `cycle_subtitle`'s chord, `q` is `quit`'s, and `c` is `clear_queue_prompt_c`'s. Ruled: the canvas's `z` (collapse), `q` (queue fold), and `c` (cycle subtitle) bind in a GUI-only key context, so a configured TUI binding for `z` or `q` does not follow into the GUI; the TUI Client's default bindings stay unchanged.

**Gap rulings:**

1. Chapters and the intro/credits marks (GAPs): ruled approved — `Chapters` and `/MediaSegments` fetches on the Client keyed on the active `ItemId`, cached like artwork; presentation data, so no Owner change. The dormant `intro_start`/`intro_end` plumbing stays dormant.
2. Trickplay (GAP): ruled approved — the same Client-side fetch against `/Videos/{itemId}/Trickplay`; absent data falls back to the time and chapter preview the spec defines.
3. Subtitle offset (GAP): ruled rejected — no subtitle-offset datum or control ships in this slice; the Non-Goal "any change to ctrl, the Owner process, mpv" stands.
4. Video-chip choice list (GAP): ruled — the chip shows the resolved stream label only and opens no popover; revisit when a multi-source item needs a choice.

Front end to Rust, three commands:

- `key(DomKey)`: the DOM `KeyboardEvent` fields (`key`, `code`, modifier flags, `repeat`). The Rust side translates them to a neutral `KeyEvent` (D6).
- `pointer(DomPointer)`: kind (press, release, move, wheel), button, the position in logical pixels relative to the window, and the wheel delta. A move with a button held carries a drag, and a move over the scrubber carries the hover. The core resolves both against the hit map.
- `report_layout(LayoutReport)`: for each list, the number of whole visible rows, the painted regions with their semantic target ids, and the container size of the panel. The front end sends it after each paint that moved a region (a Svelte `$effect` after the DOM update, plus a `ResizeObserver`).

The core thread applies a `LayoutReport` to the Surface state's capacity and `HitMap`. `App::route_gui_pointer` resolves a gesture only against the leaf it receives. So the core thread first resolves the point against one combined `HitMap` of all painted slice surfaces from the last `LayoutReport`. Then it applies the focus change that the matching TUI click applies, and passes the surface that owns the region as the leaf to `App::route_gui_pointer`. A point in no region resolves to nothing (`client-interaction-core`: pointer targets resolve from a semantic hit map). A press on a transport control therefore works while the Queue panel has focus. The front end never decides what a click means.

*Alternative:* the DOM resolves the clicked element and sends the target id. Rejected: it moves gesture rules (double-click window, range selection) into TypeScript, and the spec requires that the core resolve against the hit map.

### D5. Lists paint only the viewport

A GUI list is a fixed-row-height column with no native DOM scroll. The core's viewport decides which rows the frame carries. Wheel input goes to the core like any pointer event. So scroll follows the cursor through the same Surface state rules as the TUI. The front end measures the list's height and reports whole rows as capacity.

Row drag follows the same path. The front end paints the insertion line and the drag ghost from the frame. The core owns the drag state (the dragged slot and the drop index) and, on release, sends one move request to the Owner process. `Alt+J` and `Alt+K` send the same request through the router.

*Alternative:* native DOM scrolling with a virtual list. Rejected: the DOM would then own scroll position, which is a second scroll authority.

### D6. Keys translate on the Rust side

The DOM key fields are translated to `KeyEvent` in Rust, in `bridge/keys.rs`. A `#[case]` table test covers the mapping, as the TUI's crossterm translator has. The front end prevents the default action for every key while the window has focus. Before it does, it offers no shortcut of its own, so webview defaults such as reload, find, zoom, and back navigation never run.

*Alternative:* translate in TypeScript. Rejected: then two languages own the translation tables, and the Rust nextest gate would not cover them.

### D7. Theme

`mbv-theme` resolves each role to a neutral `Rgb`. The frame carries a role-to-colour map plus a chrome section: the fill values the shadcn-svelte chrome variables (`--background`, `--card`, `--popover`, `--muted`, `--secondary`, `--sidebar`, `--border`, `--input`, and the sidebar aliases) point at. Each chrome fill resolves the `Surface` the TUI paints the same concept with, through `surface_colors(surface, false)`; the `mbv-theme` surface table is the one fill authority. `Role` stays foreground-only — the chrome fields are fill resolutions, not new `Role` variants. The front end writes roles and chrome fills into `--mbv-<name>` custom properties on `:root`, and the shadcn-svelte theme variables (`--foreground`, `--primary`, `--accent`, `--ring`, and the rest) point at mbv properties. shadcn-svelte components therefore paint in the mbv palette with no second palette.

### D8. Fonts and icons

The canvas sets the faces: IBM Plex Sans (400, 500, 600) for text, and JetBrains Mono (400, 500, 600) with tabular figures for times, key hints, chips, and counters. The front end bundles them (`@fontsource/ibm-plex-sans` and `@fontsource/jetbrains-mono`, latin subsets, woff2), with `system-ui` and `monospace` as fallbacks. WebKitGTK still renders through fontconfig and FreeType. The woff2 files build into `ui/dist` with the rest of the bundle, so the window fetches no font at runtime (the canvas loads them from Google Fonts; the build does not) and the CSP's `default-src 'self'` covers `font-src`. Icons come from `@lucide/svelte`. The GUI paints no emoji or Nerd Font glyph. The playback-target indicator, the transport controls, the grip, and the playing-row marker are Lucide icons.

*Earlier choices:* the system UI font, then SUSE (2026-11-06). Both are superseded by the design canvas (D15). The implementer replaces the SUSE font package (`@fontsource-variable/suse`) with the two fontsource packages, pins them at their latest stable releases, and records the versions in D2.

### D9. Owner launch

The core thread calls `declare_gui_client`, then `find_owner`. When no Owner runs, it starts one from the `mbv` program: first from the directory of the `mbv-gui` executable, then from `PATH`, through the `owner_exe` override of `attach_owner`. The Owner is never the GUI program.

Each attach outcome gets the response that `startup::run_local_instance` gives the TUI Client. The core thread classifies an attach error with the public `RemotePlayerError` predicates:

- `OwnerState::Refuse` (the Owner holds the lock but does not accept connections): the window shows `refusal_text()`.
- `is_exclusive_owner` (Stay-alive disabled and a Client attached): the window shows `exclusive_owner_refusal_text(&error)`.
- `is_owner_shutting_down`: the window shows a waiting state. The core thread waits for the Owner to exit, then starts an Owner and attaches.
- `is_owner_build_mismatch`: the window shows the mismatch text with **Restart** (key R) and **Quit** (key Q). Restart calls `mbv_daemon::signal_owner` on the owner lock path, waits for the Owner to exit, starts an Owner, and attaches. A mismatch after a requested restart waits again, as the TUI does. Quit closes the window.
- Any other error: the window shows "failed to attach to Owner process: {error}".

Before attach there is no `App`. In these states the core thread handles R and Q itself, and the two buttons resolve through the `LayoutReport` regions. The front end still interprets no key.

The core thread does not run the TUI's post-attach service bootstrap. `spawn_startup_services`, `fetch_home_at_startup`, `init_idle_feed`, and capability registration (`src/app/shell/run/drains.rs:521`, `src/app/shell/run.rs:319-324`) belong to the TUI shell `Model`, not to `App`. The first slice presents no library surface (Non-Goals), so the core thread does not run those steps, and a configured-but-uncached provider does not bootstrap for the GUI Client. A later change that ports a library surface must first factor those steps into a shared Client step; it must not duplicate them in `mbv-gui`.

One ruled exception (D4a, "Fill path"): the GUI core fills `emby_runtime.client` — the minimal Emby client fill that feeds the session list, cast discovery, artwork, and the item-metadata fetches of D4a — without the library bootstrap. This is a fill of `App` state, not `spawn_startup_services`, so it does not contradict the rule above.

### D10. Panel states

The canvas switches the panel between four states (expanded, compact strip, expanded without queue, rail) on the panel's own size, not on the window size. The front end SHALL pick the layout from the measured size of the panel container, the same measurement that it reports to the Client core in `LayoutReport`, so the painted layout, the list capacity, and the hit map follow one size. This is presentation: it holds no interaction state. The container size goes to the core in `LayoutReport` so that the hit map and the list capacity match the painted layout. The `z` (collapse) and `q` (queue fold) toggles live in the core and arrive in the frame.

The shared TUI breakpoint classes (`Mini`, `Narrow`, `Wide`) are not used by the GUI slice. They differ only in Library and hero placement, and the canvas presents neither. A later change that ports a surface with Library placement decides then how it maps to the panel states.

*Earlier choice:* classify the window width with the shared breakpoint classes and a GUI threshold constant. Superseded by the canvas.

### D11. Webview hardening

The front end loads only the bundled assets. The CSP allows only `'self'` and Tauri's IPC origin. The Tauri capability file allows only the three mbv commands and the event listener. The webview's own context menu is turned off. Artwork and trickplay frames reach the window as bytes from the Rust side (an `img-src` of `'self' data: blob:` or a custom protocol, whichever the implementer picks), never as a remote URL, so the CSP still allows no remote origin.

### D12. tokio edge

`mbv-gui` joins `mbv-desktop` in the AGENTS.md list of tokio edges. Only Tauri's own runtime uses tokio. No mbv code in `mbv-gui` is `async`, apart from what a Tauri command signature requires.

### D13. Tests

- Rust (nextest, `crates/mbv-gui/tests/`): the key translator table; `LayoutReport` applied to capacity and the hit map; a `GuiFrame` built from a stubbed `App` (the root `test` feature) for a queue with slots and for an empty queue; a core-thread step test with a mocked channel: input in, frame out, and a waker wake causes a tick; a press on the play/pause region while the Queue panel has focus dispatches the playback request (D4); and one `#[case]` table that maps each attach outcome to its window state (D9). No Tauri runtime, no webview, and no live Owner.
- Front end (vitest with `@testing-library/svelte`, `@tauri-apps/api` mocked): a component renders a given frame, and a key or pointer event calls the right command with the right payload. These tests assert delivery and structure, not appearance.
- Add to the Rust tests: the drag state (drop index from a pointer move, one move request on release) and the menu and popover items with their key hints. Add to the front-end tests: each panel state renders from a frame and a container size, and a press on the scrubber, a chip, a menu item, and the row-actions button calls `pointer` with the right payload.
- Live behaviour in the real window is the user's manual check.

### D14. Build and CI

`tauri::generate_context!` panics at compile time when the `frontendDist` directory does not exist (`tauri-codegen`, `src/context.rs`). Every `--workspace` build compiles `mbv-gui`, so it needs WebKitGTK 4.1 and a built `ui/`. The `build` and feature-matrix jobs in `.github/workflows/build.yml` add `webkit2gtk-4.1` and pnpm to their pacman installs, and build `ui/` before `cargo clippy --workspace` and `cargo hack check --workspace`. CI nextest runs only `default-members`, so a new GUI job runs `cargo nextest run -p mbv-gui` and the front-end check, lint, and test targets. The local `cargo clippy --workspace` gate has the same prerequisites.

### D15. The design canvas is the target

The target is the design canvas "mbv Svelte Player" (https://claude.ai/artifact/D8uLGS6NyCeHd6ed7PHgsr). It is a visual and structural target. Its inline-style markup is not copied. The front end builds the same layout from Svelte 5 components, shadcn-svelte parts where they fit (popover, menu, toggle group), and Tailwind, painted through the theme variables of D7.

| Artboard | What it fixes | Spec requirement |
| --- | --- | --- |
| Queue panel — expanded | header, artwork, title, scrubber, transport, track chips, queue, footer; 30 px rows; Length / Ends at; finish clocks | player panel; scrubber; queue |
| Collapsed states | compact strip, expanded without queue, 72 px rail; switching on the panel container's measured size; `z` and `q` | panel states |
| Track popover, row menu, drag | subtitle popover; row hover, grip, actions button, menu with key hints; drag ghost and insertion line; `Alt+J` / `Alt+K` | track chips; queue |

Where the canvas and a TUI behaviour disagree, the canvas wins. Where the canvas is silent, keyboard behaviour follows the TUI through the one router, and a missing visual (the empty-queue state, the attach states of D9) follows the canvas's type, colour, and spacing.

The canvas draws some things as static mockup: the hover preview, the open popover, the open menu, and the drag. They are states of the same components, driven by the frame (D4).

## Risks / Trade-offs

- [WebKitGTK on some GPU and Wayland setups shows a blank window (the DMA-BUF renderer)] → The user's live check covers their setup. The workaround, `WEBKIT_DISABLE_DMABUF_RENDERER=1`, is documented. The app does not set it by default.
- [IPC per key and per pointer event adds latency] → Tauri IPC is local and takes well under one frame. The spec requires only next-frame response.
- [`LayoutReport` traffic on resize] → The front end sends it only when a region moved, at most once per animation frame.
- [TypeScript stays one major behind its latest release] → D2 records the reason (the `svelte-check` peer range) and the condition to move.
- [Two toolchains (cargo and pnpm)] → `make` targets wrap the pnpm steps. D14 sets the CI jobs.
- [The GUI links ratatui, crossterm, and TuiRealm through the `mbv` library] → Accepted, as in `extract-client-core` D2.
- [The front end grows its own behaviour over time] → The `gui-client` requirement "The window content holds no interaction state" and the reload scenario make that drift a spec failure.

## Migration Plan

There is no user data or protocol migration. `mbv-gui` is not packaged. Implementation starts after PR #984 merges. The order is: crate and front-end scaffold, the core thread and bridge, the slice components, theme and icons, then docs. Groups 1 to 5 of the tasks were built to the TUI-parity target. Group 6 retargets that code to the canvas. Rollback is to remove `crates/mbv-gui`.

## Open Questions

- ~~Where each new datum comes from (D4).~~ Answered in D4a: every datum names a held source, a new-core state, or a ruled gap. The gap rulings are recorded at the end of D4a (chapters, marks, and trickplay approved; the subtitle offset rejected; the video chip label-only).
- ~~Audiobookshelf and Feed items: how the badge, the parent line, the chapters, and the track chips map for those kinds.~~ Answered in D4a ("Non-Emby kinds"): no badge, `PlaybackTitleParts.context` for the parent line, chapters only for Audiobookshelf, kind-agnostic chips.
- ~~The meaning of the footer's right-hand marker ("woims ●" on the canvas).~~ Answered in D4a: the Player-owner label (`queue_owner_label`/`playback_host_label_and_remote`; `mbv_emby::device_name()` for the local Owner) with its live-state dot.
- The canvas palette is Everforest-like. Confirm that the default `mbv-theme` palette resolves to the same values, so that D7 reproduces the canvas by default.
- The empty-queue state and the attach states are not drawn on the canvas (D15).
