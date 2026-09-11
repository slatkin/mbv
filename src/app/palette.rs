// Raw colour primitives live in `render::theme` and are private to that
// module (openspec/changes/enforce-mbv-ui-design-system step 2); this
// re-export keeps every existing `palette::<role>` call site resolving.
// Transitional: the surface table's `SelectedRow` and `ArtworkPlaceholder`
// rows replaced these value-aliased names at every production site, but
// `artwork_placeholder_tests.rs` still pins the placeholder's value by name,
// so they stay reachable until the retirement unit (task 4.2) removes them.
#[allow(unused_imports)]
pub(crate) use crate::app::render::{list_selected_row_bg, SURFACE_ARTWORK_PLACEHOLDER};
pub(crate) use crate::app::render::{
    resolve_surface_focus, ACCENT, ACCENT_ACTIVE, ACCENT_AUDIOBOOKSHELF, BORDER_UNFOCUSED,
    INDICATOR_AUDIO_FG, INDICATOR_RESOLUTION_FG, PILL_BG, PILL_FG, PILL_OVERFLOW_FG, PILL_ROW_BG,
    PILL_SELECTED_BG, PILL_SELECTED_FG, PLAYBACK_META_FG, PLAYBACK_THROBBER_FG, PLAYBACK_VALUE_FG,
    PROGRESS_TRACK, SCROLLBAR, STATUS_AVAILABLE, STATUS_ERROR, SURFACE_ACCENT_SOFT,
    SURFACE_BACKDROP, SURFACE_CHROME, SURFACE_FOCUSED, SURFACE_ITEM_FOCUSED, SURFACE_PLAYBACK,
    SURFACE_RESTING, SURFACE_SIDEBAR, SURFACE_STATUS_PILL, TEXT_ACCENT_MUTED, TEXT_DETAIL_META,
    TEXT_EMPHASIS, TEXT_FOCUS_ACCENT, TEXT_METADATA, TEXT_MUTED, TEXT_ON_ACCENT, TEXT_PRIMARY,
    TEXT_SECONDARY, TEXT_STRONG,
};
// The closed surface table (`unify-surface-colour-neutral` D1/D2/D7): a paint
// site names a `Surface` and calls the resolver instead of naming a role.
// Transitional: no production call site names these yet, so the re-export is
// unused until the migration units land; the allow keeps `cargo check`
// warning-free until then.
#[allow(unused_imports)]
pub(in crate::app) use crate::app::render::{surface_colors, Surface};
