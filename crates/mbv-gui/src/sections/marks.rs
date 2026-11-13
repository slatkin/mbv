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

/// The gated sections a hook marks when it reports work. An empty slice is a
/// documented no-op: the hook changes no gated source.
pub(crate) fn marks_for_hook(hook: DrainHook) -> &'static [Section] {
    match hook {
        DrainHook::DrainStartupWorkers | DrainHook::DrainSuspendedHomeEvents => GATED_SECTIONS,
        DrainHook::ReprojectAudiobookshelfContent
        | DrainHook::DrainPlayerEvents
        | DrainHook::DrainLibEvents
        | DrainHook::ReprojectFeedContent
        | DrainHook::DrainFeedAddResults
        | DrainHook::DrainWsEvents
        | DrainHook::DrainAudiobookshelfSocketEvents
        | DrainHook::DrainIdleFeed => &[Section::Library, Section::Settings],
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
