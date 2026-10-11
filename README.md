## mbv

`mbv` is a terminal media player: a Rust terminal UI that shows your Emby / Audiobookshelf / RSS-feed libraries and plays and syncs them through an embedded mpv. One background process per user (the "Owner") holds the player and the queue; every UI window is a client that talks to it over a local socket. 

`mbvd` is the same daemon packaged as a headless server you can reach over the network; I use it on a small machine attached to my stereo amp like it was mbd but serving from my Emby library. 

Full Emby client session remote control is also supported.

You can download it. It's a vibe-coded funtime hobby though. I don't even care enough about licencing to type "MIT" on this! Danger.

It requires mpv, obviously.

### Instructions
You turn it on and put in some stuff and there it is.

Building from source for your own machine? `RUSTFLAGS="-C target-cpu=native" cargo build --release` tunes to your CPU instead.

### What's broken
Presumably a lot of stuff.
