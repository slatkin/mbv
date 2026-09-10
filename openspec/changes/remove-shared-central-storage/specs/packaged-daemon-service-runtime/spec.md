## REMOVED Requirements

### Requirement: Shared data remains an independent optional facility

**Reason**: Packaged `mbvd` no longer hosts or exposes a shared-data facility, so there is no optional facility whose enablement, identity, authentication, storage, or fallback behavior could be altered or preserved. The guarantee it stated is now vacuous.

**Migration**: Nothing replaces it. Packaged-daemon startup, ctrl, queue control, and playback are unaffected by shared data because no shared-data code path exists. Emby-independent daemon startup is now unconditional rather than a scenario to defend.
