// Raw colour primitives live in `render::theme` and are private to that
// module (openspec/changes/enforce-mbv-ui-design-system step 2); this
// re-export keeps every existing `palette::<role>` call site resolving.
pub(crate) use crate::app::render::{
    bar_role_fg, ACCENT, ACCENT_ACTIVE, ACCENT_AUDIOBOOKSHELF, DURATION, GROUP_HEADING_FG,
    HERO_CREDITS_NAME, HERO_CREDITS_STRIPE, HERO_META_ROLES, HERO_OVERVIEW_SEPARATOR,
    HINT_PILL_FILLS, INDICATOR_AUDIO_FG, INDICATOR_RESOLUTION_FG, PILL_OVERFLOW_FG,
    PILL_SELECTED_FG, PLAYBACK_CONTEXT_FG, PLAYBACK_META_FG, PLAYBACK_TITLE_FG, PLAYBACK_VALUE_FG,
    PLAYLIST_LOADED_FG, PLAYLIST_STRIPE_BG, PROGRESS_PERCENT, PROGRESS_TRACK, SCROLLBAR,
    SELECTED_ROW_BG, SELECTED_ROW_FG, SELECTED_ROW_PROGRESS_FG, SESSIONS_STRIPE_BG,
    SETTINGS_STRIPE_BG, SIDEBAR_SCROLLBAR, SPLIT_ROW_CONTEXT_FG, SPLIT_ROW_TITLE_FG,
    STATUS_AVAILABLE, STATUS_ERROR, SURFACE_RESTING, TEXT_ACCENT_MUTED, TEXT_EMPHASIS,
    TEXT_FOCUS_ACCENT, TEXT_METADATA, TEXT_MUTED, TEXT_ON_ACCENT, TEXT_PRIMARY, TEXT_SECONDARY,
    TEXT_STRONG, WORKSPACE_HEADER_FG,
};
#[cfg(test)]
pub(crate) use crate::app::render::{
    PILL_SELECTED_BG, SURFACE_BACKDROP, SURFACE_CHROME, SURFACE_FOCUSED,
};

// Hero-title role, kept off the main list above to mark it as a late addition
// (wide + narrow hero header title).
pub(crate) use crate::app::render::TEXT_HERO_TITLE;
// The closed surface table (`unify-surface-colour-neutral` D1/D2/D7): a paint
// site names a `Surface` and calls the resolver instead of naming a role.
pub(in crate::app) use crate::app::render::{surface_colors, Surface};
