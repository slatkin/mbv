# pinwin

Zig + C (GTK4, gtk4-layer-shell, libdbusmenu-glib) layer-shell panel that runs a command
(mbv) pinned beside tiled windows. Wayland layer-shell only. The Rust rules in the root
`AGENTS.md` do not apply here; its repo-wide rules (no bespoke scripts, no lint suppression,
file-line cap, commit hygiene) do.

* build: `cd pinwin && zig build`
* check: `cd pinwin && zig build check` must print `check_options: all passed`
* Follow existing style in `src/` and `tools/`; keep C interop in the existing C sources.
* Manual checks run under niri with `zig-out/bin/pinwin`: launch, tray, `Pin options...`,
  quit removes the panel. Nothing here is verified by `cargo`.
* Specs: `openspec/specs/pinwin-{panel,tray-options,control}/`.
