# Tasks

## 1. Lock-gated owner signalling (design D1–D3)

- [ ] 1.1 Create `crates/mbv-daemon/src/owner_lock.rs`. Move `pid_file()` there from `core.rs`
  (re-export it from the crate root under the same name, so `mbv_daemon::pid_file` callers
  compile unchanged). Add `locked_owner_pid(&Path) -> Option<u32>`, `signal_owner(&Path) ->
  Result<u32, SignalOwnerError>`, and a hand-written `SignalOwnerError { NoOwner, Signal { pid,
  error } }` whose `Display` text matches today's `single_instance::TerminateOwnerError`
  messages. The probe opens the file read-only and never creates it. Verify with tests in
  `crates/mbv-daemon/tests/` (the crate's integration binary, new `owner_lock` submodule): a temp
  file that holds a PID and that the test keeps `flock`ed → `Some(pid)`; the same file unlocked
  → `None`; a missing file → `None`. No test calls `signal_owner`. Pass:
  `cargo nextest run -p mbv-daemon`.
- [ ] 1.2 In `src/single_instance.rs`, delete `terminate_owner`, `read_pid` (if unused) and
  `TerminateOwnerError`. Point both callers in `src/main.rs` (the `mbv -q` path and the
  restart-path call near line 541) at `mbv_daemon::owner_lock::signal_owner`, keeping their
  error handling and output. Verify: `cargo check -p mbv`; `rg terminate_owner src` is empty.
- [ ] 1.3 In `crates/mbv-daemon/src/run.rs::start_daemon`, write the PID file only when `role ==
  DaemonRole::Packaged`. Acquire an exclusive non-blocking `flock` on it first, write the PID
  after acquiring it, and keep the guard alive for the daemon's run. If the lock is held,
  refuse to start with an error naming the path. Guard the shutdown `remove_file` (`run.rs:652`)
  with the same role check. Verify: `cargo check -p mbv-daemon`. Reading the diff shows no
  `pid_file()` write for `DaemonRole::Local`.
- [ ] 1.4 In `crates/mbvd/src/main.rs`: `stop_daemon` sets `MBV_SYSTEM=1` (same `SAFETY` comment
  as `connect_emby`), calls `mbv_daemon::owner_lock::signal_owner(&pid_file())`, maps errors to
  `DaemonError`, and no longer runs the `kill` command or removes the file.
  `daemon_running()` becomes `locked_owner_pid(&pid_file()).is_some()`. Verify: `cargo check -p
  mbvd`; `rg 'Command::new\("kill"\)' crates/mbvd` is empty.
- [ ] 1.5 Write `docs/invariants/21-signal-only-a-held-owner-lock.md` in the style of the
  existing entries: a PID is signalled only while its flock is held, and each Owner role has
  exactly one PID record (Local: single-instance lock file; Packaged: `mbv.pid`). Cite #915 and
  the `mbvd --quit` / Local `mbv.pid` collision. Verify: the file exists and names
  `owner_lock::locked_owner_pid` as the enforcement site.
- [ ] 1.6 Run `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings`. Both must
  be clean. Commit group 1.

## 2. Private runtime directory (design D4)

- [ ] 2.1 Add `libc.workspace = true` to `crates/mbv-config/Cargo.toml`. In
  `crates/mbv-config/src/paths.rs`, change the `runtime_dir()` fallback to
  `/tmp/mbv-<getuid()>`, and add `pub fn owner_lock_path() -> PathBuf` (`<runtime>/mbv.lock`)
  next to `control_socket_path()`. Verify: `cargo check -p mbv-config`.
- [ ] 2.2 Add `pub fn ensure_runtime_dir() -> Result<(), RuntimeDirError>` plus a private
  `check_private_dir(path, uid)` using `symlink_metadata` (symlink, not-a-dir, foreign owner,
  mode `& 0o077`), and a hand-written `RuntimeDirError` naming the path and the reason. It does
  nothing when `XDG_RUNTIME_DIR` is set or the instance is a system instance. Verify with
  `src`-side tests for the private `check_private_dir` as one `#[case]` table: own 0700 dir →
  Ok; 0755 → Permissive; symlink to an own 0700 dir → Symlink; regular file → NotADirectory;
  own dir checked with `uid + 1` → ForeignOwner. Pass: `cargo nextest run -p mbv-config`.
- [ ] 2.3 Delete `single_instance::runtime_dir()` and `lock_path()`. Replace every
  `single_instance::lock_path()` caller (`src/main.rs`, `src/local_daemon.rs`,
  `src/app/dispatch/session/daemon_restart.rs`, `src/app/dispatch/session/switch.rs`) with
  `mbv_config::owner_lock_path()`. Update the module doc comment's `$XDG_RUNTIME_DIR/mbv.lock`
  wording. Verify: `cargo check -p mbv`; `rg 'fn runtime_dir' src` is empty.
- [ ] 2.4 Call `mbv_config::ensure_runtime_dir()` at the start of `main` in `src/main.rs`
  (before the `-q` branch and single-instance resolution), in `run_local_daemon_main`
  (`src/local_daemon.rs`), and on mbvd's `Serve` path before `daemon_running()`. Each prints
  the error and exits non-zero. Verify: `cargo check -p mbv -p mbvd`.
- [ ] 2.5 Run `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings`. Both must
  be clean. Commit group 2.

## 3. Bounded ctrl connections (design D5)

- [ ] 3.1 Add `set_read_timeout(Option<Duration>)` and `set_write_timeout(Option<Duration>)` to
  `SocketStream` in `crates/mbv-net/src/stream.rs`, forwarding to both variants. Verify:
  `cargo check -p mbv-net`.
- [ ] 3.2 In `crates/mbv-daemon/src/core_ctrl_spawn.rs`, add constants `CTRL_HELLO_DEADLINE`
  (10 s) and `CTRL_WRITE_TIMEOUT` (30 s). In `spawn_ctrl_client`, set both on the accepted
  stream before `try_clone`. In `CtrlClientSession::run`, read the hello with
  `BufReader::read_line` (an error, a timeout or EOF returns, as the current `let ... else`
  does), then call `reader.get_ref().set_read_timeout(None)` before the command loop. Leave a
  one-line comment that admitted clients may be idle indefinitely. Verify: `cargo nextest run
  -p mbv-daemon` (the existing `ctrl_auth` tests still pass, which proves hello parsing is
  unchanged).
- [ ] 3.3 Run `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings`. Both must
  be clean. Commit group 3. No new test: a timeout test would need a real wait (design,
  Risks).

## 4. Warm-up bounds and single state directory (design D6–D7)

- [ ] 4.1 In `src/app/infra/image_fetch/fetch/level_warmup.rs`, build the agent with
  `native_tls_agent(HttpService::Emby, Some(5s), Some(30s))` and parse the body through
  `into_body().into_reader().take(64 * 1024 * 1024)` → `serde_json::from_reader`, keeping the
  existing `.ok()` → empty-result → `Failed` path. Verify: `cargo check -p mbv`; reading the
  diff shows no `None, None` agent left in that file.
- [ ] 4.2 Delete `state_dir()` from `src/main.rs`. Replace its callers (`crash_log_path`, the
  applog `mbv.log` path in `main.rs`, `src/local_daemon.rs:236`,
  `src/app/dispatch/session/player_event.rs:524`) with `mbv_config::state_dir()`. Before the
  `OpenOptions` in `write_crash_log` and in `src/pin.rs`'s crash-log opener, call
  `create_dir_all` on the parent and ignore its error, as the open does. Verify: `cargo check
  -p mbv`; `rg 'fn state_dir' src` is empty.
- [ ] 4.3 Run `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings`. Both must
  be clean. Run `cargo nextest run --workspace`, which must be green. Commit group 4.

## 5. Integration check

- [ ] 5.1 The user runs the manual live checks (the agent asks first and never runs these
  itself): (a) start mbv, `kill -9` the Local owner, run `mbv -q`; expect "no running
  instance" and no signal sent. (b) With mbv running, run `mbvd --quit` from a desktop shell;
  expect mbv to keep running. (c) Run `env -u XDG_RUNTIME_DIR mbv`; expect `/tmp/mbv-<uid>`
  with mode 0700. (d) Open a raw connection to the ctrl socket and send nothing; expect it to
  close after about 10 s. Done when the user confirms.

## Workflow follow-up

- Run `make check-code-file-lines` before pushing.
- Sync the delta specs and archive the change once merged.
