# Tasks

## 1. Edition bump (commit 1)

- [x] 1.1 Run `cargo fix --edition --workspace --all-targets` on edition 2021, then set `edition = "2024"` in root `Cargo.toml` `[workspace.package]`; verify `cargo check --workspace --all-targets` passes.
- [x] 1.2 Read every `tail_expr_drop_order` and `if_let_rescope` site `cargo fix` reported (design.md lists the ones known so far); keep 2024 drop order unless the earlier drop is observably wrong, in which case bind the temporary explicitly; verify each site was reviewed by listing it in the commit message.
- [x] 1.3 Wrap the `std::env::set_var`/`remove_var` calls in `crates/mbv-config/src/test_support.rs` (and any others `cargo fix` flagged) in `unsafe` blocks with a `// SAFETY:` note; fix any RPIT capture errors with `use<..>`; verify `cargo check --workspace --all-targets` passes with no new `#[allow]`/`#[expect]`.
- [x] 1.4 Update the format line in `AGENTS.md` from "stock edition-2021" to "stock edition-2024"; commit 1.1–1.4 together; verify `rg -n 'edition-2021' AGENTS.md` returns nothing.

## 2. Reformat (commit 2)

- [x] 2.1 Run `cargo fmt --all` (2024 style follows the edition) and commit the result alone; verify `cargo fmt --all -- --check` passes and the commit contains only formatting changes.

## 3. Adopt let-chains (commit 3)

- [x] 3.1 Run `cargo clippy --fix --workspace --all-targets --allow-dirty`, then `cargo fmt --all`; verify the remaining `collapsible_if` warnings are listed by `cargo clippy --workspace --all-targets -- -D warnings`.
- [x] 3.2 Hand-collapse remaining `collapsible_if` sites under `src/` into let-chains; verify `cargo clippy -p mbv --all-targets -- -D warnings` reports none in `src/`.
- [x] 3.3 Hand-collapse remaining `collapsible_if` sites under `crates/`; verify `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [x] 3.4 Rewrite the let-chain forms clippy does not flag: tuple scrutinees `if let (Some(a), Some(b)) = (x, y)` (8 sites) → `if let Some(a) = x && let Some(b) = y`, and `if let Some(v) = expr.filter(|…| cond)` where the closure only adds a condition (15 sites) → `if let Some(v) = expr && cond`; leave a `.filter` whose predicate reads the bound value if the chain would be no clearer. Verify with `rg -n 'if let \(Some\(.*\), Some\(.*\)\) = \(' src crates` returning nothing and `cargo nextest run --workspace` passing.

## 4. Verification

- [x] 4.1 Run `cargo nextest run --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and `make check-code-file-lines`; all pass (split any file the reformat pushed over 800 lines).
- [x] 4.2 Confirm `.pi-lens.json` still disables `rust-2024-let-chain-candidate` (clippy owns this check now), then close #840 in the PR description.
