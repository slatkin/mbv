# Proposal

## Why

The Pinned panel starts in pinwin's `on-demand` keyboard mode. The user has no keyboard path to give it focus, so they must click it. pinwin 0.1.0 adds `Panel::request_focus()` for this (issue #882), but the library has no transport. The host must expose the request itself.

The original pinned design was meant to allow only one Pinned panel at a time. The archived design and the live `pinned-launch` spec state the opposite ("each pinned launch SHALL open its own panel"), and no code enforces a limit. A focus command needs one target, so this change also makes the one-panel rule a hard, enforced constraint.

## What Changes

- Change the `pinwin` dependency from the deleted tag `0.2.2` to tag `0.1.0`. That tag is the first one with `Panel::request_focus()`. All older tags were deleted.
- **BREAKING**: At most one Pinned launch runs per user. The first `mbv --pin` takes an exclusive lock and holds it for the life of the process. A later `mbv --pin` never opens a panel. It sends a focus request to the running panel and exits with status 0. If the running panel does not answer, it reports the reason and exits with status 1.
- The running Pinned launch listens on a Unix socket. On `focus\n` it calls `Panel::request_focus()` and answers `ok\n` or `error\n`.
- Add `mbv --focus`. It sends the focus request to the running Pinned launch and exits. It exits with status 0 on `ok\n` and with status 1 otherwise. It never starts a panel, a TUI, or a Player owner.
- Document `--focus` in `mbv --help` and the README, with a compositor hotkey example.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `pinned-launch`: "Pin flag" changes from one panel per launch to at most one Pinned launch per user. A new requirement covers the focus request (`mbv --focus`, the second `mbv --pin`, the socket answer).

## Impact

- `Cargo.toml` and `Cargo.lock`: pinwin `tag = "0.1.0"` (commit `236774b`). The pinwin public API between the current lock (`b910591`) and `0.1.0` only grows. No mbv call site changes for the bump.
- `src/pin.rs` and a new child module for the lock, socket, and client.
- `src/main.rs`: `--focus` and the second-launch path, and the help text.
- `src/app/`: the pinned panel handle and one per-loop drain in the shell run loop.
- `CONTEXT.md`: the `--pin` entry says "one panel per launch", which this change reverses.
- `README.md`: hotkey binding instructions.
- No change to `mbv-daemon`, `mbvd`, or the Owner process. The panel lives in the TUI Client process.
