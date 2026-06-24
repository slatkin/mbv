# Tech Stack

- **Language**: Rust, edition 2021
- **Version**: mbv 0.6.9 (Cargo.toml)
- **TUI**: ratatui 0.30, crossterm 0.29
- **Media**: libmpv2 6, libmpv2-sys 4
- **HTTP**: ureq 2 (native-tls, json features)
- **WebSocket**: tungstenite 0.24 (native-tls)
- **Async runtime**: tokio 1 (rt, macros, sync, time) — used only for MPRIS (zbus 4)
- **D-Bus / MPRIS**: zbus 4, ksni 0.3
- **Images**: ratatui-image 11, image 0.25 (jpeg/png/gif)
- **Serialization**: serde 1 + serde_json 1, toml 1.1
- **Build dep**: resvg 0.47 (SVG → raster at build time)
- **Packaging**: cargo-deb (`[package.metadata.deb]`)
- **Lua script**: `scripts/mbv.lua` (mpv OSC); installed to `~/.local/share/mbv/scripts/mbv.lua`
