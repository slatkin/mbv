# "Remote session" terminology

User only calls the session-cast feature (F3 Sessions panel; `connected_session_id` /
`do_session_command` in `src/app/actions.rs`, one mbv instance controlling another mbv
instance via the Emby `/Sessions` API) a "remote session". Never apply that term to the
daemon/`remote_player.rs` (`PlayerProxy::Remote`) mechanism — that's a different concept
and the user explicitly does not want it called "remote session".

# Bug fixed 2026-07-05: paused item didn't resume when switching tracks

Root cause: in `src/player.rs`, three commands reuse an already-running (possibly
paused) mpv instance instead of spawning a fresh process, and none of them told mpv to
unpause: `PlaylistSession::handle_command`'s `JumpTo` arm (used by `Player::next`/
`previous`, and by local queue-Enter when `t != current_idx`), `SingleSession`/
`PlaylistSession`'s `LoadNew` handling (`cmd_load_new`), and
`PlaylistSession::cmd_replace_playlist`. Each only updated the in-memory
`status.paused` field (or nothing at all for JumpTo), not mpv's real `pause` property,
so the new track loaded but stayed silently paused. Fixed by adding
`mpv.set_property("pause", false)` in all three spots. The mpv property-change observer
already updates `status.paused` and reports Pause/Unpause to Emby, so no other plumbing
was needed.
