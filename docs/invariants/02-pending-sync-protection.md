# Invariant 2 — Pending progress protection has explicit limits

**Scope:** The policy that turns a stop/completion report's wire acceptance bool into queue protection (`crates/mbv-player/src/run/queue.rs`), the lifetime of an unconfirmed snapshot, and progress updates that do not report to the server.

## What types do not enforce

- **Optimistic acceptance is a player-thread policy.** The wire bool is interpreted by the player thread when deciding whether to arm pending-sync protection. The background stop-report path may report acceptance before knowing the eventual server outcome. This deliberately favors retaining the reported progress over adopting a potentially stale refresh. Callers must not treat the bool as proof that the server has persisted the report; changing this policy can change which refreshes are protected.
- **An unconfirmed snapshot has no expiry.** If the server never confirms a protected progress snapshot, it remains protected until confirmation or structural removal. There is no age- or refresh-count-based timeout, so a slot missing from the server can remain in the queue indefinitely.
- **Non-report `apply_progress` writers do not alter protection.** Applying observed progress without a corresponding accepted stop/completion report leaves existing protection untouched: it neither arms protection nor clears an existing snapshot. A future change to those writer semantics must preserve this distinction intentionally.

These are policy and lifecycle boundaries, not guarantees established by the queue's types. See `crates/mbv-player/src/run/queue.rs` for the player-thread acceptance decision.
