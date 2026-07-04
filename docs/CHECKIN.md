## Before checking in

0. Do not add `Co-Authored-By` trailers to commit messages.
1. Ask the user for permission to commit and push
2. If code changed: `cargo test` and `cargo clippy`
3. Commit and push

## Pre-commit hook (one-time setup per machine)

A git hook at `.githooks/pre-commit` runs `cargo fmt --check`, `cargo clippy`,
and `cargo test` automatically on every `git commit`. It's tracked in the repo,
but git doesn't auto-wire `core.hooksPath` on clone — run this once per
machine/clone:

```sh
git config core.hooksPath .githooks
```
