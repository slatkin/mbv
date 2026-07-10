# Tray Observer And Non-Takeover Commands

## Decision

The system tray is a special local integration point owned by the daemon process.
It does **not** participate in the daemon's ctrl-socket "connection == driver"
model from ADR 0003.

Instead, tray behavior splits into two paths:

- **Observer path.** The tray may subscribe to local daemon playback updates in
  order to render passive UI such as a `Playing` row and current title. This
  observer path is local-only, does not count as a connected client, and can
  never become the driving authority.
- **Command path.** Tray menu actions (`Play`/`Pause`, `Stop`, `Next`, `Prev`,
  `Quit`) are local control messages addressed to the owning daemon instance.
  These commands are explicitly **non-takeover**: they must not evict the
  current driving ctrl client and must not make the tray the driver.

This exception exists only for the in-process system tray surface. It does not
generalize to extra ctrl clients, remote observers, or a second reusable
observer channel for arbitrary consumers.

## Context

Issue #115 adds daemon controls to the tray. Product scope settled in grilling:

- tray always shows transport controls plus `Quit`
- show two disabled rows only when playback state is `Playing` and a title is
  available:
  - `Playing`
  - `<Title>`
- no other status rows
- idle state keeps the command menu visible; irrelevant commands no-op
- desktop "now playing started" notifications reuse existing
  `system_notifications`
- notify only when playback transitions to a new item, not on resume of the
  same item

That scope collides with ADR 0003 if implemented naively. Reusing the existing
ctrl client path would incorrectly make the tray either:

- a second connected client, which ADR 0003 forbids, or
- a command source that steals driving authority on routine tray clicks

Neither matches the intended local desktop-integration behavior.

## Considered options

- **Special-case local tray observer + non-takeover commands (chosen).** Keeps
  the tray useful without weakening exclusive ctrl ownership.
- **Reuse the ctrl-socket client path unchanged (rejected).** Would make tray
  status look like a real connection and let tray commands trigger takeover
  semantics.
- **Expose command-only tray with no playback/title rows (rejected).** Avoids
  observer semantics, but drops the lightweight now-playing signal wanted for
  #115.
- **Generalize to multi-observer clients (rejected).** Reopens the broader
  model ADR 0003 intentionally closed.

## Consequences

- The daemon needs an explicit origin distinction for tray commands versus
  normal ctrl-client commands. "Non-takeover" must be enforced in daemon logic,
  not by convention in tray code.
- Passive tray updates should come from local daemon state/broadcast plumbing,
  not by opening a ctrl connection or inventing a tray-specific request/response
  API.
- The tray can reuse the existing persisted `system_notifications` config flag
  for "new playing item" notifications instead of adding a second tray-specific
  toggle.
- Future CLI work such as #108 remains in the normal command/takeover model;
  this ADR does not give `mbvc` or other automation a free observer/non-driver
  status.
