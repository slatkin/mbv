# Tech Stack

- **Language**: Rust, edition 2021
- **Version**: 0.6.8 (Cargo.toml)
- **TUI**: ratatui 0.30 + crossterm 0.29
- **Images**: ratatui-image 11 (crossterm feature), image 0.25 (jpeg/png/gif)
- **HTTP**: ureq 2 (json + native-tls; no async)
- **Serde**: serde 1 + serde_json 1
- **Config**: toml 1.1
- **Player**: libmpv2 6 + libmpv2-sys 4
- **WebSocket**: tungstenite 0.24 (native-tls)
- **D-Bus / MPRIS**: zbus 4 (tokio feature) + tokio 1 (rt, macros, sync, time)
- **System tray**: ksni 0.3 (blocking)
- **Fuzzy search**: fuzzy-matcher 0.3
- **Text**: textwrap 0.16, unicode-width 0.2.2
- **Misc**: rand 0.10, uuid 1 (v4/fast-rng), log 0.4, libc 0.2
- **Build dep**: resvg 0.47 (SVG → font rasterization at build time)
- **Packaging**: `cargo-deb` metadata in Cargo.toml; runtime deps: libmpv2|libmpv1, libc6, libssl3|libssl1.1
