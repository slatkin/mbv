# Proposal

## Why

The workspace is three crates, one of which (`mbv`, the TUI) is ~147k lines and
recompiles as a single unit on every edit. Issue #814 ranks the split candidates
into five tiers; Tier 1 is the set of modules that are already dependency leaves,
so extracting them is mechanical and unblocks the later tiers (the provider and
runtime splits all need `mbv-net` and `mbv-ids` to exist first).

Measured on `main` at `ef10064a4`, every Tier 1 module below has **zero** inbound
`crate::` references except `ws.rs`, which has exactly one
(`crate::reconnect_backoff_sleep`). No dependency inversion is needed.

## What Changes

Six new leaf crates, in dependency order. Each is a move, not a rewrite: no
behaviour, protocol, persistence, config-format or key-binding change.

| New crate | Moved from | Lines | Deps |
|---|---|---|---|
| `mbv-ids` | `mbv-core/src/id_types.rs` | 59 | `serde` |
| `mbv-keybinds` | `mbv-core/src/keybinds/` | ~1.8k | none (pure `std`) |
| `mbv-net` | `mbv-core/src/{bounded,stream,mock_http}.rs` + `encode_path_segment`, `PATH_SEGMENT`, `native_tls_agent`, `reconnect_backoff_sleep` from `mbv-core/src/lib.rs` | ~490 | `ureq`, `percent-encoding`, `rand`, `log` |
| `mbv-ws` | `mbv-core/src/ws.rs` | 588 | `tungstenite`, `serde_json`, `log`, `mbv-net` |
| `mbv-visualizer` | `src/app/infra/visualizer_worker.rs` | 596 | `pipewire`, `log` |
| `mbv-text` | `src/app/infra/{fuzzy_match,text_safety}.rs` | ~135 | `fuzzy-matcher` |

- **BREAKING (workspace-internal only):** every moved path changes and **no
  re-export shim is left behind** (issue #814: "It only counts as done when the
  old module path is gone"). Concretely:
  - `mbv_core::{ItemId, MediaSourceId, EmbySessionId}` and
    `mbv_core::id_types::*` → `mbv_ids::*`
  - `mbv_core::keybinds::*` → `mbv_keybinds::*`
  - `mbv_core::mock_http::*` → `mbv_net::mock_http::*`
  - `mbv_core::ws::*` → `mbv_ws::*`
  - `mbv_core::native_tls_agent` → `mbv_net::native_tls_agent`
  - crate-private `mbv_core::{bounded, stream, encode_path_segment,
    reconnect_backoff_sleep}` → `pub` in `mbv_net`
  - `crate::app::infra::visualizer_worker::*` → `mbv_visualizer::*`
  - `crate::app::infra::{fuzzy_match, text_safety}::*` → `mbv_text::*`
- `mbv-core`'s `test` feature forwards to `mbv-net/test`, which gates
  `mock_http` exactly as it is gated today.
- Dependency edges the compiler now enforces: `mbv-core/config` →
  `mbv-keybinds`; `mbv-core` → `mbv-ids`, `mbv-net`, `mbv-ws`; `mbv` (TUI) →
  `mbv-visualizer`, `mbv-text`, `mbv-ids`, `mbv-keybinds`, `mbv-net`.
- Dependencies dropped outright: `mbv-core` no longer depends on
  `percent-encoding`; the `mbv` TUI crate no longer depends on `pipewire`.

Tradeoff worth naming: `mbv-text` is the one item here with negligible
compile-time payoff (~135 lines, and the TUI crate still recompiles when it
changes). It is in scope because it is in Tier 1 and it is free; it is folded
into one crate rather than the issue's separate `mbv-fuzzy` because two text
predicates do not warrant two crates, and `mbv-tui-util` is a name that invites
accretion.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
None. This is a pure code-organisation refactor with no externally observable
behaviour change, so `skip_specs: true`.

## Impact

- **New:** six `crates/mbv-*/` directories, each with `Cargo.toml` + `src/`;
  `Cargo.toml` `[workspace] members` and `default-members` grow by six.
- **`crates/mbv-core/src/lib.rs`:** loses the `id_types`, `keybinds`,
  `mock_http`, `bounded`, `stream`, `ws` module declarations, the
  `pub use id_types::{…}` root re-export, and the four helper items.
- **Call sites to rewrite** (measured, `main` @ `ef10064a4`):
  - id newtypes: 16 `use` lines across 16 files
  - `keybinds`: 61 references across 17 files
  - `ws`: 31 references across 19 files
  - `mock_http`: 32 references across 22 files
  - `bounded`: 15 references across 8 files; `stream`: 6 across 6
  - `encode_path_segment`: ~30 call sites in `mbv-core/src/{api,audiobookshelf}/`
  - `visualizer_worker`: 10 files under `src/app/`
  - `fuzzy_match` / `text_safety`: 5 files under `src/app/`
- **Visibility widening:** `bounded::run_with_hard_bound*`, `stream::SocketStream`
  (today `pub(crate)`), `fuzzy_match::word_match_score`,
  `text_safety::is_control_char`, `visualizer_worker::{StereoSampleBuffer,
  join_worker}` (today `pub(in crate::app)` / `pub(crate)`) become `pub`.
- **Docs:** `AGENTS.md` repository map gains the new crates.
- **Sequencing:** `mbv-net` must land before `mbv-ws` (the backoff helper). The
  other four are independent of each other and of `mbv-net`. Tiers 2–5 of #814
  build on `mbv-net` and `mbv-ids`, so this change goes first.
- **Conflicts:** `decompose-app-god-type` and `prune-tui-test-suite` are both
  in-flight and both touch `src/app/`. Coordinate `mbv-visualizer` and
  `mbv-text` (the two `src/app/` moves) with them, or land those two last.
- Umbrella: issue #814, Tier 1.
