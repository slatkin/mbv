## Why

When the Emby server is unreachable during startup, mbv hangs for up to 15 seconds before falling back to a login screen that looks like an authentication problem. The login screen is misleading because the user's cached credentials are still valid — the server is just down. Worse, the current code clears the cached token on any authentication failure (not just 401/403), so even when the server comes back, the user has to re-authenticate instead of reusing their still-valid token.

## What Changes

- **Wall-clock bound reduced from 15s to 5s.** The `AUTHENTICATE_HARD_BOUND` constant drops to 5s, matching the connect timeout. This is the worst-case wait before mbv reports the server is unreachable.

- **Connectivity-class failures exit to command line with a message.** When authentication fails due to network issues (timeout, connection refused, DNS error, TLS error, etc.), mbv prints a clear error to stderr and exits non-zero. No login screen appears. The error message follows existing `main.rs` patterns: just the facts, no instructions.

- **Login screen only shown for credential issues.** The login form appears only when the user actually needs to authenticate:
  - 401/403 from Emby (cached token expired or rejected)
  - No cached credentials exist (first run)
  - Missing server URL in config

- **Token preservation on connectivity failures.** The `authenticate` function currently clears the cached token on any error. This change makes it clear the token only on 401/403, not on timeout/connect errors. This way, when the server comes back, the user's still-valid credentials work without re-authentication.

- **Scope limited to Emby-startup.** This change addresses the Emby authentication path only. Daemon-connect UX and mid-session connection failures are separate concerns.

## Capabilities

### New Capabilities

- `startup-auth-graceful-failure`: Covers the graceful failure behavior when Emby is unavailable during startup authentication. Includes the reduced wall-clock bound, exit-to-CLI behavior for connectivity failures, token preservation, and the distinction between connectivity-class and credential-class failures.

### Modified Capabilities

None. This is a new capability; no existing specs are changing.

## Impact

**Affected code:**
- `src/main.rs`: `authenticate_or_login` flow, `classify_auth_failure` function, error message format
- `crates/mbv-core/src/api_client_auth.rs`: `AUTHENTICATE_HARD_BOUND` constant (15s → 5s), `authenticate` function (token-clearing logic)

**Affected behavior:**
- Startup UX when Emby server is unreachable: 15s hang → 5s wait, then exit with message
- Login screen appearance: no longer shown for connectivity failures
- Token cache: preserved on connectivity failures, cleared only on 401/403

**No breaking changes** to APIs, dependencies, or user-facing configuration.
