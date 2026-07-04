# Task Completion Checklist

Run these before considering a coding task done:

1. **Build**: `cargo build` — must succeed with zero errors
2. **Lint**: `cargo clippy` — fix all warnings; delete unused code rather than suppressing
3. **Tests**: `cargo test` — all must pass
4. **Lua copy** (if `scripts/mbv.lua` was edited): `cp scripts/mbv.lua ~/.local/share/mbv/scripts/mbv.lua`
5. **Ask user before committing or pushing** (CHECKIN.md requirement)

## Commit format

- Message: imperative summary, no Co-Authored-By trailer
- Release commits: `Release X.Y.Z: <one-line summary>`
