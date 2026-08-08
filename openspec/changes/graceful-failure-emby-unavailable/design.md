## Context

The startup authentication path currently has three issues that compound when the Emby server is unreachable:

1. **Long wait**: `AUTHENTICATE_HARD_BOUND` is 15s, much longer than the 5s connect timeout, so an unresponsive server blocks the user for 15 seconds before any feedback.

2. **Misleading UX**: Any authentication failure (including connectivity errors) falls through to the login screen. The login form suggests "type your password to fix this" when the real problem is "the server is down."

3. **Token loss**: The `authenticate()` function clears the in-memory token on any error (line 97), not just 401/403. The on-disk cache is only cleared on 401/403 (line 91), but the in-memory token is gone, so even if the server comes back, the current process can't retry with the cached token.

See `proposal.md` for the full motivation and user-facing impact.

## Goals / Non-Goals

**Goals:**
- Reduce worst-case startup wait from 15s to 5s when the server is unreachable.
- Exit to command line with a clear error message on connectivity failures, instead of showing the login screen.
- Preserve the cached token (both in-memory and on-disk) on connectivity failures so the user doesn't have to re-authenticate when the server recovers.
- Show the login screen only when the user actually needs to provide credentials: 401/403, no cached credentials, or missing server URL.

**Non-Goals:**
- Changing the login screen UI itself (beyond not showing it for connectivity failures).
- Adding retry logic or backoff on connectivity failures.
- Changing the login screen retry path (`authenticate_credentials`). That path only runs when the login screen is shown, which is now only for credential issues where the server is up.
- Handling mid-session connection failures or daemon-connect UX. Those are separate concerns.
- Adding configuration knobs for the timeout bound. 5s is hardcoded.

## Decisions

### Decision 1: Failure classification uses string matching on error messages

**Choice**: Classify connectivity vs. credential failures by matching known error strings returned by `authenticate()`, similar to the existing `classify_auth_failure()` pattern.

**Rationale**: The `authenticate()` function already returns `Result<(), String>` with distinct error messages:
- `"Cached credentials expired"` → 401/403, credential failure
- `"No cached credentials"` / `"No server URL configured"` → first run, credential failure
- `"Cached credential validation failed: {ureq_error}"` → everything else, connectivity failure

The ureq error details (timeout, connection refused, DNS, TLS) are embedded in the string. String matching is simple, matches the existing pattern, and avoids a larger refactor to change the return type to an enum.

**Alternatives considered**:
- **Return type `Result<(), AuthError>` with variants**: Cleaner, but requires changes to `authenticate()`, `authenticate_bounded()`, and `run_with_hard_bound()`. More invasive for a focused fix.
- **Add a separate function `authenticate_classified()`**: Avoids changing the existing API, but duplicates logic and still requires string matching internally.

### Decision 2: Token preservation happens in `authenticate()`, not in the caller

**Choice**: Move the token-clearing logic inside `authenticate()` so it only clears on 401/403. The caller (`authenticate_or_login`) doesn't need to know about token preservation.

**Rationale**: The `authenticate()` function already has the match arms that distinguish 401/403 from other errors. Moving the token-clearing logic into those arms keeps the concern local and avoids leaking classification logic into `main.rs`.

Currently:
```rust
Err(e) => {
    self.token.clear();  // ← clears on any error
    self.user_id.clear();
    Err(format!("Cached credential validation failed: {e}"))
}
```

After:
```rust
Err(e) => {
    // Don't clear token on connectivity errors — preserve for retry
    Err(format!("Cached credential validation failed: {e}"))
}
```

The in-memory token is preserved. The on-disk cache is already only cleared on 401/403 (line 91), so no change needed there.

**Alternatives considered**:
- **Clear token in `authenticate_or_login` based on classification**: Keeps `authenticate()` unchanged, but spreads the logic across two files and makes the contract less clear.
- **Add a `preserve_token` flag to `authenticate()`**: More explicit, but adds a parameter that's only used in one call site. The default behavior should be "preserve on connectivity errors," so the flag is unnecessary.

### Decision 3: `authenticate_or_login` branches on classification, not on `Option<String>`

**Choice**: Refactor `classify_auth_failure` into a function that returns an enum or a tuple indicating the failure class (connectivity vs. credential) and the error message. `authenticate_or_login` branches on this to decide: exit or show login.

**Rationale**: The current `classify_auth_failure` returns `Option<String>` where:
- `None` → first run, show login
- `Some("... expired ...")` → credential failure, show login
- `Some("... could not reach ...")` → connectivity failure, show login (but shouldn't)

The new behavior needs to distinguish connectivity from credential. Returning `Option<String>` conflates these. A clearer return type:

```rust
enum AuthFailure {
    FirstRun,              // no cached creds, show login
    CredentialsExpired,    // 401/403, show login
    ServerUnreachable(String), // connectivity error, exit with message
}
```

`authenticate_or_login` matches on this enum and either exits or shows login.

**Alternatives considered**:
- **Keep `Option<String>` and add a boolean flag**: `(Option<String>, bool)` where the bool indicates "exit vs. login." Less clear than an enum, but avoids introducing a new type.
- **Inline the classification in `authenticate_or_login`**: Avoids a separate function, but makes `authenticate_or_login` harder to read and test.

### Decision 4: Error message format is minimal and consistent

**Choice**: The error message printed on connectivity failure follows the existing pattern in `main.rs`:
```
mbv: could not reach {server_url}: {error_details}
```

No recovery instructions, no suggestions. Just the facts.

**Rationale**: Consistency with other startup errors in `main.rs` (e.g., `mbv: failed to connect to daemon endpoint`, `mbv: another mbv instance already owns playback`). The user can infer what to do from the message. Adding instructions ("check your configuration", "wait and retry") adds verbosity without adding information.

The `{error_details}` part comes from the `authenticate()` error string, which includes the ureq error details (e.g., "timed out after 5s", "connection refused").

### Decision 5: `AUTHENTICATE_HARD_BOUND` is a constant, not a configuration knob

**Choice**: The 5s bound is hardcoded as a constant (`pub const AUTHENTICATE_HARD_BOUND: Duration = Duration::from_secs(5)`). No configuration option.

**Rationale**: The timeout bound is a balance between "fail fast when the server is down" and "don't give up too early on a slow server." 5s is a reasonable default for most networks. Users with unusually slow servers can recompile with a different value, but adding a config knob for this edge case adds complexity (config parsing, documentation, testing) for a rare use case.

If this becomes a real pain point, it can be made configurable later. For now, 5s is the right default.

## Risks / Trade-offs

**[Risk] 5s is too short for slow servers** → Users with high-latency connections or slow Emby servers might see false-positive "server unreachable" errors. Mitigation: 5s is generous for most LAN/internet scenarios. If this becomes a real issue, the bound can be raised or made configurable. The cost of a false positive is low (user restarts mbv), and the benefit of a short bound (fast failure when the server is down) outweighs it.

**[Risk] String matching for failure classification is fragile** → If the error strings change, the classification breaks. Mitigation: The error strings are already used for classification in `classify_auth_failure`, so this isn't a new risk. The strings are stable (they're part of the API), and unit tests verify the classification logic.

**[Risk] Token preservation might hide real auth failures** → If the token is corrupted but the server is up, the user might see "server unreachable" instead of "credentials expired." Mitigation: The classification is based on the ureq error, not the token state. A corrupted token would result in a 401/403, which is classified as a credential failure, not a connectivity failure. Token preservation only affects the behavior on network errors.

**[Risk] Exit to CLI might confuse users who expect the login screen** → Users accustomed to the login screen might not understand why mbv exited. Mitigation: The error message is clear ("could not reach {url}"), and users can restart mbv when the server is back. The login screen is still shown for real credential failures, so the behavior is consistent with user expectations for that case.

**[Trade-off] No retry logic on connectivity failures** → The user has to manually restart mbv. Mitigation: Automatic retry with backoff adds complexity (state management, UI feedback, cancellation) for a rare case. Manual restart is simple and predictable. If this becomes a pain point, it can be added later.

**[Trade-off] Token preservation means the in-memory token might be stale** → If the server comes back but the token is actually invalid (e.g., server-side revocation), the user might see "server unreachable" on the first try, then "credentials expired" on the second. Mitigation: This is the correct behavior. The first error is "server unreachable" because the server was unreachable. The second error is "credentials expired" because the server came back and rejected the token. The user can then re-authenticate.
