//! The `List` component (design.md "Component catalogue"): shared row-styling
//! helpers for the renderers that still paint their own rows. `LibraryListRenderCtx` is the shell-built
//! browser input the wide TV/Music render contexts embed; the canonical
//! media-list painters (`render/components/media_list/{row,wide}.rs`) own
//! browser row painting, and `render_right_scrollbar` (`widgets.rs`) is the
//! shared `Scrollbar`.

/// Owned browser-list inputs shared by narrow and wide renderers. The shell
/// builds this once from the active source; owners read their own search
/// session and the canonical media-list painters drive row painting.
#[derive(Clone)]
pub(in crate::app) struct LibraryListRenderCtx {
    pub(in crate::app) items: Vec<mbv_core::api::EmbyItem>,
    pub(in crate::app::render) cursor: usize,
    pub(in crate::app) total_count: usize,
    pub(in crate::app) library_total: Option<usize>,
    pub(in crate::app) letter_filter: Option<super::super::LetterFilter>,
    /// The browse level's outstanding-load flag, projected for the Emby
    /// owner's push (its loading pill); never consumed by a painter.
    pub(in crate::app) loading: bool,
    /// Session-only Wide hero list-pane width override (`None` = default
    /// ratio). Carried here so the wide TV/Music render contexts that embed
    /// this struct forward it into the shared split; normalized against the
    /// active content-area width by the arrangement, never stored clamped.
    pub(in crate::app) list_pane_width: Option<u16>,
}

impl LibraryListRenderCtx {
    pub(in crate::app) fn from_items(items: Vec<mbv_core::api::EmbyItem>, cursor: usize) -> Self {
        let total_count = items.len();
        Self {
            items,
            cursor,
            total_count,
            library_total: None,
            letter_filter: None,
            loading: false,
            list_pane_width: None,
        }
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        self.cursor
    }

    pub(in crate::app) fn selected_item(&self) -> Option<&mbv_core::api::EmbyItem> {
        self.items.get(self.cursor)
    }

    pub(in crate::app) fn true_total(&self) -> usize {
        self.library_total.unwrap_or(self.total_count)
    }

    pub(in crate::app) fn has_letter_filter(&self) -> bool {
        self.letter_filter.is_some()
    }
}
