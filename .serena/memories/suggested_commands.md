# Suggested Commands

## Build

```sh
cargo build --release      # release build
cargo build                # debug build (faster compile)
```

## Test

```sh
cargo test                 # run all tests
cargo test config          # run tests matching "config"
cargo test -- --nocapture  # see println! output
```

## Lint

```sh
cargo clippy               # linter — fix all warnings; delete unused code, don't #[allow]
```

## Run

```sh
cargo run                  # debug run (TUI)
mbv -d                     # launch as daemon
```

## Lua script (must copy after editing when using cargo run)

```sh
cp scripts/mbv.lua ~/.local/share/mbv/scripts/mbv.lua
```

## Debug logs

```sh
tail -f ~/.local/state/mbv/mbv.log
grep source=mpv ~/.local/state/mbv/mbv.log
```

## Release workflow

1. Bump `version` in Cargo.toml
2. `cargo build` (updates Cargo.lock)
3. Commit: `Release X.Y.Z: <summary>` (no Co-Authored-By)
4. Push — GitHub Action updates PKGBUILD sha256
5. Push tags
