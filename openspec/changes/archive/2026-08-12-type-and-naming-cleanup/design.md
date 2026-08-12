## Context

See proposal.md for motivation. The state-machine work (#449, PR #455) established
the pattern — `player_run_state.rs` already holds `LoadState`, `StopReport`,
`NextUp`, `IntroState`, `StartupPause`. `QueueSlotId` in `playback_queue.rs` is
the existing newtype precedent.

The `power` prefix is attached to seven unrelated concepts (queue rendering, music
grouping, home feed, cards, letter pills, panel layout, continue-watching tab). It
named a "power view" that was once distinct from a "standard view"; the standard
view was removed and the power view became the only view. The prefix now carries
zero information.

## Goals / Non-Goals

**Goals:**
- Compiler-enforced ID separation in mbv-core (crossed IDs become type errors)
- Single source of truth for queue-mode state (derive, don't store)
- Remove the dead `power` naming concept from identifiers and filenames

**Non-Goals:**
- ID newtypes in `src/` (TUI layer) — follow-up work; the TUI passes IDs through
  without crossing them, so the risk is lower
- Renaming `powerline` — different word, unrelated concept
- Behavioral changes of any kind

## Decisions

### 1. Newtype shape: tuple struct with `Display`, no `Deref`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ItemId(String);

impl ItemId {
    pub fn new(s: impl Into<String>) -> Self { Self(s.into()) }
    pub fn as_str(&self) -> &str { &self.0 }
}
```

Match `QueueSlotId`'s style (tuple struct, explicit accessor). No `Deref<Target=str>`
— that would let newtypes silently compare with each other via `&str`, defeating
the purpose. `Display` for logging/API calls. `Serialize`/`Deserialize` if
serde is already derived on the containing type.

Alternative considered: a generic `Id<Tag>(String)` with phantom types. Rejected —
adds indirection for three types, and `QueueSlotId` already sets the non-generic
precedent.

### 2. ID scope: mbv-core only, at API and session boundaries

Introduce three newtypes:
- `ItemId` — Emby item identifier (movies, episodes, albums, tracks)
- `MediaSourceId` — specific media source within an item
- `EmbySessionId` — the Emby playback session

Primary migration site: `SessionReporter.ids: Arc<Mutex<(String, String, String)>>`
becomes `Arc<Mutex<(ItemId, MediaSourceId, EmbySessionId)>>`. The comment
explaining which String is which becomes the type signature. `mark_played_id` and
`series_id` on `PlaybackRun` become `Option<ItemId>` and `ItemId` respectively.

API boundary functions (`api_client_sessions.rs`, `api_client_reporting.rs`) take
the newtypes and call `.as_str()` at the HTTP call site.

### 3. Eliminate `is_queue_mode` by derivation

`is_queue_mode: Arc<AtomicBool>` on `PlaybackRun` and `RuntimeController` is
always set to `origin == PlaybackOrigin::Queue`. Remove it entirely. Each use site
(~13 refs) reads `origin` directly from whichever struct holds it.

The `Arc<AtomicBool>` was shared between `PlaybackRun` and `RuntimeController` so
the controller could read it without locking the run. After removal, the controller
reads its own `origin` field (it already has one — it's the source that writes the
AtomicBool at `player_runtime_controller.rs:341,499`). `PlaybackRun` reads
`self.origin`. The `set_origin` helper at `player_run_queue.rs:22` that syncs the
two becomes a plain field assignment.

### 4. Power rename: strip prefix, keep filenames aligned with content

Rename strategy:
- **Identifiers**: `power_*` → drop the prefix. `render_power_queue` → `render_queue`,
  `PowerTab` → keep as `Tab` or context-specific name, `power_home_actions` →
  `home_actions`.
- **Filenames**: follow the identifier. `power_widgets.rs` → `widgets.rs`,
  `power_home_actions.rs` → `home_actions.rs`, `input_lib_power_keys.rs` →
  `input_lib_keys.rs`, `power_cw_library_tab_actions.rs` →
  `cw_library_tab_actions.rs`.
- **Test files**: rename to match their non-test sibling.

Guard against `powerline` corruption: the rename must use word-boundary-aware
replacement (not blind find-replace). `powerline` appears in
`render/indicators.rs` (4 refs) and `render_cadence.rs` (1 ref) and `config.rs`
(1 ref) — all are the Nerd Font separator concept.

Separate commit from the ID/origin work so the mechanical rename doesn't bury
semantic changes in review.

## Risks / Trade-offs

- **Blast radius of power rename** (~55 non-test source files, 11 filename renames)
  → Mitigated by doing it in a dedicated commit with no semantic changes. Compiler
  catches any missed rename. Rebase conflicts are possible if other branches touch
  the same files.
- **ID newtype migration touches ~123 sites** → The compiler enforces completeness;
  any missed site is a build error, not a runtime bug. Migrate in one pass per
  newtype to keep the diff reviewable.
- **`origin` threading after `is_queue_mode` removal** → Low risk. The controller
  already owns the origin; the run already has it as a field. The AtomicBool was
  a workaround for sharing, not a design choice.
