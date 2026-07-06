## Before checking in

0. Do not add `Co-Authored-By` trailers to commit messages.
1. Ask the user for permission to commit and push
2. If code changed: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`
3. Commit and push

There is no pre-commit git hook — run the checks above yourself before
committing.
