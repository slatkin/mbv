# Project: mbv

Terminal media client in Rust with singleton Emby, Audiobookshelf, and Feeds
Services. Embeds mpv; playback is owned by the terminal process, a user-owned
Local daemon, or the separately packaged mbvd.

Paths move often. Verify before trusting one. Where this file and the code
disagree, the code wins — fix this file in the same PR.

## Read first
- `CONTEXT.md` — domain vocabulary. Its *Avoid* lists are wrong terms, not style.
- `docs/adr/` — decisions already made; check before changing architecture.
  Superseded ADRs carry a clear banner. No banner means current.

## Shape
- `src/` — interactive binary. Process-role selection, Service-independent TUI
  startup, tray, mpris, single-instance handling, and Local-daemon supervision.
- `src/local_daemon.rs` — user-owned Local-daemon bootstrap and Control
  credential. It starts without authenticating a Remote Service (ADR 0018).
- `src/app/` — TUI state. Prefixes are the index: `input_*` keys and mouse,
  `*_actions.rs` state transitions, `render/` drawing, `*_tests.rs` its sibling.
  Service setup/runtime orchestration and provider-specific Emby,
  Audiobookshelf, and Feeds browse state also live here. Selected browse
  destinations dispatch exhaustively via Tab selection (Home, EmbyLibrary,
  AudiobookshelfLibrary, Feeds) — count-aware position mapping prevents Emby and
  Audiobookshelf sharing a numeric position. Provider browse models remain
  separate and meet only at QueueItem construction and owner admission, not
  through a generic catalog model. Feed fetching is client-side.
- `crates/mbv-core/` — Service setup/runtime types, Emby and Audiobookshelf API
  boundaries, feed subscription config, ctrl/shared-data protocols, daemon
  (multi-connection, ADR 0014), Player, canonical Emby/Feed/Audiobookshelf
  queue, source preparation, and mpv projection. No UI or feed fetching.
  Audiobookshelf bare-mode queue, source resolution (direct/HLS), active-file
  projection, and progress sync/finalization are now active (milestone #515,
  PRs #520-522). Local daemon and mbvd Audiobookshelf admission and ctrl
  transport are milestone #524 (issues #525-528: transport, setup reconciliation,
  daemon-owner playback, stay-alive continuity).
- `crates/mbvd/` — separately packaged daemon with system configuration, state
  (`redb` `shared.mbvd`), and sockets. On `main` it is still Emby-gated: it
  constructs `EmbyClient` unconditionally, requires cached credentials to start,
  and uses legacy Emby-token ctrl authentication. Feed playback and owner-local
  queue/control without Emby, Service-independent startup (optional Emby
  runtime), filesystem/trusted-LAN ctrl auth, and `mbvd --connect emby` admin
  are implemented in open PR #529 tracking issue #523 — do not describe them
  as landed on `main`.

Source of truth, all in `mbv-core/src/`:
- `ctrl.rs` — ctrl protocol and capabilities (v7, additive via strings).
- `shared_protocol.rs` / `shared_state.rs` / `shared_store.rs` — shared-data
  protocol (v1), roaming documents (Queue, Library position, Last remote
  connection, Roaming settings), and keyed Feed entry state table
  `(user_id, feed_id, entry_guid)` with last-write-wins + prefix scan.
- `api_types.rs` — Emby wire types.
- `config_types_paths.rs` — Service setup (`EmbySetup`, `AudiobookshelfSetup`),
  ServiceKind, Config, and path types; `config_types_feed.rs` — feed subscriptions.
- `service_runtime.rs` — ServiceState (NotConfigured/Connecting/Ready/
  NeedsAuthentication/Unavailable) and SetupGeneration monotonic guard.
- `audiobookshelf.rs`, `audiobookshelf_catalog.rs`, and
  `audiobookshelf_playback.rs` — Audiobookshelf API contracts, paged podcast
  show listing, and playback session lifecycle (direct/HLS, Bearer isolation,
  bounded HLS readiness, finalization).
- `playback_queue_items.rs` — QueueItem enum (Emby/Feed/Audiobookshelf), typed
  QueueItemContentId, and owner admission rules (media kind, Service
  eligibility, audiobookshelf capability); `playback_queue.rs` — canonical
  queue slots, stable slot identity, revision, refresh/consume/protection.
- `player_sources.rs` — owner-local just-in-time source/lifecycle preparation;
  `player_projection.rs` — eager (full queue mirrored) and active-file (only
  active slot materialized) mpv projection.

Change source-of-truth types before their callers.

## Commands
Prefer cargo nextest over cargo test.

Additional cargo tools are available and should be preferred: cargo watch, cargo expand,

- `cargo edit` - manage cargo dependencies
- `cargo expand` - show the result of macro expansion
- `cargo watch` - compiles projects when sources change

Tests:
- `cargo check -p <affected-package>` — prefer the narrowest affected package
- `cargo nextest run -p <affected-package>`
- `cargo clippy --workspace --all-targets`
- `make check-code-file-lines`

Use difftastic (difft) is also available and can be used with git.

You run in an environment where ast-grep is available; whenever a search requires syntax-aware or structural matching, default to ast-grep --lang rust -p '<pattern>' (or set --lang appropriately) and avoid falling back to text-only tools like rg or grep unless I explicitly request a plain-text search.

Prefix every command with `rtk`. It filters when it has a filter and passes
through unchanged when it doesn't, so it is always safe. Prefix each command in
a chain, not just the first. `rtk grep` with a format flag (`-c`, `-l`, `-L`,
`-o`, `-Z`) runs raw; `rtk proxy <cmd>` bypasses filtering.

## Constraints
- 800-line cap per source file, pre-commit enforced. Over the line means split
  it in the same PR.
- Ctrl protocol: additive changes get a capability string, not a version bump.
  Rule sits above `CTRL_PROTOCOL_VERSION` in `mbv-core/src/ctrl.rs`.
- Flaky test: delete it, write a new one. Don't troubleshoot.
- Symbol-specific rules go above the symbol, not here. Rules at the edit site
  can't rot unnoticed.

## Planning
- Design work: `openspec/changes/<name>/`. The one local exception to
  GitHub-first.
- Issues and discussion: GitHub Issues (slatkin/mbv), via `gh`. Ad-hoc notes:
  gists, not loose markdown.
- Specs, plans and docs commit with their code. Applied OpenSpec deltas merge
  into `openspec/specs/`, then the change is archived; completed changes must
  not remain under active changes as current intent.

## Workflow
- Gather → plan → execute. Past ~3 files, plan first and execute in fresh context.
- Delegate context-heavy exploration to a subagent; ingest only the summary.
- Search `src/ crates/ docs/ openspec/`. `.worktrees/` and `.opencode/` hold
  duplicates and give false hits.
- After a PR from a worktree, switch back to main.

## Code Exploration and Editing Policy

Use JCodeMunch for code discovery, retrieval, and impact analysis. Do not use
native Read, Grep, Glob, or Bash to explore code. Use JCodeMunch to find and
understand code before deciding to edit it.

Use Serena for code edits, not routine reads:
- Once per coding session, call `serena_initial_instructions` before using
  Serena. The OpenCode MCP server starts Serena in `ide` mode for the current
  repository.
- Replacing an entire function, method, impl, type, or class →
  `serena_replace_symbol_body`.
- Adding a declaration adjacent to an existing symbol →
  `serena_insert_before_symbol` or `serena_insert_after_symbol`.
- Renaming or deleting a code symbol → `serena_rename_symbol` or
  `serena_safe_delete_symbol`; these are reference-aware.
- Changing only a few lines inside a larger symbol → targeted
  `serena_replace_content` (use a precise literal or regex and let ambiguity
  fail safely), rather than a broad text patch.
- Use normal edit tools for non-code files (including `AGENTS.md`, docs,
  YAML/TOML/JSON, lockfiles) or only when Serena cannot resolve the code
  target. This is the fallback, not the default for Rust code.

After Serena changes code, call `register_edit` for the changed paths unless a
hook has already reindexed them; this keeps JCodeMunch's retrieval index fresh.

**Start any session:**
1. `resolve_repo { "path": "." }` — confirm the project is indexed. If not: `index_folder { "path": "." }`
2. `suggest_queries` — when the repo is unfamiliar

**Finding code:**
- symbol by name → `search_symbols` (add `kind=`, `language=`, `file_pattern=`, `decorator=` to narrow)
- decorator-aware queries → `search_symbols(decorator="X")` to find symbols with a specific decorator (e.g. `@property`, `@route`); combine with set-difference to find symbols *lacking* a decorator (e.g. "which endpoints lack CSRF protection?")
- string, comment, config value → `search_text` (supports regex, `context_lines`)
- database columns (dbt/SQLMesh) → `search_columns`

**Reading code:**
- before opening any file → `get_file_outline` first
- one or more symbols → `get_symbol_source` (single ID → flat object; array → batch)
- symbol + its imports → `get_context_bundle`
- specific line range only → `get_file_content` (last resort)

**Repo structure:**
- `get_repo_outline` → dirs, languages, symbol counts
- `get_file_tree` → file layout, filter with `path_prefix`

**Relationships & impact:**
- what imports this file → `find_importers`
- where is this name used → `find_references`
- is this identifier used anywhere → `check_references`
- file dependency graph → `get_dependency_graph`
- what breaks if I change X → `get_blast_radius`
- what symbols actually changed since last commit → `get_changed_symbols`
- find unreachable/dead code → `find_dead_code`
- class hierarchy → `get_class_hierarchy`

## Session-Aware Routing

**Opening move for any task:**
1. `plan_turn { "repo": "...", "query": "your task description", "model": "<your-model-id>" }` — get confidence + recommended files; the `model` parameter narrows the exposed tool list to match your capabilities at zero extra requests.
2. Obey the confidence level:
   - `high` → go directly to recommended symbols, max 2 supplementary reads
   - `medium` → explore recommended files, max 5 supplementary reads
   - `low` → the feature likely doesn't exist. Report the gap to the user. Do NOT search further hoping to find it.
3. **One-call shortcut for a concrete task** — `assemble_task_context { "repo": "...", "task": "..." }` returns a single token-budgeted, source-attributed context capsule. It auto-classifies the task (explore / debug / refactor / extend / audit / review), auto-extracts anchor symbols, and runs the intent-appropriate sequence of the tools below end-to-end — so you get the whole context in one request instead of chaining the primitives by hand. Prefer it over a manual chain when the task is well-defined; fall back to step 1's routing when you need to decide *whether* the feature exists first.

**Interpreting search results:**
- If `search_symbols` returns `negative_evidence` with `verdict: "no_implementation_found"`:
  - Do NOT re-search with different terms hoping to find it
  - Do NOT assume a related file (e.g. auth middleware) implements the missing feature (e.g. CSRF)
  - DO report: "No existing implementation found for X. This would need to be created."
  - DO check `related_existing` files — they show what's nearby, not what exists
- If `verdict: "low_confidence_matches"`: examine the matches critically before assuming they implement the feature

**After editing files:**
- If PostToolUse hooks are installed (Claude Code only), edited files are auto-reindexed
- Otherwise, call `register_edit` with edited file paths to invalidate caches and keep the index fresh
- For bulk edits (5+ files), always use `register_edit` with all paths to batch-invalidate

**Token efficiency:**
- If `_meta` contains `budget_warning`: stop exploring and work with what you have
- If `auto_compacted: true` appears: results were automatically compressed due to turn budget
- Use `get_session_context` to check what you've already read — avoid re-reading the same files

## Model-Driven Tool Tiering

Your jcodemunch-mcp server narrows the exposed tool list based on the model you are running as. To avoid wasting requests on primitives when a composite would do, always include `model="<your-model-id>"` in your opening `plan_turn` call.

Replace `<your-model-id>` with your active model:
- Claude Opus variants → `claude-opus-4-7` (or any `claude-opus-*`)
- Claude Sonnet variants → `claude-sonnet-4-6`
- Claude Haiku variants → `claude-haiku-4-5`
- GPT-4o / GPT-5 / o1 / Llama → use the model id as printed by your runner

The `model=` parameter rides on the existing `plan_turn` call — it does **not** add a separate tool invocation. If `plan_turn` is not appropriate for a non-code task, call `announce_model(model="...")` once instead.
