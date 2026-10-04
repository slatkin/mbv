# Design

## Context

mbv consumes pinwin through `crates/mbv-pinwin` ("Safe Rust wrapper over the vendored pinwin
panel C ABI"):

- `src/ffi.rs`: `PinwinLayout` / `PinwinAccent` / `PinwinStartup` mirrors + `unsafe extern "C"`
  declarations for `pinwin_start`, `pinwin_apply_layout_animated`, `pinwin_stop`.
- `src/panel.rs`: `Panel<'fd>` borrowing the pty master fd; `start(BorrowedFd, Layout,
  KeyboardMode)` fills a hardcoded disabled accent; `apply_layout_animated(&Layout, u32)`;
  `ANIM_DEFAULT_MS = 200`; `Drop` calls `pinwin_stop`.
- `src/layout.rs`: `Side { Left, Right }`, `Gutters`, `Layout { side, cols: u16, gutters }`,
  `KeyboardMode { None, Exclusive, OnDemand }`, each with `to_abi()`.
- `src/error.rs`: `PinwinError` over codes `OK/ERR_INVALID/ERR_ALREADY_RUNNING/ERR_NOT_RUNNING/
  ERR_NO_DISPLAY/ERR_INTERNAL` plus `Unknown(i32)`.
- `build.rs` + `build.zig` + `build.zig.zon` (pin `c64af49`) + `zig-pkg/`: Zig-build the static
  `libpinwin` and link it ahead of `ghostty-vt-static` and the GTK libraries.

The only wrapper-touching file is `src/pin.rs`: `PinnedPanel = mbv_pinwin::Panel<'static>`
(the master fd is `Box::leak`ed because the library outlives any borrow), `apply_layout`,
`start_panel` (openpty, detach, stdio hand-over, env), plus `PinnedWidth` and failure
reporting. Call sites only name `crate::pin` items and `mbv_pinwin::Panel::ANIM_DEFAULT_MS`;
no component or painter names the wrapper (`App`/runtime boundary holds).

Upstream (`feat/port`, `port-to-rust` change) replaces all of this with a Rust crate whose
public surface is, per its `design.md` D4–D7:

- `Panel::start(RawFd, …)` on the host thread, lazily spawning one process-lifetime
  `pinwin-gtk` thread (parked between panels, distinct application ids); apply posts a closure
  via `MainContext::invoke` with a 5 s bounded reply; `Drop` posts teardown without joining.
  Single-instance guard stays process-wide.
- `Layout { side: Side, cols: NonZeroU16, … }`, `Side { Left, Right }`, `Keyboard { None,
  OnDemand, Exclusive }`, accent as `Option<Accent>` (absent is disabled), all `Copy` with
  private fields built by `new()`; unrepresentable states (unknown side, zero cols) lose their
  runtime checks.
- `PinwinError`: `#[non_exhaustive]` enum with `InvalidLayout`, `InvalidFd`, `NoDisplay`,
  `AlreadyRunning`, `NotRunning`, `Internal`; `std::error::Error` without a derive crate.
  `panic = "unwind"` everywhere; `catch_unwind` at every boundary with a poisoned flag that
  reports `Internal`, never `NotRunning`.
- The fd is a `RawFd`: validated, set non-blocking, never closed by the library (D7). Zig
  remains only as a build-time requirement for ghostty's `libghostty-vt` (D2).

## Goals / Non-Goals

**Goals:**

- mbv builds and runs against the Rust crate with zero hand-mirrored ABI surface.
- The pinned launch behaves exactly as today (start, animated toggle, stop, failure paths).
- The `pinned-panel-focus-accent` change has a clean target (`Option<Accent>`) afterwards.

**Non-Goals:**

- Any panel behaviour change (widths, gutters, animation, accent semantics).
- Any pinwin-side work — the port is upstream's; this change only consumes it.
- Repackaging (`mbv-aur`, `mbv-git-aur`, dist notes) beyond recording what changed.

## Decisions

**D1: Replace, don't shim.** `crates/mbv-pinwin` is deleted as a wrapper rather than kept as a
thin re-export, because a shim over an identical-type crate adds a second naming of every type
with no seam to justify it (no second backend, no version bridging). If the crate needs a
patch before release, the patch goes upstream, not into a local shim.

**D2: Pin the crate like the C lib was pinned.** Today the panel is pinned by git hash
(`c64af49`) in `build.zig.zon`. The Rust dependency gets the equivalent: a git-rev pin at
cutover time (mirrors today's reproducibility against the port's breaking-change window),
moving to a version requirement once upstream publishes one. The exact rev is chosen at
implementation time, after 4.1 + 6.3 land — never floating on `main`.

**D3: Type mapping (the whole adaptation).** `src/pin.rs` absorbs every difference so call
sites only see moved paths:

| Today | New |
|---|---|
| `Panel<'fd>` borrowing the master | `pinwin::Panel` (no lifetime; the `Box::leak` stays — the lib still never closes the fd) |
| `Layout { cols: u16 }` struct literal | constructor with `NonZeroU16` (`layout_from_config` maps `PanelConfig` widths; `0` is already rejected by config validation, so no new error path) |
| `KeyboardMode::OnDemand` | `Keyboard::OnDemand` |
| hardcoded disabled accent in `Panel::start` | `None` (D4 wires the real accent) |
| `apply_layout_animated(&Layout, ANIM_DEFAULT_MS)` | same shape under the new paths; both plain and animated apply exist upstream |
| `PinwinError::Unknown(code)` display arm | deleted; new `InvalidFd` maps into `PinStartError::Panel` with the same one-line reporting |

**D4: Accent sequencing.** `pinned-panel-focus-accent` was written against the C ABI mirror
(`PinwinAccent` field, struct-size assertion). After this change its target is
`Some(Accent)` passed at start. Order: this change lands first, then the accent change
rebases onto `Option<Accent>` (its `None` arm is this change's `start_panel` call). The
accent change does not block this one and vice versa, but they must not both be in flight
against different `Panel::start` signatures — the second to land rebases.

**D5: Build requirements.** mbv loses the Zig panel build (`build.rs`, `build.zig`, `zig-pkg/`)
but inherits the crate's build-time need: Zig at build time for ghostty's `libghostty-vt`
(port D2). CI/packaging notes that name "Zig for the panel" are updated to "Zig at build time
for ghostty via the pinwin crate"; if no doc names it, nothing changes.
