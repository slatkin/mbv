## 1. Reduce wall-clock bound

- [x] 1.1 Change `AUTHENTICATE_HARD_BOUND` in `crates/mbv-core/src/api_client_auth.rs` from `Duration::from_secs(15)` to `Duration::from_secs(5)`

## 2. Token preservation on connectivity failures

- [x] 2.1 In `authenticate()` (crates/mbv-core/src/api_client_auth.rs, lines 96-100): remove `self.token.clear()` and `self.user_id.clear()` from the generic `Err(e)` branch. The 401/403 branch (lines 90-94) must continue clearing both the in-memory fields and the on-disk cache via `clear_cached_token()`.
- [x] 2.2 Add or update tests in `crates/mbv-core/src/api_tests.rs` (or the appropriate test module) verifying: after a connectivity-class error (e.g. connection refused), `self.token` and `self.user_id` are preserved; after a 401/403, both are cleared.

## 3. Failure classification

- [x] 3.1 In `src/main.rs`: define an `AuthFailure` enum with three variants: `FirstRun`, `CredentialsExpired`, `ServerUnreachable(String)`. The `String` carries the ureq error details.
- [x] 3.2 Replace `classify_auth_failure()` with a new function (e.g. `classify_auth_failure_kind()`) that returns `AuthFailure`. Match logic:
  - `"No cached credentials"` / `"No server URL configured"` → `FirstRun`
  - `"Cached credentials expired"` → `CredentialsExpired`
  - `"Cached credential validation failed: ..."` (or anything else, including `"timed out after ..."`) → `ServerUnreachable(error_details)`
- [x] 3.3 Update existing unit tests for `classify_auth_failure` to test the new enum return type. The test `classify_auth_failure_network_error_names_the_server` should verify `ServerUnreachable` is returned; `classify_auth_failure_hard_join_timeout_flows_through_generic_branch` should verify `ServerUnreachable` with the timeout string.

## 4. Startup branching in authenticate_or_login

- [x] 4.1 Refactor `authenticate_or_login()` in `src/main.rs` to match on the `AuthFailure` enum:
  - `ServerUnreachable(details)` → `eprintln!("mbv: could not reach {}: {}", server_url, details)` and `std::process::exit(1)`. No login screen.
  - `CredentialsExpired` → `login::run(client, ui_config, Some("Your session has expired — please log in again.".to_string()))`
  - `FirstRun` → `login::run(client, ui_config, None)`
- [x] 4.2 Verify the error message format matches existing `main.rs` patterns: `mbv: could not reach {url}: {details}` with no recovery instructions.

## 5. Cleanup

- [x] 5.1 Remove the old `classify_auth_failure()` function and its associated test `classify_auth_failure_expired_credentials_gets_quiet_wording`, `classify_auth_failure_first_run_cases_stay_silent`, `classify_auth_failure_network_error_names_the_server`, `classify_auth_failure_hard_join_timeout_flows_through_generic_branch`. Replace with new tests for the enum-based classifier.
- [x] 5.2 Update the doc comment on `classify_auth_failure_kind()` (or whatever the new function is named) to document the three-way classification and reference issue #192.

## 6. Verification

- [x] 6.1 Run `cargo check -p mbv-core` to verify the crate compiles (`cargo check --workspace` passed).
- [x] 6.2 Run `cargo test -p mbv-core` to verify existing tests pass and new token-preservation tests pass (308 passed, incl. the 2 new authenticate tests; `cargo test --bin mbv` 664 passed).
- [x] 6.3 Run `cargo clippy --workspace --all-targets` to verify no new warnings (only pre-existing `too_many_arguments` warnings in `src/app/render/`).
- [x] 6.4 Manually verify: server unreachable → `mbv: could not reach http://127.0.0.1:1: ... Connection refused` exits 1 in <1s; blackhole IP → `mbv: could not reach ...: timed out after 5s` exits 1 in ~5s; no login screen; token cache file preserved on disk in both cases.
