# Tasks

## 1. Logger

- [ ] 1.1 Add a level parameter to `applog::init` that sets `log::set_max_level`; update all three callers to pass Info. Verify with a unit test that a debug record is not enabled after init at Info (`cargo nextest run -p mbv-core applog`).
- [ ] 1.2 Prefix stderr lines with `<3>/<4>/<6>/<7>` by level; file lines unchanged. Verify with an `rstest` `#[case]` table over the four levels on a pure line-formatting function.

## 2. Flag

- [ ] 2.1 Parse `--log-level <error|warn|info|debug>` in `mbvd` (`crates/mbvd/src/main.rs`) and pass it to init; bad value prints usage and exits non-zero; add it to the usage string. Verify with a parse unit test covering valid and invalid values.
- [ ] 2.2 Same flag in `mbv` (`src/main.rs`) and usage text; when spawning `--__local-daemon`, forward `--log-level <level>`; `run_local_daemon_main` reads it and passes it to init. Verify with a parse unit test and a test on the spawn argv builder.

## 3. Sinks and call sites

- [ ] 3.1 System-instance `mbvd` passes `None` as the log path (keep `log_path()` for other instances). Verify by reading the diff; `cargo check -p mbvd`.
- [ ] 3.2 Demote `ws.rs` per-message inbound log (already debug — confirm) and the Capabilities request/reply logs in `api_client_reporting.rs` from info to debug; leave failure paths at warn/error. Verify with `cargo check -p mbv-core`.

## 4. Gate

- [ ] 4.1 `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run -p mbv-core -p mbvd -p mbv` all pass.
