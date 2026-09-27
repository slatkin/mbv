//! The `List` component (design.md "Component catalogue"): shared row-styling
//! helpers for the renderers that still paint their own rows. `LibraryListRenderCtx` is the shell-built
//! browser input the wide TV/Music render contexts embed; the canonical
//! media-list painters (`render/components/media_list/{row,wide}.rs`) own
//! browser row painting, and `render_right_scrollbar` (`widgets.rs`) is the
//! shared `Scrollbar`.

/// Owned browser-list inputs shared by narrow and wide renderers. The shell
/// builds this once from the active source; owners read their own search
/// session and the canonical media-list painters drive row painting.
#[derive(Clone, Debug)]
pub struct LibraryListRenderCtx {
    pub items: Vec<mbv_emby_model::EmbyItem>,
    pub cursor: usize,
    pub total_count: usize,
    pub library_total: Option<usize>,
    pub letter_filter: Option<super::super::LetterFilter>,
    /// The browse level's outstanding-load flag, projected for the Emby
    /// owner's push (its loading pill); never consumed by a painter.
    pub loading: bool,
    /// Session-only Wide hero list-pane width override (`None` = default
    /// ratio). Carried here so the wide TV/Music render contexts that embed
    /// this struct forward it into the shared split; normalized against the
    /// active content-area width by the arrangement, never stored clamped.
    pub list_pane_width: Option<u16>,
}

impl LibraryListRenderCtx {
    #[must_use]
    pub fn from_items(items: Vec<mbv_emby_model::EmbyItem>, cursor: usize) -> Self {
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

    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    #[must_use]
    pub fn selected_item(&self) -> Option<&mbv_emby_model::EmbyItem> {
        self.items.get(self.cursor)
    }

    #[must_use]
    pub fn true_total(&self) -> usize {
        self.library_total.unwrap_or(self.total_count)
    }

    #[must_use]
    pub fn has_letter_filter(&self) -> bool {
        self.letter_filter.is_some()
    }
}
