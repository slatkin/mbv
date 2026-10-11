## mbv

`mbv` is a terminal media player: a Rust terminal UI that shows your Emby / Audiobookshelf / RSS-feed libraries and plays and syncs them through an embedded mpv. It runs as one background process per user (the "Owner") that holds the player and the queue; every TUI window is a client that talks to it over a local socket. 

Full Emby client session remote control is also supported. `mbv` can control (and be remote-controlled by) standard Emby clients.

`mbvd` uses Emby client protocols to act as a headless server that can be reached over the network; I use it on a small machine attached to my stereo amp like it was mbd but serving from my Emby library. 

If you use Wayland, it comes with a small Smithay/libghostty-vt panel that allows you to pin the app as a layer that can be hidden as necessary.

You can download it. It's a vibe-coded funtime hobby though. I don't even care enough about licencing to type "MIT" on this! Danger.

It requires mpv, obviously.

### Instructions
You turn it on and put in some stuff and there it is.

Building from source for your own machine? `RUSTFLAGS="-C target-cpu=native" cargo build --release` tunes to your CPU instead.

### What's broken
Presumably a lot of stuff.
