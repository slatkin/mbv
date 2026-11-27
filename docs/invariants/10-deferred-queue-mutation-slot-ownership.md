# Invariant 10 — Deferred queue mutations execute only through their owning transitions

## The invariant

`QueueDeferrals` in `mbv_client_core::state::queue_deferrals` owns the deferred
queue mutation that waits on a user prompt: the local-play deferral. Its
private field and transition methods keep the payload in its own lifecycle:
local play is taken only on its confirmation.

The save/discard deferral and the gated populated-queue replacement left
`QueueDeferrals` when their prompts were removed (user decision 2026-11-27:
every queue replacement runs without a confirmation or save/discard prompt).
A queue replacement executes through `request_queue_replacement` at once; the
"play locally instead" prompt is the only prompt that can still hold one.

## Why it matters

A payload executed by the wrong boundary can cause playback the user never
confirmed; a payload silently dropped or overwritten can make an explicit
Play request disappear. This ownership boundary prevents those cross-flow
mixups while preserving the intended prompt-specific behavior.

## What remains unenforced

The transition methods are named for their callers (for example,
`take_confirmed_local_play`), but visibility cannot prevent another `App`
method from calling a transition it does not own. That caller-to-transition
discipline remains conventional rather than type-enforced.
