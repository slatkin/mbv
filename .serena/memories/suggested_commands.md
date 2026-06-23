# Suggested Commands

## Build
```sh
cargo build              # debug (faster compile)
cargo build --release    # release
```

## Run
```sh
cargo run                # launch TUI (debug build)
cargo run -- -d          # daemon mode
```
After editing `scripts/mbv.lua`, copy before `cargo run`:
```sh
cp scripts/mbv.lua ~/.local/share/mbv/scripts/mbv.lua
```

## Test
```sh
cargo test               # all tests
cargo test config        # tests matching "config"
cargo test -- --nocapture  # see println! output
```

## Lint
```sh
cargo clippy             # linter — fix all warnings, delete unused code rather than #[allow]
```

## Logs (debug)
```sh
tail -f ~/.local/state/mbv/mbv.log          # main log
tail -f ~/.local/state/mbv/player-diag.log  # mpv diagnostics
grep -i "navigate\|error" ~/.local/state/mbv/mbv.log | tail -30
```

## Release (see CHECKIN.md)
1. Bump version in Cargo.toml
2. `cargo build` to update Cargo.lock
3. Commit: `Release X.Y.Z: <summary>`
4. Push → GitHub Action updates PKGBUILD sha256
5. `git tag vX.Y.Z && git push origin vX.Y.Z`
