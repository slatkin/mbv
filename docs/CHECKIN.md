## Before checking in

0. Do not add `Co-Authored-By` trailers to commit messages.
1. Ask the user for permission to commit and push
2. For PR branches: `git fetch origin && git merge --no-ff origin/main`, resolve conflicts, then run checks before pushing or opening the PR.
3. If code changed: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`
4. Commit and push
