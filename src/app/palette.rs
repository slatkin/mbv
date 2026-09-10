// Raw colour primitives live in `render::theme` and are private to that
// module (openspec/changes/enforce-mbv-ui-design-system step 2); this
// re-export keeps every existing `palette::<role>` call site resolving.
pub(crate) use crate::app::render::{
    ACCENT, ACCENT_ACTIVE, ACCENT_AUDIOBOOKSHELF, BORDER_UNFOCUSED, INDICATOR_AUDIO_FG,
    INDICATOR_RESOLUTION_FG, PILL_FG, PILL_OVERFLOW_FG, PILL_SELECTED_FG, PLAYBACK_META_FG,
    PLAYBACK_THROBBER_FG, PLAYBACK_VALUE_FG, PROGRESS_TRACK, SCROLLBAR, STATUS_AVAILABLE,
    STATUS_ERROR, TEXT_ACCENT_MUTED, TEXT_DETAIL_META, TEXT_EMPHASIS, TEXT_FOCUS_ACCENT,
    TEXT_METADATA, TEXT_MUTED, TEXT_ON_ACCENT, TEXT_PRIMARY, TEXT_SECONDARY, TEXT_STRONG,
    TEXT_TAB_INACTIVE,
};
// The closed surface table (`unify-surface-colour` D2/D3): a paint site names
// a `Surface` and calls the resolver instead of naming a role. Re-exported at
// `crate::app` because that is the table's own visibility.
pub(in crate::app) use crate::app::render::{
    dim_backdrop_color, surface_colors, surface_colors_for_column_focus, Surface,
};
