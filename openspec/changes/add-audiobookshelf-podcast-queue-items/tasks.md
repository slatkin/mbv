## 1. Prerequisite

- [ ] 1.1 Confirm #513 is applied and Audiobookshelf episode input reaches a provider-specific inert handler without entering Emby selection or action code.

## 2. Provider-Qualified Queue Items

- [ ] 2.1 Add the source-of-truth Audiobookshelf podcast QueueItem payload with provider-native identity, presentation metadata, duration, progress, completion, and Service-scoped artwork identity, excluding credentials and ephemeral playback fields.
- [ ] 2.2 Add typed Service-qualified content/position identity for Emby, Feed, and Audiobookshelf items while preserving independent QueueSlotId occurrence identity and avoiding formatted-string matching.
- [ ] 2.3 Extend QueueItem accessors, status/metadata projection, rendering, queue mutations, and exhaustive variant matches for Audiobookshelf episodes without converting them to Emby or Feed data.

## 3. Persistence And Service Ownership

- [ ] 3.1 Extend tagged queue persistence and restoration for Audiobookshelf items, preserve legacy untagged Emby state reading, and exclude setup, credentials, playback session IDs, resolved URLs, and headers.
- [ ] 3.2 Preserve Composed and persisted Audiobookshelf snapshots after explicit credential rejection while making them ineligible for Bound queues until repair and later playback enablement.
- [ ] 3.3 Purge Audiobookshelf items from Composed, Bound, and persisted state on confirmed Service replacement/removal without changing Emby or Feed items.

## 4. Owner Admission

- [ ] 4.1 Add a semantic in-process-owner query derived from the current player endpoint, distinct from same-machine and launch-mode predicates.
- [ ] 4.2 Extend canonical owner admission to evaluate required Service capability as well as media kind for explicit submission, Composed-to-Bound binding, restored queues, and cold startup.
- [ ] 4.3 Keep every owner ineligible for Audiobookshelf items in this change, visibly reject explicit submission without fall-through, and preserve other playable items during mixed binding.
- [ ] 4.4 Remove the cold queue-start dependency on an Emby snapshot for Feed and future Audiobookshelf QueueItems without adding Audiobookshelf source resolution.
- [ ] 4.5 Keep Audiobookshelf items out of ctrl submissions/state and advertise no Audiobookshelf transport capability.

## 5. Verification

- [ ] 5.1 Verify typed identity, duplicates, mixed edits, metadata, persistence, restore admission, rejection/repair preservation, replacement/removal purge, cold Feed startup, unsupported-owner behavior, and ctrl absence at the closest existing boundaries.
- [ ] 5.2 Run focused queue/App nextest suites, `cargo check -p mbv-core`, `cargo check -p mbv`, formatting, clippy, `make check-code-file-lines`, strict OpenSpec validation, and diff checks.
