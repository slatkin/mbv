## Context

See `proposal.md` — Why. `prepare_mpv_config_dir` currently writes only mbv's IPC option when `use_mpv_config` is false and copies the sanitized user `mpv.conf` when it is true. `init_mpv` then applies output and cache properties after mpv initialization: headless runs get 10M/10M and video-window runs get 50M/100M. Applying both cache properties after initialization deliberately prevents a user `mpv.conf` from changing mbv's playback-class cache policy.

The two cache classes come from separate measured constraints. The fixed headless budget mitigates #656 on an audio-only Player owner, while the larger video budget prevents repeated buffering of high-bitrate network feeds. This change must make only the video class configurable and preserve both defaults.

## Goals / Non-Goals

**Goals:**
- Give isolated video-window playback a portable hardware-decoding default with software fallback.
- Keep full user mpv configuration authoritative for hardware-decoding selection when enabled.
- Carry validated video cache budgets from mbv configuration to every local Player owner.
- Preserve the fixed headless audio policy and current video defaults.

**Non-Goals:**
- Select or detect a vendor-specific decoder.
- Parse or selectively inherit lines, profiles, includes, scripts, or companion files from the user's mpv configuration.
- Make headless cache sizing configurable or enable video playback on an intended audio-only headless owner.
- Add numeric cache editing to the Settings sidebar; the existing TOML configuration is the narrow user surface for these advanced values.
- Claim that cache sizing alone resolves #656's long-run resident-memory growth.
- Change source selection, queue behavior, or mpv's cache algorithm.

## Decisions

### Use `auto-safe` only for the isolated video path

After mpv initialization, `init_mpv` sets `hwdec` to `auto-safe` only when the run is non-headless and `use_mpv_config` is false. This asks mpv to choose among its safe hardware backends and retains mpv's software fallback without embedding platform or GPU detection in mbv.

When `use_mpv_config` is true, mbv does not set `hwdec`; the sanitized user configuration and mpv defaults remain authoritative. Headless runs do not set it because their intended workload is audio-only.

Alternative rejected: copy selected `hwdec`, `vo`, or `gpu-*` lines into the isolated config. That would require mbv to implement a partial mpv configuration language and would behave unpredictably around profiles and includes.

Alternative rejected: expose an mbv `hwdec` setting. The desired isolated default is already known, while users wanting a different mpv policy have the existing full-config switch.

### Configure video budgets as positive integer MiB values

Add `video_cache_forward_mb` and `video_cache_back_mb` under `[mpv]`, represented internally as positive integer MiB values. Their defaults are 50 and 100 respectively. Omitted, non-integer, zero, negative, or out-of-range values resolve independently to their defaults, matching the configuration parser's existing fallback style.

Integer MiB values keep the user contract small and map directly to the existing mpv `M` values. They avoid exposing mpv's free-form byte-size grammar or accepting arbitrary option text. The resolved values are persisted by the existing configuration save path and shown in the distributed example configuration.

Alternative rejected: one total-cache setting. mpv separately controls forward and retained/back budgets, and collapsing them would invent an allocation rule with no evidence behind it.

Alternative rejected: generic `demuxer-max-*` string passthrough. It leaks mpv option names into mbv's contract and adds validation ambiguity without providing a capability needed by these issues.

### Keep cache precedence owned by mbv

Both cache properties continue to be set after mpv initialization. A video-window run uses the resolved mbv settings regardless of `use_mpv_config`; a headless run always uses 10M/10M. Thus full user mpv configuration controls hardware-decoding choice but does not bypass the playback-class cache safety policy.

This is intentionally asymmetric: `use_mpv_config` grants control of ordinary mpv behavior, while mbv's cache settings are explicit first-class policy with known defaults and headless safeguards.

Alternative rejected: let `mpv.conf` override cache values when enabled. That would make two configuration surfaces authoritative for the same setting and could silently undo #656's fixed headless budget.

### Project resolved values through existing Player run configuration

Store the two values on the long-lived `Player` beside `use_mpv_config`, then copy them into each `MpvRunConfig`. This reaches ordinary startup, lazy runs, and pre-warm through the existing construction flow without global state or a second configuration read. Headless initialization receives the values but ignores them by policy.

## Risks / Trade-offs

- `auto-safe` can still fail to activate on unsupported hardware or content → mpv's software fallback remains enabled, and initialization tests verify the configured policy rather than requiring GPU hardware.
- A user can choose a video cache too small for a high-bitrate feed → retain 50/100 defaults and describe the settings as an explicit network/memory trade-off.
- Very large configured integers could be rejected by mpv → constrain parsing to positive in-range integer MiB values and log property-application failures rather than silently claiming the value applied.
- Adding constructor parameters can make existing test setup noisier → keep the values as plain fields and constants; do not introduce a new configuration abstraction solely for two numbers.

## Migration Plan

No data migration is required. Existing files omit the new keys and therefore retain 50M/100M video behavior. Deployment is the normal binary/config-example release path. Rollback removes the two optional keys and restores the hardcoded video values; existing files containing the keys remain parseable as unknown settings by an older binary.
