## Before checking in

0. Do not add `Co-Authored-By` trailers to commit messages.
1. Ask the user for permission to commit and push
2. For PR branches: `git fetch origin && git merge --no-ff origin/main`, resolve conflicts, then run checks before pushing or opening the PR.
3. Run checks once before pushing/opening a PR, not after every edit or every commit.
4. If code changed: run the narrowest relevant checks for the change. Include `cargo fmt --all -- --check` for Rust code. Add targeted tests when they directly cover the changed path; do not run broad suites unless the change warrants it or the user asks.
5. Commit and push
