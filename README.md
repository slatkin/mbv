## mbv

A TUI front-end for mpv with Emby, Audiobookshelf, media RSS feed support. You can download it. It's a vibe-coded funtime hobby though. You don't wanna.

### Requirements
Prebuilt binaries (tarball, .deb, AUR) need an x86-64-v2 CPU, about 2009 or newer (SSE4.2, POPCNT).

### Instructions
You turn it on and put in some stuff and there it is.

Building from source for your own machine? `RUSTFLAGS="-C target-cpu=native" cargo build --release` tunes to your CPU instead.

### What's broken
Presumably a lot of stuff.
