# Design

## Context

See `proposal.md` for the reason. This section records the state the design depends on, checked on 2026-11-13.

**The GUI core is on `main`.** PR #1001 (`72e525307`, "update the window through change-driven sections") put the core on `main`: `core_thread`, `attach`, `sections`, `art`, `leaves`, `painted_choices`, and the `mbv-art` path. `CoreInput::Art`, `ArtRequest`, `ArtResponse`, `GuiArtRef`, `SourceOnly` image decoding, and `declare_gui_client()` (`src/owner_link.rs`, re-exported from `src/lib.rs`) are on `main`. `feat/gui-change-driven-sections` holds 66 later commits that `main` lacks: performance work, docs, and an archive commit. This change needs the APIs on `main`, not those commits. Task 1.1 checks this.

**`mbv-gui` depends on Tauri at package level.** `main` declares `tauri = "=2.12.5"` as a normal dependency, and `build.rs` runs `tauri_build::try_build`. A library dependency on `mbv-gui` therefore builds Tauri and WebKitGTK unless the gate in D18 is applied.

**The Queue panel's CSS uses features Blitz may not paint.** The Queue rows use `truncate`, `tabular-nums`, and `font-mono`. The scrubber uses `translate`. The playing band is a 3 px spread `box-shadow` ring. The icons are Lucide SVGs. The fonts are IBM Plex Sans and JetBrains Mono from `@fontsource` 5.3.0, Latin subset, weights 400, 500, and 600. The app CSS is compiled Tailwind v4 (`oklch`, `color-mix`, `:has()`, `@apply`), which this change does not reuse (D11).

### Probe evidence (2026-11-13)

The probes ran outside the repository, in `~/.cache/blitz-probe/` (`p8`, `p9`, `p10`, `ws`). `ws` is a `git archive main` copy with the `mbv-native` crate added and the D18 gate applied. The repository was not changed.

- **P1, feature set compiles.** A minimal `dioxus-native` app with `default-features = false` and features `prelude`, `svg`, `system-fonts`, `woff`, `vello-hybrid` passes `cargo check` (exit 0). Without a renderer feature it fails at dioxus-native's own `compile_error!("At least one renderer feature must be enabled")`. That is the B1 cause, not `app_title`.
- **P2, launch copy compiles.** The launch copy (D6) compiles with the net provider swapped, `navigation_provider: None`, and the Wayland app id set through winit's platform attributes.
- **P3, workspace check.** In `ws`, `cargo check -p mbv-native` passes with no warnings, and `cargo check -p mbv-gui --no-default-features --lib` passes. With no default features, `cargo tree -p mbv-gui --no-default-features -i tauri` (and `wry`, `webkit2gtk`, `tauri-runtime-wry`) prints nothing. `tauri-build` stays in the build graph.
- **P4, four headless tests pass** (`cargo test -p mbv-native`, 4 passed, 0.44 s, no window, no sleep):
  - an `mbv-art` image request reaches the core channel with path `card/abc`, and a `Found` answer of a 1024×768 PNG yields a decoded raster of 512×384 on the `img` node;
  - a `Missing` answer yields the WebP placeholder raster (this proves the `webp` image feature);
  - a value sent from another thread updates a signal and the DOM text after one `poll`;
  - an inline `<style>` `@font-face` requests `mbv-asset://localhost/font/ibm-plex-sans-400.woff2` through the single net provider. The font in the probe is a WOFF2 made from a local TTF, and the real vendored files replace it in the implementation.

The headless harness uses `DioxusDocument::initial_build`, `Document::poll`, and `BaseDocument::handle_messages` (the last one drains resource events, which `poll` does not).

### Sources

- Blitz repository, `AGENTS.md`, `CONTRIBUTING.MD`, `docs/licensing.md` (licensing review dated 2026-10-07): https://github.com/DioxusLabs/blitz
- `dioxus-native` 0.8.0-alpha.3 and 0.7.10 on crates.io: https://crates.io/crates/dioxus-native/0.8.0-alpha.3
- Crate sources read from `static.crates.io` and the cargo registry: `dioxus-native` 0.8.0-alpha.3 (`src/lib.rs`, `src/dioxus_renderer.rs`, `Cargo.toml`), `dioxus-native-dom` 0.8.0-alpha.3 (`src/events.rs`, `src/dioxus_document.rs`), `blitz-dom` 0.3.0-beta.4 (`src/net.rs`, `src/document.rs`, `src/lib.rs`), `blitz-traits` 0.3.0-beta.4 (`src/net.rs`), `blitz-shell` 0.3.0-beta.4, `winit-wayland` 0.31.0-beta.3 (`src/lib.rs`), `blitz-paint` 0.3.0-beta.4 (`src/render/box_shadow.rs`).
- Issues cited in the gap table: https://github.com/DioxusLabs/blitz/issues/<number> (numbers are given per row).

## Goals / Non-Goals

**Goals:**

- Build one window that paints the Queue panel from the GUI core, in process, on Blitz 0.3 beta.
- Produce a measurement protocol and a Blitz gap table that the user can decide on.
- Keep `mbv-gui` behavior the same.

**Non-Goals:**

- Any panel besides the Queue panel: the Library, Settings, transport buttons, the header and footer summary, and every overlay.
- Keyboard and pointer input (D10).
- The full GUI replacement, an ADR, packaging, and release work.
- Running the live measurement. It runs against the user's Owner, so it needs the user's approval.
- Patching Blitz, Dioxus, or Stylo. Upstream fixes go upstream as drafts.

## Decisions

### D1. Pin `dioxus-native` 0.8.0-alpha.3 with Blitz 0.3.0-beta.4

The user chose this version as an early adopter. The earlier stable-release rationale is withdrawn: the project rule cited (`M-LATEST-EDITION`) governs the Rust edition, not dependency versions.

Resolved versions in `ws` (the lock as resolved against `main`):

| Crate | Version | Role |
| --- | --- | --- |
| `dioxus-native` | 0.8.0-alpha.3 | the Dioxus renderer (direct) |
| `dioxus-native-dom`, `dioxus-core`, `dioxus-document`, `dioxus-signals`, `dioxus-hooks` | 0.8.0-alpha.3 | pulled by `dioxus-native` |
| `dioxus-asset-resolver`, `dioxus-cli-config` | 0.8.0-alpha.3 | pulled by `dioxus-native` (see D13) |
| `blitz-dom`, `blitz-paint`, `blitz-shell`, `blitz-traits` | 0.3.0-beta.4 | Blitz (`blitz-shell` and `blitz-traits` direct) |
| `anyrender`, `anyrender_vello_hybrid` | 0.14.0, 0.11.0 | paint backend (transitive) |
| `wgpu`, `wgpu_context` | 30.0.2, 0.10.0 | GPU renderer (transitive) |
| `winit` | 0.31.0-beta.3 | window system (transitive; reached through `dioxus_native::winit`) |
| `stylo`, `taffy`, `parley`, `usvg`, `accesskit` | 0.22.0, 0.15.1, 0.12.0, 0.48.1, 0.25.2 | style, layout, text, SVG, accessibility (transitive) |
| `tokio` | 1.54.1 | transitive (D13); the repository already locks this version |
| `image` | 0.25.10 | art decode and downscale (direct, D4) |
| `futures-channel`, `futures-util` | 0.3.34 | wake channel (direct, D4) |
| `mimalloc` | 0.1.52 | global allocator (direct, D4) |

Peer ranges, from the crate manifests: `blitz-shell` 0.3.0-beta.4 requires `winit` `=0.31.0-beta.3` and `blitz-dom` `=0.3.0-beta.4`. `dioxus-native` requires `blitz-dom`, `blitz-shell`, and `blitz-traits` at `=0.3.0-beta.4`. `blitz-dom` 0.3.0-beta.4 requires `image` 0.25.6 with default features off. The lock resolves `image` to 0.25.10, which satisfies that range, so Cargo unifies to one copy.

### D2. Renderer: `vello-hybrid` (GPU, wgpu)

`dioxus-native` 0.8.0-alpha.3 requires one renderer feature. This change enables `vello-hybrid`, which is the crate's default renderer. It draws through wgpu on Vulkan or GL. The CPU renderers (`vello-cpu-softbuffer`, `vello-cpu-pixels`) exist in this version and are not enabled. The CPU path is a later measurement if the GPU share of memory breaks the gate.

Consequences:

- The unpaced CPU animation problem (#1309) does not apply, because the GPU path presents at vsync. The whole-window animation repaint problem (#1497) does apply to any CSS animation. The spike uses no CSS animation (D8 drives the playhead with signal writes).
- The memory measurement includes the GPU driver share, which D16 reports on its own line.
- The GPU adapter failure (#1505) is a real risk on hosts without Vulkan. The crash gate (G-Crash) runs on a host with a GPU.

### D3. Feature set

`dioxus-native = { version = "=0.8.0-alpha.3", default-features = false, features = ["prelude", "svg", "system-fonts", "woff", "vello-hybrid"] }`.

- `prelude`: `rsx!`, hooks, signals, `VirtualDom`, `Element`.
- `svg`: inline SVG for the Lucide icons (G11).
- `system-fonts`: system fallback faces through fontconfig.
- `woff`: WOFF and WOFF2 decoding for `@font-face` (D12).
- `vello-hybrid`: the GPU renderer (D2).

Defaults that this set turns off in `dioxus-native`: `accessibility`, `net`, `html`, `hot-reload`, `clipboard`, `file-dialog`, `text-transform-icu` (kept on below), `apple-font-embolden`, and the `vello` and `vello-cpu` renderers.

### D4. Direct dependencies (exact)

Each crate sets `[lints] workspace = true`.

- `dioxus-native = "=0.8.0-alpha.3"` with the features in D3.
- `blitz-shell = { version = "=0.3.0-beta.4", default-features = false }`: `BlitzShellProxy`, `WindowConfig`, `create_default_event_loop`, `BlitzShellEvent`.
- `blitz-traits = { version = "=0.3.0-beta.4", default-features = false }`: `NetProvider`, `NetHandler`, `Request`, `Url`, `Bytes`.
- `futures-channel = "=0.3.34"`: the unbounded wake channel (D7). This upgrades the repository lock from 0.3.32.
- `futures-util = { version = "=0.3.34", default-features = false, features = ["std"] }`: `StreamExt` for the receiver. This upgrades the lock from 0.3.32.
- `image = { workspace = true, features = ["png", "jpeg", "webp"] }`: decode, downscale, and PNG encode (D6). The workspace lock already holds 0.25.10, so nothing downgrades. WebP is needed for the placeholder.
- `mbv`, `mbv-gui` (with `default-features = false`, D18), `mbv-images` (the placeholder bytes): internal.
- `mimalloc.workspace = true`: the same global allocator as `mbv-gui`, so the two RSS numbers compare. The `profiling` feature drops it (D16). `mimalloc` is already at 0.1.52 in the lock.
- Dev only: `blitz-dom = { version = "=0.3.0-beta.4", default-features = false }`, for the `Document` trait in the headless tests (D17).

`winit` is not a direct dependency. The launch copy uses `dioxus_native::winit`, which dioxus-native re-exports, so the window attributes match the version the stack uses.

Lock effect, measured on the repository lock (commit 3d3a8d758, `ws` resolution, 2026-11-13): no repository entry downgrades, and no entry is removed except the nine replaced entries below. The nine replaced entries (name and version change, patch releases forced by the new graph) are `bytemuck` 1.25.0 to 1.25.2, `smallvec` 1.16.1 to 1.16.5, and seven `futures-*` crates from 0.3.32 to 0.3.34: `futures-channel`, `futures-core`, `futures-io`, `futures-macro`, `futures-sink`, `futures-task`, and `futures-util`. Forty-two other entries keep their version and change only their resolved dependency edges, for example `rustix` and `tempfile` (`windows-sys` 0.52.0 to 0.61.2, and `tempfile`'s `getrandom` 0.3.4 to 0.4.2) and `num_enum_derive` (`proc-macro-crate` 1.3.1 to 3.5.0). 276 crate names are added. `tokio` 1.54.1 is already in the repository lock through `mbv-desktop`. `wgpu` 29.0.4 appears in the lock through `pixels` 0.17.2 (`pixels_window_renderer`), but `cargo tree --workspace -e all` finds no path from a workspace crate to it, so it is locked and not built. Task 2.3 re-checks this.

### D5. Share the GUI core through the feature gate

The spike uses `core_thread`, `attach`, `sections`, `art`, and the `bridge` DTOs from `mbv-gui`. It uses them as a library, without Tauri. The option of moving those modules into a new crate was rejected, because it moves about 50 files. The gate in D18 changes `mbv-gui`'s manifest and its `build.rs` only. Its behavior does not change. The user approved this change (decision 1).

### D6. Artwork in process through a custom `NetProvider`

**How an `<img>` loads in blitz-dom 0.3.** The document holds `DocumentConfig.net_provider: Option<Arc<dyn NetProvider>>`. The trait is non-generic:

```rust
pub trait NetProvider: Send + Sync + 'static {
    fn fetch(&self, doc_id: usize, request: Request, handler: Box<dyn NetHandler>);
}
pub trait NetHandler: Send + Sync + 'static {
    fn bytes(self: Box<Self>, resolved_url: String, bytes: Bytes);
    fn error(self: Box<Self>, url: String, error: String) {}  // default: no-op
}
```

The image handler decodes with `image::ImageReader`, calls `into_rgba8().into_raw()`, and `respond`s with a `DocumentEvent::ResourceLoad`. The document then loads the resource and asks the shell for a redraw. The handler runs on whichever thread calls `bytes`. The spike calls it from the core thread.

**Why a custom launch.** In `dioxus-native` 0.8.0-alpha.3, `launch_cfg_with_props` installs its own provider. Without the `net` feature, that provider has no inner provider. The provider type is private (`mod assets`), and no injection point exists. The spike copies the launch body (`src/lib.rs`, `launch_cfg_with_props`, about 100 lines) and replaces the provider. The copy uses these public items: `DioxusNativeWindowRenderer`, `RendererOptions`, `DioxusNativeApplication::new(proxy, event_queue, window)`, `WindowConfig::with_attributes`, `BlitzShellProxy::new`, `create_default_event_loop`, `DioxusDocument`, and `DocumentConfig`, all of which `dioxus_native` re-exports. The probe `ws/crates/mbv-native/src/launch.rs` compiles.

**The provider.** `MbvNetProvider` holds the core's `Sender<CoreInput>`.

- `mbv-art://localhost/<path>`: the path (without its leading slash) goes into `ArtRequest { path, reply }`. The `reply` closure keeps the `handler` and the resolved URL. On `Found { bytes, .. }`, it decodes the source, scales it to a long edge of at most 512 px, encodes PNG, and calls `handler.bytes(url, Bytes::from(png))`. On `Missing`, it sends the bundled placeholder (`QUEUE_CARD_PLACEHOLDER_BYTES`, WebP) through the same call.
- The `Missing` path does not call `handler.error`, because the 0.3 `ResourceHandler` does not implement it (G8). A placeholder answer keeps the image painted.
- `mbv-asset://localhost/font/<file>`: the vendored WOFF2 bytes (`include_bytes!`), answered in place (D12).

**Downscale before Blitz decodes.** The reply downscales on the core thread, so Blitz decodes a small PNG and holds a small RGBA8 copy. The probe measured the 512 px cap on a 1024×768 source: the node holds 512×384 RGBA8. The measured cost of the decode on the core thread is recorded in `measurements.md`.

**Not used:** a `data:` URL (base64 costs about 33 %), `blitz-shell`'s `DataUriNetProvider` (feature `data-uri`), `blitz-net` (it pulls `reqwest` and a tokio runtime), and a renderer-owned texture (#896 is open and asks for that path, so it is not ready).

### D7. Feed the Dioxus side from the core thread

The core thread's sink is `Box<dyn Fn(GuiSectionBatch) + Send>`. It calls `unbounded_send` on a `futures_channel::mpsc::UnboundedSender`. The send is synchronous and thread-safe, so the core does not wait on the UI. A Dioxus `use_future` awaits the receiver and writes each section into its signal.

Wake path: the task waits on the receiver, so the sender's wake reaches the task's waker. Dioxus polls the task, and the signal write marks the component dirty. A window's redraw follows the document's `ResourceLoad` path or the shell's redraw request. Headless proof: P4 shows a value from another thread updating the DOM after one `poll`. The window-level path (a batch sent while the window is idle, with no input) is a first-build check (task 6.2).

Window close: the window's close sends `CoreInput::Close` through the returned `Sender`, then the core handle is joined.

### D8. The playhead

The window keeps the last `GuiPlayhead` anchor and the `std::time::Instant` at which it received the anchor. A std thread (Dioxus and Blitz have no timer) sends a tick into the channel every 250 ms, which is 4 Hz, while the anchor is active and not paused.

On each tick, the window writes its predicted position, `anchor.position_ticks + elapsed`, clamped to `[0, runtime_ticks]`, into the playhead signal. The value is not quantized. A paused or absent anchor writes nothing. The clock text (elapsed and remaining) is derived from the position and changes only when the whole second changes, so the text repaints at most once per second. The re-anchor rules come from `gui-change-driven-sections` D5 (`PLAYHEAD_TOLERANCE` = 1 s), and the core applies them.

Why 4 Hz: a feature-length item moves the bar about 1 px per 10 to 15 s, so the fill itself changes little. The gate (G-Playhead) fixes the write rate, not the pixel motion. The measurement records CPU at this rate (G-CPU).

### D9. Queue panel scope

The window paints these values only:

- the queue list from the `queue` section: the viewport rows, the active marker, and the active row's played fraction;
- the scrubber from the `playback` and `playhead` sections: the played fill, the chapter ticks, the intro and credits marks, and the elapsed and remaining clocks;
- the playback artwork from `playback.artwork`.

The artwork reference is `GuiArtRef`, with three variants: `Image { src }`, `Placeholder { src }`, and `Loading`. There is no key. The window paints `src` for `Image` and `Placeholder`, and paints nothing for `Loading` or an absent reference (B5). `src` is already a complete `mbv-art` URL built by `art_url`, so the window does not build paths.

Out of scope: the scrubber hover preview and its trickplay tile, the transport buttons, the track chips, the playback target chooser, the queue header and footer (summary and finish times), and every overlay.

### D10. No input in this spike

The window sends no key or pointer input to the core, and the only input is the close.

Geometry is available in this version. `dioxus-native-dom` 0.8.0-alpha.3 implements `MountedData::get_client_rect` through `get_client_bounding_rect` (`src/events.rs`), so a later input change can report regions. The user decided on no input for this spike (decision 4). The gap table (G5) records the change from 0.7.10.

### D11. Look and layout

Plain CSS in `crates/mbv-native/assets/queue.css`, included with `include_str!` and placed in one `style` element. The values come from the GUI's `app.css` tokens and the `theme` section's role values. Colours that the GUI writes as `oklch` are converted once to sRGB when the file is written. The spike does not run Tailwind, because Tailwind needs Node in the build, and this crate stays Rust only. The class names follow the GUI's roles, so the two Queue panels can be compared side by side.

### D12. Fonts and styles through the one provider

- Vendor the Latin WOFF2 files of `@fontsource/ibm-plex-sans` 5.3.0 and `@fontsource/jetbrains-mono` 5.3.0 at weights 400, 500, and 600 into `crates/mbv-native/assets/fonts/` (six files). Both families are under the SIL Open Font License 1.1.
- The inline `style` element declares six `@font-face` rules with `src: url('mbv-asset://localhost/font/<file>') format('woff2')`. `blitz-dom` fetches each `@font-face` source through the document's net provider (`document.rs`, `add_stylesheet_for_node` calls `fetch_font_face`). The provider answers with the `include_bytes!` bytes (D6). Probe P4 proves the request.
- `font-family` falls back to `system-ui` and `monospace`, as the GUI's `--font-sans` and `--font-mono` do. `font_ctx` stays `None`, so `system-fonts` provides the fallbacks.
- Whether the registered face renders the expected glyphs is a first-build check (task 6.2).

### D13. The tokio edge

`dioxus-native` 0.8.0-alpha.3 depends on `dioxus-asset-resolver` with `features = ["native"]`, and that dependency is not optional (`Cargo.toml`, the `dioxus-asset-resolver` entry). The `native` feature enables `tokio` 1.54.1 with the `rt` feature. The edge therefore exists with every feature set in D3, and `dioxus-native` gives no switch for it. This change uses no tokio API, and `dioxus-native` creates no tokio runtime on this path (`net` is off).

The earlier note that tokio is optional there was wrong: the `native` feature is always on. The repository already locks tokio 1.54.1 through `mbv-desktop`. The user approved the exception (decision 5). Task 2.5 adds `crates/mbv-native` to the edge list in `AGENTS.md`, which names the tokio edge crates.

### D14. Window identity and display backend

Title `mbv`. The Wayland app id is `io.github.slatkin.mbv-native`, set through `WindowAttributesWayland::default().with_name(APP_ID, "mbv-native")`, attached with `WindowAttributes::with_platform_attributes`. The app id differs from the GUI's `io.github.slatkin.mbv-gui`, so the two do not share a desktop entry.

winit 0.31.0-beta.3 enables `wayland` and `x11` by default. The spike keeps those defaults. winit picks Wayland when a Wayland display exists and falls back to X11. A Wayland-only build is a later choice for a full replacement.

### D15. Owner attach

`main` calls `declare_gui_client()` first, which claims the non-participant status for Pin swap (`src/owner_link.rs`). It then calls `core_thread::spawn`, which runs the attach campaign with `LibraryOwnerSteps` and `attach_owner(config, Some(owner_program), ImageDecode::SourceOnly)`. The owner program is the sibling `mbv`. The spike adds no attach code.

### D16. Measurement

The protocol uses commands only, and writes no script (AGENTS.md, Tooling). Commands:

- Process IDs: `pgrep -a mbv-native`, `pgrep -a mbv-gui`, `pgrep -a WebKitWebProcess`, `pgrep -a WebKitNetworkProcess`.
- PSS and anonymous PSS per process: `rg '^(Pss|Pss_Anon):' /proc/<pid>/smaps_rollup`.
- GPU driver share: read `/proc/<pid>/smaps` and sum the `Pss:` lines of mappings whose path names a driver library (`libvulkan`, `libGL`, `libEGL`, `libgallium`, or the NVIDIA driver libraries). Record the sum on its own line.
- Threads: `rg '^Threads:' /proc/<pid>/status`, and `ps -L -p <pid> | wc -l` as a cross-check.
- CPU: `/proc/<pid>/stat` fields 14 and 15 (utime and stime, in clock ticks) at two times 300 s apart, divided by 300 times `getconf CLK_TCK`.
- Heap: a profiling build with `--features profiling`, which drops the global allocator. Run `heaptrack ./target/profiling/mbv-native`, then `heaptrack_print`, and read the peak heap.

The GUI baseline (2026-11-13: about 505 MB PSS, 248 MB `mbv-gui`, 222 MB `WebKitWebProcess`, 35 MB network process, heaptrack peak 59.5 MB) is the comparison. Before the spike's run, the same protocol runs against the GUI on the same machine, with the same Owner and the same item. Its thread count is the threads baseline.

The live run needs the user's approval, because it runs against the user's Owner (AGENTS.md, Tests). It is a post-implementation step, not a task.

### D17. Tests

Five headless contracts, in `src/` (private items), with no window, no Owner, no sleep, and no mpv:

1. `MbvNetProvider` answers `Found` with downscaled PNG (512 px cap) on the image node (P4).
2. `MbvNetProvider` answers `Missing` with the placeholder raster (P4).
3. An inline `@font-face` source is requested through the provider (P4).
4. A value sent from another thread updates the DOM after one `poll` (P4).
5. The playhead writes only while an anchor is active and not paused. This tests a pure function `playhead_write(anchor, now) -> Option<f64>`, not the timer.

A sixth check, `art_src(GuiArtRef) -> Option<&str>` returning `None` for `Loading`, is a pure one-line test. Test count stays low by design (AGENTS.md, Tests).

### D18. The `mbv-gui` feature gate

- `crates/mbv-gui/Cargo.toml`: `tauri = { version = "=2.12.5", optional = true }`. Add `[features] default = ["tauri"]` and `tauri = ["dep:tauri"]`. Add `[[bin]] name = "mbv-gui" path = "src/main.rs" required-features = ["tauri"]`.
- `tauri-build` stays a normal build dependency. It does not pull WebKit. The probe confirms this: `cargo tree -p mbv-gui --no-default-features -e normal,build -i tauri-build` lists it, and `tauri`, `wry`, `webkit2gtk`, and `tauri-runtime-wry` are absent.
- `build.rs`: return early unless `CARGO_FEATURE_TAURI` is set. A `cfg(feature)` check does not work in a build script, so the environment variable is the gate. The probe applies this.
- `mbv-gui`'s library target compiles with no default features (P3). Its behavior does not change. The user approved the gate (decision 1).

### D19. The new Client kind: "Blitz Client"

The window is a new Client kind, named **Blitz Client**, with capability `blitz-client-window`.

Why not a GUI Client variant: `CONTEXT.md` defines **GUI Client** by kind, as "the Client kind that presents a native window", and `openspec/specs/gui-client` holds the requirements of the WebKit program. A second GUI Client would collide with that name and that definition. Modifying `gui-client` would put spike-only requirements into the main spec, and a no-go would have to remove them. A separate term keeps the spike out of the main specs.

The term says that the Blitz Client is not the GUI Client, that it presents only the Queue panel, and that a full replacement would fold it into the GUI Client. The term enters `CONTEXT.md` with task 7.1. The crate and binary keep the name `mbv-native`, which the user chose.

## Blitz gap table (Blitz 0.3.0-beta.4, dioxus-native 0.8.0-alpha.3)

Status values: **present** (the code path exists in the pinned crate), **absent** (checked in the pinned source; no code path), **moot** (the spike does not reach it), **open** (the upstream issue is open and applies), **verify** (needs a fixture window run, task 6.2).

| # | Gap | Queue panel use | Evidence | Status (2026-11-13) |
| --- | --- | --- | --- | --- |
| G1 | `text-overflow: ellipsis` is not painted | queue row titles (`truncate`) | grep of `blitz-dom` and `blitz-paint` 0.3.0-beta.4 finds no `text-overflow` handling (only `overflow_wrap`); issue #888 open | **absent**. Upstream draft (task 7.3). |
| G2 | `box-shadow` painting: outer and inset shadows, blur, offsets | the 3 px spread ring of the playing band | `blitz-paint` `render/box_shadow.rs` draws outset and inset shadows; issues #349 (outer shadows, open) and #1038 (inset, open) | **present, quality open**. Verify the ring in task 6.2. |
| G3 | Animation-driven frames repaint the whole window | none (no CSS animation) | issue #1497 open; its fix is on a fork | **moot**. G-CPU measures the cost with no animation. |
| G4 | Unpaced animation frames on CPU renderers | none (the spike uses `vello-hybrid`, D2) | issue #1309 open; it describes the CPU path | **moot**. |
| G5 | Element geometry (`get_client_rect`) | blocks input (D10) | 0.7.10 returned `NotSupported`; 0.8.0-alpha.3 implements it through `get_client_bounding_rect` (`dioxus-native-dom` `src/events.rs`) | **present** (changed from absent in 0.7.10). |
| G6 | `<img>` decodes to RGBA8 in the handler | every art image | `blitz-dom` 0.3.0-beta.4 `ImageHandler::parse`: `into_rgba8().into_raw()`. The 0.2 double clone is gone. Probe P4: a 512×384 raster from a 1024×768 source | **present** (no clone in 0.3). Mitigated by the downscale (D6). |
| G7 | No replaceable image provider for renderer-owned textures | would avoid the decode | issue #896 open | **open**. |
| G8 | Error results are dropped | missing art | `NetHandler::error` defaults to a no-op, and `ResourceHandler` implements no `error` (grep of `blitz-dom` 0.3.0-beta.4 `net.rs`) | **absent**. Mitigated by the placeholder answer (P4). Upstream draft (task 7.3). |
| G9 | The launch installs its own net provider; no injection point | art and fonts in process | `dioxus-native` 0.8.0-alpha.3 `launch_cfg_with_props`; the provider type is private (`mod assets`) | **absent**. Mitigated by the launch copy (P2). Upstream draft (task 7.3). |
| G10 | Static transforms (the `translate` centring and the thumb) | the scrubber thumb | issues #840 (transitioned transform not painted) and #663 (transform affects coordinates), both open | **verify** (no transition is used, so #840 does not apply; #663 needs the fixture run). |
| G11 | Inline SVG (Lucide icons) | none in the queue rows; the transport buttons are out of scope | `svg` feature (`usvg` 0.48.1) is on; issues #260 and #1543 (parity) open | **present, parity open**. Verify icon output in task 6.2. |
| G12 | Tabular figures | every clock and duration | `stylo_to_parley.rs` maps `FontVariantNumeric` and `font_feature_settings` | **present**. Verify the glyphs in task 6.2. |
| G13 | Wayland: slow resize; GPU surface `Timeout` panic | the crash gate | issue #293 open | **open**. G-Crash runs on Wayland. |
| G14 | No usable GPU: panic at surface creation | hosts without Vulkan | issue #1505 open; the spike has no software fallback | **open**. Applies to `vello-hybrid`. |
| G15 | Startup shows uninitialized content | first frame | issue #374 open | **verify** (task 6.2). |
| G16 | Memory: idle demo over 200 MB on Windows | context for the memory gate | issue #286 open | **context**. D16 measures Linux. |
| G17 | `navigation_provider` impl is private | link handling | `DioxusNativeNavigationProvider` is `pub(crate)` (`src/link_handler.rs`) | **absent**. The launch copy sets `None`; the Queue panel has no links. |
| G18 | Accesskit stays on | memory | `accesskit` comes from `blitz-dom` defaults through `dioxus-native-dom`'s default `accessibility` feature. `dioxus-native` cannot turn it off. | **present** (a dependency fact, not a gap). Measured in D16. |

Dependency facts that are not gaps: `icu_normalizer` (`text-transform-icu`, from `dioxus-native-dom`'s defaults) and `dioxus-cli-config` (from `dioxus-asset-resolver`'s defaults) stay on. `atspi`, `zbus`, `arboard`, `rfd`, `reqwest`, `blitz-net`, `blitz-html`, `dioxus-devtools`, `anyrender_vello_cpu`, and Skia are absent under D3.

## Upstream reports

The drafts for G1, G8, and G9 go in `openspec/changes/blitz-queue-panel-spike/upstream/`. Each draft names the existing issues and WPT tests it searched first. Nothing is posted. The user approves each draft before anyone posts it (decision 6).

## Go / No-Go

The user approved these bounds (decision 3), except that G-Playhead changed under B6.

- **G-Mem:** total PSS of the `mbv-native` process, with the GPU driver share, at idle with playback, is at most 250 MB.
- **G-Heap:** the profiling build's heaptrack peak is at most 30 MB.
- **G-Threads:** the total thread count is at most the GUI's thread count under the same protocol.
- **G-CPU:** average CPU over 5 minutes of playback is at most 5 % of one core, with the playhead written at 4 Hz.
- **G-Playhead:** while playing, the playhead signal is written at least every 250 ms, and a seek or a pause takes effect within one tick. The position error against the Owner's position is at most 1 s. No write occurs while paused.
- **G-Crash:** 30 minutes of playback with track changes and window resizes, with no panic and no GPU surface timeout exit, on a host with a GPU.
- **G-Look:** the user judges whether the Queue panel matches the GUI's layout and colours, with differences limited to the absent or open gaps G1, G2, and G10. The user makes this judgement. It is not a task.

The verdict is **go** when G-Mem, G-Heap, G-Threads, G-CPU, G-Playhead, and G-Crash pass and the user accepts G-Look. Any other result is **no-go**.

## Risks / Trade-offs

- [Blitz 0.3.0-beta.4 and dioxus-native 0.8.0-alpha.3 are prerelease] → The user accepted this as an early adopter (decision 2). The probes pin every crate exactly, so an upstream change cannot move the build.
- [The GPU driver share dominates memory] → D16 reports it on its own line. If it dominates, the no-go result names the CPU renderer as the next measurement.
- [The art decode runs on the core thread] → D6 caps the source at 512 px. The measurement records the tick time before and after.
- [A missing image shows the placeholder, with no error signal] → D6 `Missing`, and G8.
- [The window-level wake is not proven] → Headless P4 proves the document path. Task 6.2 proves the window path with a fixture run.
- [The tokio edge is unavoidable with this `dioxus-native`] → D13. The repository already locks the same tokio version.
- [`accesskit` stays on] → G18. It is measured, not removed.
- [MPL-2.0 from Stylo 0.22.0] → The spike is not distributed. A full replacement needs the MPL notices and the source offer for the Servo files (`docs/licensing.md` in the Blitz repository).
- [No input, so no hit map yet] → D10.

## Migration Plan

1. Task 1.1 confirms the core APIs on `main`. The spike starts from `main`, not from the branch.
2. Apply the gate (D18) and add the `mbv-native` crate (D3, D4). Build it, and run the four headless contracts (D17).
3. Run the fixture window check (task 6.2), after the user approves a window on the dev machine with no Owner.
4. Write `measurements.md` (task 7.2). Run the protocol only after the user approves the live run.
5. The user decides go or no-go. A go starts a full-replacement change with an ADR. A no-go removes the crate and the workspace member in one commit. The gate stays or goes by the user's choice.

Rollback at any step: remove `crates/mbv-native` from the workspace. The gate is a Cargo feature with a default, so the GUI builds the same with the default features.

## Open Questions

1. Do you confirm `vello-hybrid` (D2) as the renderer for the spike? The CPU renderers are available in this version and are not enabled. Yes or no?
2. Do you approve the live measurement run (D16) after implementation? Each run needs your approval. Yes or no?
3. Do you approve one fixture window run on your machine for task 6.2, with no Owner, no Emby, and no network? The run opens a window and sends fixture batches. Yes or no?
