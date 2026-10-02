# pinwin

Zig + C (GTK4, gtk4-layer-shell, pango/cairo) static library that docks a pty's terminal
(mbv's, when pinned) beside tiled windows. Wayland layer-shell only; no installed executable,
no tray, no control socket. The Rust rules in the root `AGENTS.md` do not apply here; its
repo-wide rules (no bespoke scripts, no lint suppression, file-line cap, commit hygiene) do.

* build: `cd pinwin && zig build` — produces `zig-out/lib/libpinwin.a` (consumers also link
  the ghostty archives installed beside it).
* check: `cd pinwin && zig build check` runs the layout-core and C ABI unit tests.
* dev-only demo: `zig build demo` builds `zig-out/bin/pinwin-demo`; it is never installed and
  never shipped.
* Follow existing style in `src/`; keep C interop in the existing C sources.
* Manual checks run under niri through a host (mbv's `--pin`); nothing here is verified by
  `cargo`.
* Specs: `openspec/specs/pinwin-panel/`.
