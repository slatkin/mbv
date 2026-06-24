# Task Completion Checklist

Run these before considering a coding task done:

1. `cargo build` — must compile clean (zero errors)
2. `cargo clippy` — must pass with zero warnings (fix by deleting dead code, not suppressing)
3. `cargo test` — all tests must pass
4. If Lua script changed: `cp scripts/mbv.lua ~/.local/share/mbv/scripts/mbv.lua`
5. **Ask user before committing or pushing** (CHECKIN.md requirement)

See CHECKIN.md and RELEASE.md for full pre-commit and release steps.
