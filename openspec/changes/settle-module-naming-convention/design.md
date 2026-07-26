## Context

After #365 and #367, `src/app/` has ~80 modules with three naming conventions coexisting:
- `types_*` prefix (8 files): type-definition modules
- `*_actions` suffix (18 files): action/handler modules
- Bare nouns (~54 files): everything else

The conventions are mostly consistent within their categories. The issue is that no rule was explicitly decided, so future splits may introduce new conventions by accident.

## Goals / Non-Goals

**Goals:**
- Document a clear naming rule for `src/app/` modules
- Prevent inconsistent naming in future module splits (#368, #369, #374)

**Non-Goals:**
- Mass-renaming all existing files (noisy, low value)
- Changing the `render/` subdirectory conventions

## Decisions

**Decision: Keep the existing two-convention scheme with a documented rule.**

The current conventions are already mostly consistent:
- `types_*` prefix for type-definition modules (8 files)
- `*_actions` suffix for action/handler modules (18 files)
- Bare nouns for all other modules (constructors, state, events, input handling, etc.)

**Rationale:**
1. The `types_` prefix is useful — it clusters type-definition modules together in `ls` output and editor file pickers
2. The `_actions` suffix is useful — it immediately signals "this module handles user/system actions"
3. Forcing a single convention (all suffix or all prefix) would require renaming ~26 files with no functional benefit
4. The bare-noun convention works well for the remaining modules that don't fit the other two categories

**Alternatives considered:**
- *All suffixes (`*_types`, `*_actions`, `*_state`)*: Would rename 8 type files for no gain. The `types_` prefix is more discoverable.
- *All prefixes (`types_*`, `actions_*`)*: Would rename 18 action files. The `_actions` suffix reads more naturally in Rust code.
- *Bare nouns only*: Loses the useful clustering of types and actions.

**Rule to document:**
> Modules in `src/app/` follow two naming conventions:
> - **Prefix `types_`**: For modules that primarily define type aliases, enums, or structs used across the app (e.g., `types_browse.rs`, `types_playback.rs`)
> - **Suffix `_actions`**: For modules that implement action handlers or event dispatch logic (e.g., `queue_actions.rs`, `notify_actions.rs`)
> - **Bare noun**: For all other modules (constructors, state, input handling, etc.)
>
> Future module splits should follow this existing pattern. When in doubt, prefer bare nouns — the prefix/suffix conventions are reserved for modules that genuinely benefit from categorization.

## Risks / Trade-offs

- **Risk**: Developers may still pick a new convention by accident → **Mitigation**: The rule is written down in `AGENTS.md` (or ADR) and referenced in the issue tracker
- **Risk**: The two-convention scheme is slightly more complex than a single rule → **Mitigation**: The complexity is low (two simple patterns) and the benefit (discoverability) is real
