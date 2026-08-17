## 1. Fix on `main` (PR #555 already merged — this is new, standalone work)

- [ ] 1.1 Branch from current `origin/main` (`81383e4f` or later). Change `crates/mbv-core/src/lib.rs`: `pub(crate) fn native_tls_agent` → `pub fn native_tls_agent`.
- [ ] 1.2 Replace `src/app/feed_parse.rs`'s `tls_agent` body with `mbv_core::native_tls_agent(None, global_timeout)`; delete the now-dead inline `Agent::config_builder()` incantation.

## 2. Verify

- [ ] 2.1 `rtk cargo check --workspace --all-targets`.
- [ ] 2.2 `rtk cargo clippy --workspace --all-targets` clean.
- [ ] 2.3 `rtk cargo nextest run --workspace` — no test changes expected (no behavior change).
- [ ] 2.4 `rtk make check-code-file-lines` clean.

## 3. Land

- [ ] 3.1 Open as its own small PR against `main`. (Not an amendment to #555 — that PR is merged and closed.)
