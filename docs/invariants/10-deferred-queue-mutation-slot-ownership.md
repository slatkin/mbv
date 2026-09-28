# Invariant 10 — Deferred queue mutations execute only through their owning transitions

## The invariant

`QueueDeferrals` in `src/app/state/queue_deferrals.rs` owns the three deferred
queue mutations: the save/discard deferral, the gated populated-queue
replacement, and the local-play deferral. Its private fields and transition
methods keep each payload in its own lifecycle: save completion requires the
bound mutation ID, the gated replacement is taken only on confirmation, and
local play is taken only on its confirmation.

## Why it matters

A payload executed by the wrong boundary can cause playback the user never
confirmed; a payload silently dropped or overwritten can make an explicit
Play request disappear. This ownership boundary prevents those cross-flow
mixups while preserving the intended prompt-specific behavior.

## What remains unenforced

The transition methods are named for their callers (for example,
`take_on_save_complete` and `take_confirmed_replacement`), but visibility
cannot prevent another `App` method from calling a transition it does not
own. That caller-to-transition discipline remains conventional rather than
type-enforced.
