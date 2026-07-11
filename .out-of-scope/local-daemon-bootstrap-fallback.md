# Local daemon bootstrap fallback

mbv does not need a richer local-playback fallback for failures during the
implicit same-machine daemon bootstrap path.

## Why this is out of scope

This request improves a narrow failure mode in the old "auto-detect a local
daemon and attach to it as a thin client" startup path: when the adopt-queue
send fails during bootstrap, the TUI currently falls back to local browsing
with an inert remote proxy instead of constructing a real local player and
continuing playback.

That path is being removed by #156. The settled stay-alive design replaces the
attended same-machine daemon story with a locally owned playback session that
survives terminal detach/reattach. It explicitly removes the implicit
local-daemon auto-attach behavior, `-d` / `--daemon-inner` self-spawn, and the
special `is_local_daemon` bootstrap mode this request depends on.

Because the underlying path is going away, adding a more elaborate fallback for
its rare bootstrap-failure edge case would spend implementation effort on code
the project has already decided to retire.

## Prior requests

- #138 - "Local-daemon bootstrap has no real local-playback fallback on adoption failure"
