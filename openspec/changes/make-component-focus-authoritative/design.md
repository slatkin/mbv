## Context

See `proposal.md` — Why, and `specs/interactive-component-framework/spec.md` for the focus-authority contract.

TuiRealm 4.1 already owns the mounted focus stack. `Application::active` writes `Attribute::Focus(Flag(true))` to the incoming component and `Flag(false)` to the outgoing component; `blur` and focus-stack restoration use the same path. mbv's mounted components currently implement `Component::attr` as a no-op, while shell content adapters separately pass booleans derived from `PanelFocus` into component fields or render contexts.

Those booleans happen to remain current on surfaces whose content is pushed every synchronisation pass. Event-driven surfaces such as Music and Wide TV retain the value from their last content push instead, so TuiRealm event focus and painted/component focus can disagree. Home and Audiobookshelf have the same latent ownership split. Queue participates in the same focus boundary even though its projection currently runs every pass.

The framework specification requires focus-stack behavior to be tested through the shell's real synchronisation order and `Application::tick()`. Embedded controls such as `WideMediaList` are plain Components and remain inside their mounted parent's event and focus boundary.

## Goals / Non-Goals

**Goals:**

- Establish one source for mounted-component focus across destinations, Queue, and overlays.
- Remove focus from shell-owned content snapshots and focus-only projection seams.
- Preserve component-private pane focus and selection across temporary component blur.
- Fix the reported Music keyboard and Wide TV painting defects at their shared cause.
- Cover delivery and painting through the real shell composition.

**Non-Goals:**

- Change Panel focus commands, Panel mode, keyboard precedence, or mouse eligibility.
- Give embedded controls independent `ComponentId`s or focus-stack entries.
- Clear or re-anchor component-private pane focus when Library loses focus.
- Treat semantic states such as playback emphasis, disabled rows, or selected-but-unfocused identity as framework focus.
- Change Inline Search ownership or lifecycle.

## Decisions

### 1. Consume TuiRealm focus at the mounted component boundary

Every mounted Interactive Component whose behavior or painting depends on focus will handle `Attribute::Focus(AttrValue::Flag(value))` in its existing `Component::attr` implementation and store that value as component-private framework-focus state. Unknown attributes remain no-ops unless the component already supports them.

This uses the lifecycle TuiRealm already emits for `active`, `blur`, overlay focus, and focus-stack restoration. No new focus trait, shell dispatcher, or component registry is introduced.

**Alternatives rejected:**

- Continue pushing focus booleans on every synchronisation pass: this duplicates TuiRealm state and makes correctness depend on projection cadence and ordering.
- Add focus to `InlineSearchHost` or another capability trait: focus belongs to every mounted Interactive Component, not to one embedded feature.
- Query `Application::focus()` while painting each component: components do not own the application and this would reverse the established shell/component boundary.

### 2. Separate shell-owned content from component focus

Remove mounted focus booleans from destination and Queue `set_content` arguments and from shell-built render contexts where they represent component focus. A content update may replace shell-owned rows, labels, images, loading state, and playback projections, but cannot write framework-focus state.

The owning component combines its stored framework-focus state with its existing private pane selection only at event interpretation and view construction. For example, Music derives album-versus-track treatment from its retained track focus; TV derives series-versus-episode treatment from its retained pane. When framework focus is false, neither pane paints focused even though the private pane value survives.

The implementation will audit all mounted focus-aware components rather than patch only Music and TV. Presentation booleans that do not mean mounted-component focus, such as playback semantic emphasis, remain unchanged.

**Alternative rejected:** retain the focus parameter as an idempotent fallback after adding `attr`. Two writers remain two authorities even when they usually agree, and a later content push can reintroduce the defect.

### 3. Let TuiRealm remain the keyboard-delivery guard

The focused mounted component continues to receive keyboard events through `Application::tick()`; `UiRoot` remains the permanent observer governed by the Keyboard Router. Component checks that exist only to compensate for potentially stale projected focus may be removed where TuiRealm delivery already makes them redundant. Mouse handling keeps the current ADR 0024 eligibility and click-to-focus rules because non-focused painted surfaces may legitimately receive eligible mouse events.

Embedded controls receive event delegation and a derived focused presentation from their mounted parent. They do not consume application focus attributes independently.

**Alternative rejected:** preserve duplicate key guards everywhere for defensive direct calls. Direct `Component::on` tests are not proof of composition, and redundant guards can mask a broken focus handoff.

### 4. Verify focus delivery and visible behavior together

Add a shell integration test using the substitutable event listener, the production synchronisation unit, and live `Application::tick()` delivery. It will exercise Library → Queue → Library with Music, proving that Music cannot navigate while blurred and can navigate immediately after focus returns without a click or content push.

Add focused/unfocused render assertions for Wide TV through the same shell focus transition, proving that the right rail drops its green surface and selected-row marker while retaining selection identity. Existing narrow component tests may directly characterize local rendering, but they do not replace the shell-tick test.

A focused-content audit will cover Home, Browser, Feeds, Audiobookshelf podcast/book, Music, TV, and Queue, adjusting the list only when inspection proves a field has different semantics.

## Risks / Trade-offs

- **A `focused` field may represent semantic emphasis rather than mounted focus.** → Trace each writer and render consumer before removal; change only fields derived from `PanelFocus` or `Application` focus.
- **A component may accidentally clear local pane focus on blur.** → Focus attributes update only framework-focus state; regression assertions preserve selected identity and pane state.
- **Direct component tests may depend on content setters to establish focus.** → Drive focus through `Component::attr` in narrow unit tests and retain live tick coverage for composition.
- **Overlay restoration may expose incorrect focus-stack assumptions.** → Reuse TuiRealm's existing `active`/`blur` path and run the existing blocking-overlay integration suite.
- **Concurrent destination work may change setter signatures.** → Rebase mechanically while preserving the no-focus-in-content boundary; do not add a compatibility mirror.

## Migration Plan

1. Inventory mounted component fields and parameters that are projections of Panel focus; distinguish unrelated semantic booleans.
2. Make each affected mounted component consume TuiRealm focus attributes and derive local pane/list focus from that state.
3. Remove focus parameters from content models, setters, and shell projection calls, including focus-only refresh helpers.
4. Add the Music round-trip and Wide TV visual regressions through the production synchronisation and tick path; update focused component tests to use the framework attribute.
5. Run focused component and tick suites, then the package checks, formatting, Clippy, architecture scan, and file-size check.

Rollback is a normal source revert; no persisted data, configuration, protocol, or dependency migration is involved.
