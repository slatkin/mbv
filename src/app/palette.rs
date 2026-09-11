// Raw colour primitives live in `render::theme` and are private to that
// module (openspec/changes/enforce-mbv-ui-design-system step 2); this
// re-export keeps every existing `palette::<role>` call site resolving.
pub(crate) use crate::app::render::{
    ACCENT, ACCENT_ACTIVE, ACCENT_AUDIOBOOKSHELF, BORDER_UNFOCUSED, INDICATOR_AUDIO_FG,
    INDICATOR_RESOLUTION_FG, PILL_FG, PILL_OVERFLOW_FG, PILL_SELECTED_FG, PLAYBACK_META_FG,
    PLAYBACK_THROBBER_FG, PLAYBACK_VALUE_FG, PROGRESS_TRACK, SCROLLBAR, STATUS_AVAILABLE,
    STATUS_ERROR, TEXT_ACCENT_MUTED, TEXT_DETAIL_META, TEXT_EMPHASIS, TEXT_FOCUS_ACCENT,
    TEXT_METADATA, TEXT_MUTED, TEXT_ON_ACCENT, TEXT_PRIMARY, TEXT_SECONDARY, TEXT_STRONG,
};
// Task 4.2: the retired role names and the value-aliased resolver survive only
// as test-fed re-exports — each is pinned by a frozen pre-existing test file
// the neutrality rule forbids editing, so a minimal named re-export stays for
// exactly those names (see the change report for the per-name reasons).
#[cfg(test)]
pub(crate) use crate::app::render::{
    resolve_surface_focus, PILL_BG, PILL_ROW_BG, PILL_SELECTED_BG, SURFACE_ACCENT_SOFT,
    SURFACE_ARTWORK_PLACEHOLDER, SURFACE_BACKDROP, SURFACE_CHROME, SURFACE_FOCUSED,
    SURFACE_PLAYBACK, SURFACE_RESTING,
};
// The closed surface table (`unify-surface-colour-neutral` D1/D2/D7): a paint
// site names a `Surface` and calls the resolver instead of naming a role.
pub(in crate::app) use crate::app::render::{surface_colors, Surface};
