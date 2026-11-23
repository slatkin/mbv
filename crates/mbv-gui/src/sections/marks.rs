//! The drain-hook mark table (design D3, rule 2): which gated sections each
//! `ClientTickHost` hook marks when it reports work. One exhaustive `match`
//! holds the table, so the compiler forces a row for every variant. The trait
//! impl forces only a body for a new method, so the audit (design D3) guards
//! the rows the compiler cannot.

use super::{GATED_SECTIONS, Section};

/// One `ClientTickHost` hook of the GUI core. `app_mut` and `tick_input` are
/// not hooks: the input phase sets its own marks (design D3, rule 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DrainHook {
    DrainStartupWorkers,
    ReprojectAudiobookshelfContent,
    DrainSuspendedHomeEvents,
    DrainPlayerEvents,
    DrainLibEvents,
    /// The GUI Library's Client-only fetch results (design D5 of
    /// `decouple-client-core`): the stores they install are the Library
    /// section's source, so the row names the Library beside the sections a
    /// library event marks.
    DrainClientEvents,
    DrainSearchResults,
    TickSearchClock,
    HandleServiceRequest,
    TickInlineSearchClock,
    ReprojectFeedContent,
    DrainFeedAddResults,
    DrainCardImageCompletions,
    DrainResizeResponses,
    DrainTransportEvents,
    DrainWsEvents,
    DrainAudiobookshelfSocketEvents,
    DrainIdleFeed,
    RunPeriodicMaintenance,
}

/// The sections the Client-only fetch drain marks when it reports a landed
/// result (`decouple-client-core` design D5). The GUI Library's own stores are
/// the Library section's source, so the drain's row names the sections such a
/// result can change, read here by the drain's own test.
#[cfg(any(test, feature = "test"))]
#[doc(hidden)]
#[must_use]
pub fn client_event_marks() -> &'static [Section] {
    marks_for_hook(DrainHook::DrainClientEvents)
}

/// The gated sections a hook marks when it reports work. An empty slice is a
/// documented no-op: the hook changes no gated source.
pub(crate) fn marks_for_hook(hook: DrainHook) -> &'static [Section] {
    match hook {
        DrainHook::DrainStartupWorkers | DrainHook::DrainSuspendedHomeEvents => GATED_SECTIONS,
        DrainHook::ReprojectAudiobookshelfContent
        | DrainHook::DrainPlayerEvents
        | DrainHook::DrainLibEvents
        | DrainHook::DrainClientEvents
        | DrainHook::ReprojectFeedContent
        | DrainHook::DrainFeedAddResults
        | DrainHook::DrainWsEvents
        | DrainHook::DrainAudiobookshelfSocketEvents
        | DrainHook::DrainIdleFeed => &[Section::Library, Section::Settings, Section::Playlists],
        DrainHook::DrainSearchResults => &[Section::Search],
        DrainHook::DrainCardImageCompletions => &[Section::Library],
        DrainHook::TickSearchClock
        | DrainHook::HandleServiceRequest
        | DrainHook::TickInlineSearchClock
        | DrainHook::DrainResizeResponses
        | DrainHook::DrainTransportEvents
        | DrainHook::RunPeriodicMaintenance => &[],
    }
}
