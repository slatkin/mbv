## mbv

A TUI front-end for mpv with Emby, Audiobookshelf, media RSS feed support. You can download it. It's a vibe-coded funtime hobby though. You don't wanna.

### Instructions
You turn it on and put in some stuff and there it is.

### Desktop launcher

`contrib/mbv.desktop` starts `mbv --desktop`. With `pinwin` installed, mbv
opens pinned to a screen edge in a pinwin panel instead of a terminal; without
`pinwin`, it opens in your default terminal through `xdg-terminal-exec`. This
needs a Wayland compositor with layer-shell support; on a compositor without
it, pinwin exits and nothing opens, so launch `mbv` from a terminal there.

### What's broken
Presumably a lot of stuff.
