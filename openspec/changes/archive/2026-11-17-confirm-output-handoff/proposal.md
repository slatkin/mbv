# Proposal

## Why

When the user selects a playback target while a slot is active on the current output, the Owner
hands the playing slot over by itself, with no question. The user report: selecting a Cast
receiver, selecting an Emby Session (with or without a direct endpoint), and selecting the local
target while a receiver or a Session plays all moved the playing audio without asking. The spec
already has a hand-off fork, but only for an Emby Session that advertises a direct endpoint.

## What Changes

- **Continue-playback prompt.** Whenever a user playback-output selection would hand off an
  active slot, the Client asks first, in both Clients (they share the App dispatch and the
  confirm modal). The prompt names the target — "Continue playback on SHIELD?" — with Yes and
  No buttons. Yes runs today's hand-off unchanged. No cancels: nothing changes and the current
  output keeps playing (for a direct-endpoint Emby Session the existing hand-off fork stays, and
  its decline keeps attaching the direct endpoint).
- **Covered selections.** A Cast target, an Emby Session without a direct endpoint, the local
  target while a Cast receiver or an Emby Session is the output, and any switch between remote
  outputs. The prompt is one shared App-side gate, not one copy per selection site.
- **No prompt** when nothing is active on the output, and none when the selected device already
  is the output (the Owner's no-op hand-off). Queue Hand off / Take are untouched: they already
  ask, and they move a queue between owners, which is a different relation.

## Impact

- **Affected specs:** `cast-session-control` (the target-switch scenarios gain the ask),
  `owner-playback-output` (the Client-side gate over the existing hand-off semantics).
- **Affected code:** the shared confirm gate (`ConfirmAction::ContinueOutputHandOff`), the Cast
  selection, the Session activation, and the detach-to-local sites. The Owner protocol and the
  Owner-side hand-off code are unchanged.
