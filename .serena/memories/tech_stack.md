# Tech Stack

- **Language**: Rust, edition 2021
- **TUI**: ratatui 0.30 + crossterm 0.29
- **HTTP**: ureq 2 (sync, native-tls)
- **JSON**: serde_json 1
- **Config**: toml 1.1
- **Media playback**: libmpv2 6 (wraps libmpv2-sys 4)
- **WebSocket**: tungstenite 0.24 (native-tls)
- **D-Bus / MPRIS**: zbus 4 + tokio 1
- **Images**: ratatui-image 11 + image 0.25 (jpeg/png/gif)
- **Fuzzy search**: fuzzy-matcher 0.3
- **Logging**: log 0.4 (ring buffer via applog.rs; files at ~/.local/state/mbv/)
- **Build dep**: resvg 0.47 (SVG → icon at build time)
- **Package manager**: Cargo (Cargo.lock checked in)
- **Lua script**: `scripts/mbv.lua` deployed to `~/.local/share/mbv/scripts/mbv.lua` for `cargo run`
