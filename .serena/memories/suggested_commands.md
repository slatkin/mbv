# Suggested Commands

```sh
cargo build --release      # release build
cargo build                # debug build (faster compile)
cargo test                 # run all tests
cargo test config          # run tests matching "config"
cargo test -- --nocapture  # see println! output in tests
cargo clippy               # lint (fix all warnings; delete unused code, don't suppress)
```

## Lua script (cargo run)

After editing `scripts/mbv.lua`, copy to the installed location so `cargo run` picks it up:

```sh
cp scripts/mbv.lua ~/.local/share/mbv/scripts/mbv.lua
```

## Logs (debugging)

```sh
tail -f ~/.local/state/mbv/mbv.log
grep 'source=mpv' ~/.local/state/mbv/mbv.log   # Lua script messages
```
