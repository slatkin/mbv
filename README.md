## mbv

`mbv` is a terminal media player: a Rust text UI that shows your Emby / Audiobookshelf / RSS-feed libraries and plays and syncs them through an embedded mpv. One background process per user (the "Owner") holds the player and the queue; every UI window is a client that talks to it over a local socket. `mbvd` is the same daemon packaged as a headless server you can reach over the network. Full Emby client session remote control is supported.

You can download it. It's a vibe-coded funtime hobby though. You don't wanna.

### Requirements
Prebuilt binaries (tarball, .deb, AUR) need an x86-64-v2 CPU, about 2009 or newer (SSE4.2, POPCNT).

### Instructions
You turn it on and put in some stuff and there it is.

Building from source for your own machine? `RUSTFLAGS="-C target-cpu=native" cargo build --release` tunes to your CPU instead.

### What's broken
Presumably a lot of stuff.
