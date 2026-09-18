## 1. Non-active row progress

- [x] 1.1 Take every queue row's non-active position from the item's own stored resume ticks, keeping the playing row's live ticks authoritative; verify with a projection test asserting a non-active feed row carries its stored percentage (and that restoring the literal `0` fails it)

## 2. Close-out

- [x] 2.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run -p mbv`, then archive the change so its delta lands in `openspec/specs/`
