## Why

After the #365 and #367 split waves, `src/app/` uses three different naming conventions side by side in a single directory listing:

| Convention | Form | Examples |
|---|---|---|
| Prefix | `types_*` | `types_browse.rs`, `types_playback.rs`, `types_settings.rs`, `types_events.rs` |
| Suffix | `*_actions` | `queue_actions.rs`, `notify_actions.rs`, `lib_event_actions.rs`, `ws_event_actions.rs` |
| Neither | bare noun | `construct.rs`, `bootstrap.rs`, `resize.rs`, `player_event.rs`, `app_struct.rs` |

The scheme is emergent rather than decided. The next split will pick one by accident. Resolving this before remaining split issues (#368, #369, #374) land prevents more files from being added under inconsistent conventions.

## What Changes

- Decide and document a single naming convention for modules in `src/app/`
- Optionally rename existing files to match the chosen convention (compiler catches every `mod` declaration, so renames are safe but noisy)
- A documented rule with existing files grandfathered in is an acceptable outcome

## Capabilities

### New Capabilities
- `module-naming-convention`: Defines the naming rule for `src/app/` modules and documents it in `AGENTS.md` or an ADR

### Modified Capabilities
_(none)_

## Impact

- `src/app/mod.rs`: All `mod` declarations for renamed files
- `src/app/*.rs`: Any file renamed to match the convention
- `AGENTS.md` or `docs/adr/`: Documentation of the decided rule
- All files that `use` renamed modules (compiler-enforced, no manual audit needed)
