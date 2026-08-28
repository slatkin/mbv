//! Key-policy precedence table — the direct successor to `CONTEXT_STACK`'s
//! ordering (in `input_resolver.rs`), minus the per-surface handler bodies.
//! This table documents how each input context maps to its TuiRealm owner and
//! gate, and will be used to generate mutually-exclusive `SubClause` guards
//! when subscriptions are wired per-surface (tasks 3.x/4.x).
//!
//! Pre-wiring scaffolding: the table and its types are declared here as tested
//! dead code. Live wiring is deferred to per-surface conversion.
//!
//! **Static-table proof path (decision D15, task 5.4).** The six
//! precedence/mouse proofs are asserted *directly against this table* plus the
//! structural mouse checks recorded in task 5.4 — `Component::perform(Cmd)` is
//! **not** adopted as the table's execution path, because that adoption would
//! broaden this unit into the later framework teardown.
//! `#![allow(dead_code)]` therefore stays in place, naming this decision.

#![allow(dead_code)]

use super::components::component_id::OverlayId;
use super::components::{ComponentId, ATTR_BLOCKING_OVERLAY_ACTIVE};
use tuirealm::props::{AttrValue, Attribute};
use tuirealm::subscription::SubClause;

/// One layer of the key-policy precedence stack: a name for assertions/debugging,
/// the TuiRealm owner that receives the key, the gate that determines
/// eligibility, and whether this context blocks lower-priority contexts when
/// active.
#[derive(Debug, Clone)]
pub(super) struct KeyPolicyEntry {
    pub name: &'static str,
    pub owner: KeyPolicyOwner,
    pub gate: KeyPolicyGate,
    pub blocking: bool,
}

/// How a key-policy entry maps to a TuiRealm component.
#[derive(Debug, Clone)]
pub(super) enum KeyPolicyOwner {
    /// The active/focused component receives the key first.
    Active(Option<ComponentId>),
    /// A gated subscription on the named component.
    Sub(ComponentId),
}

/// The eligibility condition for a key-policy entry. When subscriptions are
/// wired per-surface, these translate to TuiRealm `SubClause` guards.
#[derive(Debug, Clone)]
pub(super) enum KeyPolicyGate {
    /// Always eligible (global binding).
    Always,
    /// Eligible when this component is mounted.
    IsMounted(ComponentId),
    /// Eligible when this component is NOT mounted (e.g., blocking overlay is
    /// down).
    NotIsMounted(ComponentId),
    /// Custom gate described by string; wired at surface conversion.
    Custom(&'static str),
    /// A live TuiRealm attribute clause for state resolved by the shell.
    HasAttrValue(ComponentId, Attribute, AttrValue),
    /// The negated form of a live TuiRealm attribute clause.
    NotHasAttrValue(ComponentId, Attribute, AttrValue),
}

impl KeyPolicyGate {
    pub(super) fn sub_clause(&self) -> Option<SubClause<ComponentId>> {
        match self {
            Self::HasAttrValue(id, attr, value) => {
                Some(SubClause::HasAttrValue(id.clone(), *attr, value.clone()))
            }
            Self::NotHasAttrValue(id, attr, value) => Some(SubClause::Not(Box::new(
                SubClause::HasAttrValue(id.clone(), *attr, value.clone()),
            ))),
            _ => None,
        }
    }
}

/// The full key-policy precedence table, first-match-wins, mirroring the exact
/// order of `CONTEXT_STACK` in `input_resolver.rs`. See design Table B.
pub(super) const KEY_POLICY: &[KeyPolicyEntry] = &[
    // 1–6 — Blocking overlays and modals (highest priority)
    KeyPolicyEntry {
        name: "selection_modal",
        owner: KeyPolicyOwner::Active(Some(ComponentId::Overlay(OverlayId::SelectionModal))),
        gate: KeyPolicyGate::IsMounted(ComponentId::Overlay(OverlayId::SelectionModal)),
        blocking: true,
    },
    // Global overlay-open keys (only when no blocking overlay is up)
    KeyPolicyEntry {
        name: "global_overlay_open",
        owner: KeyPolicyOwner::Sub(ComponentId::UiRoot),
        gate: KeyPolicyGate::NotHasAttrValue(
            ComponentId::Playback,
            ATTR_BLOCKING_OVERLAY_ACTIVE,
            AttrValue::Flag(true),
        ),
        blocking: false,
    },
    // 12 — Queue column width
    KeyPolicyEntry {
        name: "queue_column_width",
        owner: KeyPolicyOwner::Sub(ComponentId::Queue),
        gate: KeyPolicyGate::IsMounted(ComponentId::Queue),
        blocking: false,
    },
    // 15 — Panel mode cycle
    KeyPolicyEntry {
        name: "panel_mode_cycle_x",
        owner: KeyPolicyOwner::Sub(ComponentId::Library),
        gate: KeyPolicyGate::IsMounted(ComponentId::Library),
        blocking: false,
    },
    // 18 — Clear-queue prompt. Despite the name, `handle_key_clear_queue_prompt`
    // claims 'c' (sans Alt) unconditionally -- it never checks a "prompt
    // visible" flag, it always shows the confirmation itself. Always-eligible,
    // like `ctrl_l_force_clear`/`f5_refresh` below.
    KeyPolicyEntry {
        name: "clear_queue_prompt_c",
        owner: KeyPolicyOwner::Sub(ComponentId::Queue),
        gate: KeyPolicyGate::Always,
        blocking: false,
    },
    // 19 — Visualizer. `handle_key_visualizer` claims 'v' (no modifiers)
    // unconditionally and toggles the visualizer itself -- it doesn't require
    // the visualizer to already be active. Always-eligible.
    KeyPolicyEntry {
        name: "visualizer",
        owner: KeyPolicyOwner::Sub(ComponentId::Playback),
        gate: KeyPolicyGate::Always,
        blocking: false,
    },
    // 20 — Playback transport. `handle_playback_key` resolves eligibility
    // per-key via `resolve_key(InputContext::Playback, snapshot, chord)` — a
    // command table, not a fixed boolean — so unlike the two prompt gates
    // above, this cannot be represented as a static `SubClause::HasAttrValue`
    // check; the real gate depends on which key was pressed. A future
    // subscriber that needs to be mutually exclusive with this entry must
    // either run this same resolution itself or defer to the legacy handler
    // for any key it doesn't unambiguously own.
    KeyPolicyEntry {
        name: "playback",
        owner: KeyPolicyOwner::Sub(ComponentId::Playback),
        gate: KeyPolicyGate::Custom("player_active/remote gate"),
        blocking: false,
    },
    // 21–22 — Global always-on bindings
    KeyPolicyEntry {
        name: "ctrl_l_force_clear",
        owner: KeyPolicyOwner::Sub(ComponentId::UiRoot),
        gate: KeyPolicyGate::Always,
        blocking: false,
    },
    KeyPolicyEntry {
        name: "f5_refresh",
        owner: KeyPolicyOwner::Sub(ComponentId::UiRoot),
        gate: KeyPolicyGate::Always,
        blocking: false,
    },
    // 23 — View dispatch (catch-all; resolves to focused Library leaf at runtime).
    // (The former `album_track_mode` entry was removed with the legacy
    // track-mutation path: inline album track mode now lives in the active
    // MusicWorkspaceComponent, which owns its keys locally and routes only
    // activation across the boundary — there is no precedence-gate entry left
    // for it. Task 5.3d, Album track focus.)
    KeyPolicyEntry {
        name: "view_dispatch",
        owner: KeyPolicyOwner::Active(None),
        gate: KeyPolicyGate::Custom("focused Library leaf"),
        blocking: false,
    },
];

// ---------------------------------------------------------------------------
// Mouse subscription pattern (design D8)
// ---------------------------------------------------------------------------
//
// Per-surface conversion tasks (3.x/4.x) follow this pattern for mouse
// routing:
//
// * Each currently visible top-level region (Queue, the active Library
//   destination, an overlay) subscribes with `Sub::new(EventClause::Any,
//   guard)`.
//
// * On a mouse event, every such subscriber's `on()` runs, but each returns
//   a `Msg` only if `(column, row)` falls in the geometry it painted during
//   `view()`. All others return `None`.
//
// * This yields simultaneous Queue+Library mouse targets with no
//   destination-tag gate.
//
// * Geometry is component-owned: a component records the `Rect`s/rows it
//   paints during `view()` in its private state and hit-tests against exactly
//   that, so painting and hit-testing cannot drift.
//
// * Overlay blocking: while a blocking overlay is mounted, the region
//   subscriptions carry `Not(IsMounted(overlay))` guards, so underlying
//   regions receive no mouse event and cannot mutate.
//
// * During the migration, converted surfaces own mouse hit-testing and the
//   shell runs any remaining App effects. This pattern is wired per-surface.
//
// * Note: `EventClause::Mouse` in TuiRealm 4.1 only range-checks
//   `column`/`row` and ignores `kind`/`modifiers`, so `EventClause::Any` is
//   the correct subscription clause.

#[cfg(test)]
mod tests {
    use super::super::input_resolver::CONTEXT_STACK;
    use super::*;

    #[test]
    fn key_policy_order_matches_context_stack() {
        let policy_names: Vec<&str> = KEY_POLICY
            .iter()
            .map(|entry| entry.name)
            .filter(|name| *name != "selection_modal")
            .collect();
        let context_names: Vec<&str> = CONTEXT_STACK.iter().map(|entry| entry.name).collect();
        assert_eq!(
            policy_names, context_names,
            "KEY_POLICY must mirror the remaining legacy CONTEXT_STACK order; converted TuiRealm surfaces are excluded"
        );
    }

    // Contract 1 (ADR 0002 Swallow, expressed as first-match table order): a
    // blocking context must sit above every parent/global (Sub) binding so it
    // claims the key before any lower-priority or global binding can. The
    // ordering test above already pins the table's order against
    // CONTEXT_STACK; this proves the *precedence relationship* among the
    // entries that the order implies, plus the gate that makes the swallow
    // real for the global binding.
    #[test]
    fn blocking_contexts_swallow_before_lower_and_global_entries() {
        let blocking_indices: Vec<usize> = KEY_POLICY
            .iter()
            .enumerate()
            .filter(|(_, e)| e.blocking)
            .map(|(i, _)| i)
            .collect();
        assert!(
            !blocking_indices.is_empty(),
            "KEY_POLICY must retain at least one blocking context"
        );

        let sub_indices: Vec<usize> = KEY_POLICY
            .iter()
            .enumerate()
            .filter(|(_, e)| matches!(e.owner, KeyPolicyOwner::Sub(_)))
            .map(|(i, _)| i)
            .collect();

        for &b in &blocking_indices {
            for &s in &sub_indices {
                assert!(
                    b < s,
                    "blocking context at index {b} must precede sub (parent/global) entry at index {s}"
                );
            }
        }

        // The swallow mechanism for the global overlay-open binding: while a
        // blocking overlay is active (ATTR_BLOCKING_OVERLAY_ACTIVE on Playback),
        // the global sub is gated off, so the key dies with the overlay and
        // cannot reach a lower/global handler.
        let global_open = KEY_POLICY
            .iter()
            .find(|e| e.name == "global_overlay_open")
            .expect("global_overlay_open binding must exist");
        assert!(
            matches!(
                &global_open.gate,
                KeyPolicyGate::NotHasAttrValue(
                    ComponentId::Playback,
                    ATTR_BLOCKING_OVERLAY_ACTIVE,
                    AttrValue::Flag(true),
                )
            ),
            "global_overlay_open must be gated off while a blocking overlay is active"
        );
    }

    // Contract 2 (parent/global bindings retain their precedence and owners):
    // each parent/global binding keeps the owner design Table B assigns, and
    // the unconditional (Always) global bindings stay the lowest-priority subs
    // — every parent binding outranks them, mirroring CONTEXT_STACK's
    // parent-over-global order. (The gated global_overlay_open binding sits
    // above the parents by design; its swallow gate is proven separately.)
    #[test]
    fn parent_and_global_bindings_retain_precedence_and_owners() {
        let expected_owner: &[(&str, ComponentId)] = &[
            ("queue_column_width", ComponentId::Queue),
            ("panel_mode_cycle_x", ComponentId::Library),
            ("clear_queue_prompt_c", ComponentId::Queue),
            ("confirm_skip_intro", ComponentId::Playback),
            ("confirm_next_up", ComponentId::Playback),
            ("visualizer", ComponentId::Playback),
            ("playback", ComponentId::Playback),
            ("global_overlay_open", ComponentId::UiRoot),
            ("ctrl_l_force_clear", ComponentId::UiRoot),
            ("f5_refresh", ComponentId::UiRoot),
        ];
        for (name, owner) in expected_owner {
            let entry = KEY_POLICY
                .iter()
                .find(|e| e.name == *name)
                .unwrap_or_else(|| panic!("binding {name} missing from KEY_POLICY"));
            assert!(
                matches!(entry.owner, KeyPolicyOwner::Sub(ref c) if *c == *owner),
                "binding {name} must retain its documented owner"
            );
        }

        let parent_indices: Vec<usize> = KEY_POLICY
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                matches!(
                    e.owner,
                    KeyPolicyOwner::Sub(
                        ComponentId::Queue | ComponentId::Library | ComponentId::Playback
                    )
                )
            })
            .map(|(i, _)| i)
            .collect();
        let always_global_indices: Vec<usize> = KEY_POLICY
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                matches!(e.owner, KeyPolicyOwner::Sub(ComponentId::UiRoot))
                    && matches!(e.gate, KeyPolicyGate::Always)
            })
            .map(|(i, _)| i)
            .collect();
        for &p in &parent_indices {
            for &g in &always_global_indices {
                assert!(
                    p < g,
                    "parent binding at index {p} must precede always-global binding at index {g}"
                );
            }
        }
    }
}
