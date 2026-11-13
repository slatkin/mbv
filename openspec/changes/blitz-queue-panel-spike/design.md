# Design

## Context

See `proposal.md` for the reason. This section gives the state the design depends on, all checked on 2026-11-13.

**The GUI core is not on `main`.** `gui-change-driven-sections` lives on `feat/gui-change-driven-sections`, 65 commits ahead of `main`. The core thread, the attach campaign, the section ledger, the art vocabulary, and the `mbv-art` path are on that branch. This change starts only after that branch merges.

**The GUI core is Tauri-free.** Outside `crates/mbv-gui/src/main.rs`, the Rust modules mention Tauri only in comments. The `tauri` dependency and `tauri-build` belong to the package. A library dependency on `mbv-gui` therefore builds `tauri` and WebKitGTK.

**The core already exposes what the window needs.** `core_thread::spawn(sink, quit)` returns a `Sender<CoreInput>`. The `SectionSink` carries `GuiSectionBatch`. `CoreInput::Art(ArtRequest { path, reply })` answers an image request through a reply closure. `attach::AttachCampaign` is the attach path, and `declare_gui_client()` marks the process as a non-participant in Pin swap. `App::gui_art_source` and `App::trickplay_sheet_bytes` are the read-only art queries.

**The Queue panel's CSS uses features Blitz may not paint.** The Queue panel reads `--canvas-*` tokens from `app.css`, which is compiled Tailwind v4 with `oklch`, `color-mix`, `:has()`, and `@apply`. The Queue rows use `truncate`, `tabular-nums`, and `font-mono`. The scrubber uses `translate`, and the playing band is a 3 px spread `box-shadow` ring. The icons are Lucide SVGs. The fonts are IBM Plex Sans and JetBrains Mono from `@fontsource` 5.3.0, Latin subset, weights 400, 500, and 600.

**Blitz has no version with a CPU renderer on Linux in its stable line.** See D1 and D2.

### Sources

- Blitz repository, README, `AGENTS.md`, `CONTRIBUTING.MD`, and `docs/licensing.md` (licensing review dated 2026-10-07): https://github.com/DioxusLabs/blitz
- `dioxus-native` 0.7.10 manifest at tag `v0.7.10`: https://github.com/DioxusLabs/dioxus/blob/v0.7.10/packages/native/Cargo.toml
- `dioxus-native` 0.7.10 and 0.8.0-alpha.3 on crates.io: https://crates.io/crates/dioxus-native/0.7.10 and https://crates.io/crates/dioxus-native/0.8.0-alpha.3
- Dependency resolution, measured with `cargo tree` and the lockfile of a probe crate outside this repo (2026-11-13).
- Blitz issues, cited inline in the gap list below.

## Goals / Non-Goals

**Goals:**

- Build one native window that paints the Queue panel from the GUI's core, in process.
- Produce a measurement protocol, and the gap list the Queue panel exposes, so the user can decide the full replacement.
- Keep `mbv-gui` behavior the same.

**Non-Goals:**

- Any panel besides the Queue panel. That covers the Library, Settings, the player transport buttons, the header and footer summary, and every overlay.
- Keyboard and pointer input (D10).
- A full GUI replacement, an ADR, packaging, or a release.
- Running the live measurement. It needs the user's approval (see Open Questions).
- Patching Blitz, Dioxus, or Stylo. Upstream fixes go upstream as draft reports.

## Decisions

### D1. Pin `dioxus-native` 0.7.10, the latest stable release

The repository's project rules require the latest stable release and check peer ranges. On 2026-11-13, the latest stable `dioxus-native` is 0.7.10. An earlier note said 0.7.10 uses Blitz 0.3.0-beta. That is wrong: 0.7.10 pins Blitz 0.2.x. Blitz 0.3.0-beta.4 comes with `dioxus-native` 0.8.0-alpha.3, a prerelease. This design does not use the prerelease.

Resolved versions, from the lockfile of a probe crate that depends on `dioxus-native = { version = "=0.7.10", default-features = false, features = ["prelude", "svg", "system-fonts"] }`:

| Crate | Version | Role |
| --- | --- | --- |
| `dioxus-native` | 0.7.10 | the Dioxus renderer (direct) |
| `dioxus-core`, `dioxus-html`, `dioxus-hooks`, `dioxus-signals` | 0.7.10 | pulled by `dioxus-native` |
| `dioxus-asset-resolver` | 0.7.10 | pulls `tokio` (see D13) |
| `blitz-dom` | 0.2.4 | DOM, style, layout, image decode (direct) |
| `blitz-shell` | 0.2.3 | window and event loop (direct) |
| `blitz-traits` | 0.2.0 | `NetProvider`, `NetHandler`, `NetCallback` (direct) |
| `blitz-paint` | 0.2.1 | paint (transitive) |
| `anyrender` / `anyrender_vello` | 0.6.2 | paint backend (transitive) |
| `wgpu` / `wgpu_context` | 26.0.1 / 0.1.2 | GPU renderer (transitive) |
| `winit` | 0.30.13 | window system (direct) |
| `stylo` / `taffy` / `parley` | 0.8.0 / 0.9.2 / 0.6.0 | style, layout, text (transitive) |
| `tokio` | 1.54.1 | transitive (D13) |

Alternatives considered:

- **0.8.0-alpha.3** (Blitz 0.3.0-beta.4, winit 0.31.0-beta.3, `vello-cpu` available). Rejected by the stable rule. Its CPU renderer would answer the CPU-cost question, so the user can approve it later as an Open Question.

### D2. The renderer is the GPU path on Linux

In 0.7.10, `dioxus_renderer.rs` aliases `InnerRenderer` to `anyrender_vello::VelloWindowRenderer` (wgpu). It aliases the CPU renderer only for the iOS simulator. The Linux build therefore renders through wgpu. The CPU renderer is not selectable on Linux in this version.

Consequences:

- The unpaced-animation problem in #1309 applies to the CPU renderer, which this build does not use. The repaint problem in #1497 applies to the GPU path: an animation frame repaints the whole window. The spike uses no CSS animation, so #1497 does not fire from the spike's own code.
- The memory baseline includes the GPU driver share (Vulkan or GL driver libraries mapped into the process). The measurement records that share as its own line, so it is visible.

### D3. Feature set

`dioxus-native = { version = "=0.7.10", default-features = false, features = ["prelude", "svg", "system-fonts"] }`.

- `prelude`: `rsx!`, the hooks, and the signals.
- `svg`: inline SVG for the Lucide icons. Gap G11.
- `system-fonts`: fallback faces through fontconfig.
- Off: `net` (no `blitz-net`, no `reqwest`, no tokio runtime created by `dioxus-native`), `html` (no HTML string parsing), `accessibility` (AccessKit adds memory and the spike has no assistive-tech goal), `hot-reload`, `clipboard`, `file_dialog`.

`blitz-dom` keeps its default features, including `woff-rust`. That lets `@font-face` load the vendored woff2 files (D12).

### D4. Direct dependencies (exact)

Each direct dependency pins the version above. The crate sets `[lints] workspace = true`.

- `dioxus-native = "=0.7.10"` with the features in D3.
- `blitz-traits = "=0.2.0"`: `NetProvider<Resource>`, `NetHandler`, `NetCallback`, `Request`, `Bytes`.
- `blitz-dom = "=0.2.4"`: `blitz_dom::net::Resource`, `DocumentConfig`.
- `blitz-shell = "=0.2.3"`: `BlitzShellEvent`, `BlitzShellNetCallback`, `WindowConfig`, `create_default_event_loop`.
- `winit = "=0.30.13"`: window attributes and the Wayland app id extension.
- `futures-channel = "=0.3.34"`: `unbounded` channel, to wake the Dioxus task from the core thread (D7). This upgrades the repository lock from 0.3.32 to 0.3.34, the latest stable.
- `image = { version = "=0.25.10", default-features = false, features = ["png", "jpeg"] }`: downscale and encode the art (D6). 0.25.10 is the version the repository lock already holds.
- `mimalloc = "=0.1.52"`: the same global allocator as `mbv-gui`, so the two RSS numbers compare. The profiling build drops it (D16). 0.1.52 is the version the repository lock already holds.
- Internal: `mbv` (root), `mbv-gui` (feature gate, D5), `mbv-images`, `mbv-ui-model`, `mbv-emby-model`, `mbv-config`.

Peer ranges, read from the crates' manifests: `blitz-dom` 0.2.4 requires `blitz-traits` 0.2.0. `blitz-shell` 0.2.3 requires `winit` ^0.30.2, `blitz-dom` ^0.2.0, and `blitz-traits` 0.2.0. `dioxus-native-dom` 0.7.10 requires `blitz-dom` 0.2.4. The lock resolves `winit` to 0.30.13, which satisfies every range. The build step runs `cargo tree -d` to check for duplicate Blitz or winit versions.

### D5. Share the GUI core through a Cargo feature gate, not a move

The spike needs `core_thread`, `attach`, `sections`, `leaves`, `painted_choices`, `art`, and the `bridge` DTOs from `mbv-gui`. It needs them without Tauri. Two options:

- **Gate (recommended):** make `tauri` and `tauri-build` optional behind a default feature `tauri` in `crates/mbv-gui/Cargo.toml`. Set `required-features = ["tauri"]` on the `mbv-gui` binary target. Move `main.rs`'s Tauri glue under the same feature. `mbv-native` depends on `mbv-gui` with `default-features = false`. The library compiles with no WebKitGTK.
- **Move (rejected):** move every Tauri-free module into a new `mbv-gui-core` crate and update `mbv-gui` to depend on it. That moves about 50 files and changes many import paths. It adds churn the spike does not need.

The gate changes `mbv-gui`'s manifest and entry point only. Its behavior does not change. Open Question 1 asks for approval.

The `tauri` feature gate must keep the `mbv-gui` tests that do not need Tauri. Each test that needs Tauri gets `required-features = ["tauri"]`.

### D6. Artwork in process through a custom `NetProvider`

**How an `<img>` loads in Blitz 0.2.4.** A node's `src` goes to `DocumentConfig.net_provider` (`Arc<dyn NetProvider<Resource>>`). The provider calls `handler.bytes(doc_id, bytes, callback)`. `ImageHandler::bytes` decodes the bytes into RGBA8, clones the decoded image once (`image.clone().into_rgba8()`), and calls `callback.call`. The callback is `BlitzShellNetCallback`, which sends `BlitzShellEvent::ResourceLoad` to the event loop. The document then holds the decoded image.

**Why a custom launch.** `dioxus_native::launch_cfg_with_props` in 0.7.10 always installs its own `DioxusNativeNetProvider`. That provider is in a private module (`mod assets`), and it has no way to inject a second provider. The spike copies the launch body (about 100 lines, `dioxus-native` 0.7.10 `src/lib.rs`). The copy uses the public types `DioxusNativeWindowRenderer::new()`, `DioxusNativeApplication::new(proxy, config)` with winit's `EventLoopProxy<BlitzShellEvent>`, `WindowConfig::with_attributes`, `create_default_event_loop`, and `DocumentConfig`. It sets `net_provider` to `MbvArtNetProvider` and does not create a tokio runtime.

**The provider.** `MbvArtNetProvider` holds the core's `Sender<CoreInput>` and a `callback: Arc<dyn NetCallback<Resource>>` made with `BlitzShellNetCallback::shared(proxy)`.

- `fetch(doc_id, request, handler)`: if the scheme is not `mbv-art`, it logs and returns. For `mbv-art://localhost/<path>`, it builds `CoreInput::Art(ArtRequest { path, reply })`. The `reply` closure keeps `handler`, `doc_id`, and `callback`. It sends the answer back through `handler.bytes(doc_id, Bytes::from(bytes), callback)`.
- `Found`: the bytes are the downscaled PNG the core encoded (D6, below).
- `Missing`: the bytes are the bundled queue placeholder (`QUEUE_CARD_PLACEHOLDER_BYTES`). `BlitzShellNetCallback::call` drops errors (`TODO: handle error case`), so an error answer would leave the image blank. The placeholder answer avoids that.
- The URLs come from the GUI's existing `art_url`, `card_path`, and `placeholder` helpers. The scheme and encoding stay in one place.

**Downscale before Blitz decodes.** The core's art reply downscales the source to at most 512 px on the long edge. It encodes PNG, which keeps alpha. The reply runs on the core thread. Blitz then decodes the small PNG and holds a much smaller RGBA8 copy. This keeps the decode clone (the doubled transient in `ImageHandler::bytes`) small too. The measured cost of the downscale is recorded in `measurements.md`.

**Not used:** a `data:` URL (base64 costs 33 %, and `blitz-shell`'s `DataUriNetProvider` needs the `data-uri` feature and the same decode); `blitz-net` (its `Provider` takes its own callback, and it pulls `reqwest` and tokio); a GPU texture registered through `Widget` and `Paint::Resource` (#896 is open and asks for exactly this, so the path is not ready).

### D7. Feed the Dioxus side from the core thread

The core thread calls `core_thread::spawn(sink, quit)`. The `sink` is `Box<dyn Fn(GuiSectionBatch) + Send>`. It sends the batch into a `futures_channel::mpsc::UnboundedSender<GuiSectionBatch>`. `unbounded_send` is synchronous and thread-safe, so the core never blocks on the UI.

A Dioxus `use_future` awaits the receiver. It writes each section into its `Signal` (`queue`, `playback`, `playhead`, `hover`, `footer`, `status`, `theme`). Waking works like this: the receiver's task waker is the document's waker. blitz-shell builds that waker with `create_waker`, which sends `BlitzShellEvent::Poll` to the event loop. Dioxus then polls the task, which writes the signal, which re-renders. The spike verifies this path in the first build.

The window's close sends `CoreInput::Close` through the returned `Sender`, as the GUI does.

### D8. The playhead

The window keeps the last `GuiPlayhead` anchor and the `std::time::Instant` at which the window received it. A std thread, not an async timer (Dioxus and Blitz have none), sends a tick into the same channel at 4 Hz while the anchor is active and not paused. The tick does not run while paused or with no anchor.

On each tick, the window computes `position = anchor.position + elapsed`, clamped to `[0, runtime]`. It writes the Signal only when the quantized value changes: the scrubber fill in whole pixels of the bar width, and the clock text in whole seconds. A paused item writes nothing. This avoids a repaint for a change nobody can see. The tolerance and the re-anchor rules come from `gui-change-driven-sections` D5 (`PLAYHEAD_TOLERANCE` = 1 s), and the core already applies them.

4 Hz is the proposed tick. The bar moves about 1 px per 10 to 15 s for a feature-length item, so 4 Hz is fine for the fill. The clock text changes once per second. The measurement records CPU at this rate.

### D9. Queue panel scope

The window paints these values only:

- the queue list from the `queue` section (the viewport rows, the active marker, and the active row's played fraction);
- the scrubber from the `playback` and `playhead` sections (the played fill, the chapter ticks, the intro and credits marks, the elapsed and remaining clocks);
- the playback artwork from `playback.artwork` (`GuiArtRef`).

These are out of scope: the scrubber hover preview and its trickplay tile, the transport buttons, the playback target chooser, the queue header and footer (summary and finish times), and every overlay.

### D10. No input in this spike

The window sends no key or pointer input to the core. The only input is the close. Reasons:

- The goal is to measure memory, threads, CPU, paint, and gaps. Input is not needed to measure those.
- Element geometry does not work in 0.7.10. `MountedData::get_client_rect` returns `MountedError::NotSupported` by default, and `dioxus-native-dom` 0.7.10 does not implement it (G5). The core's hit map needs geometry, so input would need a separate geometry path.

Open Question 4 asks for approval.

### D11. Look and layout

Plain CSS in `crates/mbv-native/assets/queue.css`. The values come from the GUI's `app.css` tokens and from the `theme` section's role values. The spike does not run Tailwind. Running Tailwind would need Node in the build, and the spike must stay Rust-only. The CSS keeps the same class roles as the GUI so the look can be compared side by side. Layout uses flexbox, which Blitz supports (Taffy). Colour uses `rgb()` or hex values in the spike's CSS. Where a value is `oklch` in the GUI, the spike converts it once, when the CSS is written.

### D12. Fonts

Vendor the Latin woff2 files of `@fontsource/ibm-plex-sans` 5.3.0 and `@fontsource/jetbrains-mono` 5.3.0 at weights 400, 500, and 600 into `crates/mbv-native/assets/fonts/`. Both families are under the SIL Open Font License 1.1. Load them with `@font-face` (blitz-dom's default `woff-rust` feature decodes woff2). `font-family` falls back to `system-ui` and `monospace`, as the GUI's `--font-sans` and `--font-mono` do.

### D13. The tokio edge exception

`dioxus-native` 0.7.10 depends on `dioxus-asset-resolver`, which depends on `tokio` 1.54.1 (not optional). `cargo tree` with `default-features = false` shows it. The `net` feature is off, so `dioxus-native` creates no tokio runtime. This change uses no tokio API. Under mbv's "prefer synchronous code first" rule, this is an edge exception. It is recorded here and in the crate's `Cargo.toml` comment. Open Question 5 asks for approval.

### D14. Window identity and display backend

Title `mbv`. Wayland app id `io.github.slatkin.mbv-native`, set with winit's Wayland window extension. The app id differs from the GUI's `io.github.slatkin.mbv-gui`, so the two do not share a desktop entry.

Wayland: winit 0.30.13's default features enable `wayland`, `x11`, `wayland-dlopen`, and `wayland-csd-adwaita`. The spike keeps those defaults, so winit picks Wayland when a Wayland display exists and falls back to X11 otherwise. A Wayland-only build is a task for a full replacement, not this spike.

### D15. Owner attach

The binary calls `declare_gui_client()` first. It then runs `AttachCampaign` with `LibraryOwnerSteps`, as the GUI does. `attach_owner(config, Some(owner_program), ImageDecode::SourceOnly)` is the attach. The Owner is the sibling `mbv` program. The spike adds no attach code of its own.

### D16. Measurement

The protocol uses commands only. It writes no script (AGENTS.md, Tooling). The commands are:

- Process IDs: `pgrep -a mbv-native`, `pgrep -a mbv-gui`, `pgrep -a WebKitWebProcess`, `pgrep -a WebKitNetworkProcess`.
- PSS and anonymous PSS per process: `rg '^(Pss|Pss_Anon):' /proc/<pid>/smaps_rollup`. For the GPU driver share, read `/proc/<pid>/smaps` and sum the `Pss:` lines of the mappings whose path names a driver library (`libvulkan`, `libGL`, `libEGL`, `libgallium`, or the `nvidia` driver libraries). Record that sum on its own line, separate from the Rust heap. Sum the lines by hand into `measurements.md`.
- Threads: `rg '^Threads:' /proc/<pid>/status`, and `ps -L -p <pid> | wc -l` as a cross-check.
- CPU: `/proc/<pid>/stat` fields 14 and 15 (utime and stime, in clock ticks) at two times 300 s apart, divided by 300 times `getconf CLK_TCK`.
- Heap: a profiling build with `--features profiling`. The feature drops the `#[global_allocator]` (mimalloc) so heaptrack can see the allocations. Run `heaptrack ./target/profiling/mbv-native`, then `heaptrack_print` and read the peak heap.

The GUI baseline (measured 2026-11-13: about 505 MB PSS, 248 MB `mbv-gui`, 222 MB `WebKitWebProcess`, 35 MB network process, heaptrack peak 59.5 MB) is the comparison. Before the spike's run, the same protocol is run against the GUI on the same machine, with the same Owner and the same item, and its thread count is recorded. That record is the threads baseline.

The run happens after implementation, with the user's approval, because it runs live against the user's Owner (AGENTS.md, Tests). It is not a task.

### D17. Tests

The spike owns two contracts. Each lives in `src/` (private items), with mocks and no window:

1. `MbvArtNetProvider` answers an `mbv-art` request with the core's bytes (`Found`), or with the placeholder (`Missing`). Mock: a fake core reply, a fake `NetHandler`, a fake `NetCallback`.
2. The playhead tick writes a Signal only when the quantized value changes. Test the quantizer, not the timer.

No test starts a window, a thread with a real sleep, a live mpv, or the Owner.

## Blitz gaps the Queue panel reaches

Blitz issues are tracked on Blitz `main`, the 0.3 line. The 0.2.x crates this change pins may differ. Each gap is marked: **confirmed in source** (checked in the pinned crate), **issue** (open upstream issue; the spike verifies it), or **verify** (the spike checks it on the first build).

| # | Gap | Queue panel use | Evidence | Status |
| --- | --- | --- | --- | --- |
| G1 | `text-overflow: ellipsis` not painted | queue row titles (`truncate`) | https://github.com/DioxusLabs/blitz/issues/888 | issue |
| G2 | `box-shadow` painting: outer blurred and offset shadows, multiple shadows, and per-corner radius are wrong; inset shadows are wrong (hairlines, averaged radius) | the playing band's 3 px spread ring (`0 0 0 3px`); the drag ghost's blurred shadow is out of scope | https://github.com/DioxusLabs/blitz/issues/349 (outer shadows); https://github.com/DioxusLabs/blitz/issues/1038 (inset) | issue; the spread ring is verify, the inset rule is not used by the Queue panel |
| G3 | Animation-driven frames repaint the whole window | none in the spike (no CSS animation); the GUI's spinner rules show the cost | https://github.com/DioxusLabs/blitz/issues/1497 (fix on a fork, not merged) | issue |
| G4 | Unpaced animation frames on CPU renderers | none on Linux in 0.7.10 (D2) | https://github.com/DioxusLabs/blitz/issues/1309 | issue, not applicable to this build |
| G5 | `get_client_rect` is `NotSupported`; no element geometry for a hit map | blocks input (D10) | `dioxus-html` 0.7.10 `events/mounted.rs` default; no implementation in `dioxus-native-dom` 0.7.10 | **confirmed in source** |
| G6 | `<img>` decodes to RGBA8 in the net handler and clones the decoded image once | every art image (D6) | `blitz-dom` 0.2.4 `net.rs`, `ImageHandler::bytes` | **confirmed in source**; mitigated by D6's downscale |
| G7 | No replaceable image provider for renderer-owned textures | would avoid the decode | https://github.com/DioxusLabs/blitz/issues/896 | issue |
| G8 | `BlitzShellNetCallback::call` drops error results (`TODO`) | missing art (D6) | `blitz-shell` 0.2.3 `net.rs` | **confirmed in source**; mitigated by the placeholder answer |
| G9 | The 0.7.10 launch hard-codes its own net provider | art in process (D6) | `dioxus-native` 0.7.10 `src/lib.rs`, `src/assets.rs` (private module) | **confirmed in source**; mitigated by the custom launch |
| G10 | Transforms: a transitioned `transform` is not painted; a transform changes coordinates | the scrubber thumb and the `translate-y-1/2` centring | https://github.com/DioxusLabs/blitz/issues/840, https://github.com/DioxusLabs/blitz/issues/663 | issue; verify static transforms only (the spike uses no transition) |
| G11 | Inline SVG parity (Lucide icons) | none in the Queue panel's rows; the transport buttons are out of scope (D9) | https://github.com/DioxusLabs/blitz/issues/260, https://github.com/DioxusLabs/blitz/issues/1543 | verify |
| G12 | Tabular figures (`font-variant-numeric`, `tabular-nums`) | every clock and duration | https://github.com/DioxusLabs/blitz/issues/688 (closed) | verify |
| G13 | Wayland: slow resize; GPU surface `Timeout` panic | the crash gate (G-Crash in Go / No-Go) | https://github.com/DioxusLabs/blitz/issues/293 | issue |
| G14 | No usable GPU: panic at surface creation; a software adapter costs several cores | hosts without Vulkan | https://github.com/DioxusLabs/blitz/issues/1505 | issue |
| G15 | Startup shows uninitialized content before the first real paint | first frame | https://github.com/DioxusLabs/blitz/issues/374 | issue |
| G16 | Memory: an idle demo measured over 200 MB on Windows, about 50 MB after idling 30 minutes | context for the memory gate | https://github.com/DioxusLabs/blitz/issues/286 | issue; measure on Linux |

### Upstream reports

For each gap that the spike confirms and that no issue covers, the change drafts a minimal HTML and CSS repro. The drafts go in `openspec/changes/blitz-queue-panel-spike/upstream/`. Each draft searches existing issues and WPT first. **Nothing is posted.** Posting publishes content outside the machine, so it needs the user's explicit approval for each draft (Open Question 6).

## Go / No-Go

The bounds are proposals. The user confirms or changes them (Open Question 3). The measurement run follows the protocol in D16.

- **G-Mem:** total PSS of the `mbv-native` process, with the GPU driver share, at idle with playback, is at most 250 MB. The GUI baseline is 505 MB across its processes.
- **G-Heap:** the profiling build's heaptrack peak heap is at most 30 MB. The GUI baseline is 59.5 MB.
- **G-Threads:** the total thread count is at most the GUI's thread count under the same protocol. The GUI count is recorded first.
- **G-CPU:** the average CPU over 5 minutes of playback is at most 5 % of one core, with the playhead ticking at 4 Hz.
- **G-Playhead:** while playing, the scrubber fill moves at least once every 250 ms. A seek or a paused state re-anchors within one tick. The position error against the Owner's position is at most 1 s.
- **G-Crash:** 30 minutes of playback with track changes and window resizes, with no panic and no GPU surface timeout exit.
- **G-Look:** the user judges whether the Queue panel's layout and colours match the GUI, with the differences limited to gaps G1 to G3 and G10. The user makes this judgement. It is not a task.

The verdict is **go** when G-Mem, G-Heap, G-Threads, G-CPU, G-Playhead, and G-Crash pass and the user accepts G-Look. Any other result is **no-go**. The numbers and the verdict go in `measurements.md`.

## Risks / Trade-offs

- [Blitz 0.2.x differs from the issues, which track 0.3] → Each gap is marked confirmed, issue, or verify. The first build checks the verify items.
- [The GPU driver's share dominates the memory] → D16 reports it on its own line. If it dominates, the no-go result names the CPU path (0.8 alpha) as the next measurement, not a fix.
- [The art decode runs on the core thread and stalls a tick] → D6 downscales to 512 px. The measurement records tick time before and after.
- [A missing image shows the placeholder, with no error signal to the document] → D6, Missing. Accepted: this matches the GUI's loading fill.
- [Dioxus wake through the document waker is not verified] → D7 names the path. The first build checks that a batch from another thread repaints without input.
- [No hit geometry] → D10 excludes input. G5 records the gap.
- [No GPU panics at start] → G14. The crash gate runs on a host with a GPU.
- [`tokio` enters the build] → D13. The spike uses no tokio API.
- [MPL-2.0 from Stylo] → The spike is not distributed. A full replacement would need the MPL notices and the source offer for the Servo files (the licensing review in the Blitz repository, `docs/licensing.md`).

## Migration Plan

1. Merge `feat/gui-change-driven-sections` to `main`. This change does not start before that.
2. Apply the gate to `mbv-gui` (Open Question 1) and add the `mbv-native` crate. `mbv-gui` builds and its tests pass with the gate on and off.
3. Build `mbv-native` and check it on the first build: the wake path (D7), the art path (D6), and each verify gap.
4. Write the measurement protocol into `measurements.md` (commands only). Run it only after the user approves the live run.
5. The user decides go or no-go. A go starts a new change for the full replacement, with an ADR. A no-go removes the crate and the workspace member in one commit. The feature gate on `mbv-gui` stays or goes with the user's choice.

Rollback at any step: remove `crates/mbv-native` from the workspace. The gate on `mbv-gui` is a Cargo feature with a default, so it has no effect on the GUI's build.

## Open Questions

1. Do you approve the Cargo feature gate on `mbv-gui` (D5)? Recommended: Yes. Without it, the spike builds WebKitGTK.
2. Do you approve `dioxus-native` 0.7.10 (stable) instead of 0.8.0-alpha.3 (Blitz 0.3.0-beta.4, CPU renderer)? Recommended: Yes, by the stable rule. Choosing the alpha would change D1 to D4 and the measurement.
3. Do you approve the go or no-go bounds as written in "Go / No-Go"? Reply Yes, or name new numbers for G-Mem, G-Heap, G-CPU, and G-Playhead.
4. Do you approve the spike with no keyboard or pointer input (D10)? Recommended: Yes.
5. Do you approve the tokio edge exception (D13)? Recommended: Yes.
6. Do you approve drafting upstream Blitz reports into `upstream/` in this change, with posting only after you approve each one? Recommended: Yes.
