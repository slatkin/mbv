//! Position-free content and identity channels for `BrowserComponent`
//! (task 3.7).
//!
//! `BrowserContent` is the sole input of `BrowserComponent::set_content`. It
//! carries everything the legacy Wide/Narrow list painters need EXCEPT the
//! cursor and scroll — those are owned by the active canonical control and are
//! re-seeded only through the separately gated `BrowserComponent::apply_position`
//! push. An ordinary content push therefore has no field in which stale shell
//! position could travel; that is the by-construction proof design D3 asks for.
//!
//! `BrowserIdentity` is the browse identity the shell compares between pushes to
//! decide whether `apply_position` runs: within one identity (pagination,
//! loading completion, ordinary refresh, the component's own cursor echo) no
//! position crosses the boundary.

use mbv_core::api::EmbyItem;

use crate::app::render::{LetterFilter, LibraryListRenderCtx};

/// Position-free browser content. Mirrors every `LibraryListRenderCtx` field
/// except `cursor`/`scroll`.
#[derive(Clone, Default)]
pub(in crate::app) struct BrowserContent {
    pub(in crate::app) items: Vec<EmbyItem>,
    pub(in crate::app) total_count: usize,
    pub(in crate::app) library_total: Option<usize>,
    pub(in crate::app) letter_filter: Option<LetterFilter>,
    pub(in crate::app) loading: bool,
    pub(in crate::app) search_query: Option<String>,
    pub(in crate::app) search_loading: bool,
    pub(in crate::app) group_pills: bool,
}

impl BrowserContent {
    /// Content with only an item list (no library total, filter, or search).
    pub(in crate::app) fn from_items(items: Vec<EmbyItem>) -> Self {
        let total_count = items.len();
        Self {
            items,
            total_count,
            ..Self::default()
        }
    }

    /// Strip the position off a shell-built `LibraryListRenderCtx`, keeping
    /// every other field. Used at the `push_emby_browser_content` seam so no
    /// shell-supplied cursor/scroll can reach the component.
    pub(in crate::app) fn from_render_ctx(ctx: LibraryListRenderCtx) -> Self {
        Self {
            items: ctx.items,
            total_count: ctx.total_count,
            library_total: ctx.library_total,
            letter_filter: ctx.letter_filter,
            loading: ctx.loading,
            search_query: ctx.search_query,
            search_loading: ctx.search_loading,
            group_pills: ctx.group_pills,
        }
    }

    pub(in crate::app) fn item_count(&self) -> usize {
        self.items.len()
    }

    pub(in crate::app) fn has_group_pills(&self) -> bool {
        self.group_pills
    }

    pub(in crate::app) fn is_search_active(&self) -> bool {
        self.search_query.is_some()
    }

    pub(in crate::app) fn true_total(&self) -> usize {
        self.library_total.unwrap_or(self.total_count)
    }

    /// THE single site that reconstitutes a `LibraryListRenderCtx` for the
    /// legacy painters, from this position-free content plus the active
    /// control's own cursor/scroll. No shell-supplied position reaches it.
    pub(in crate::app) fn with_cursor_scroll(
        self,
        cursor: usize,
        scroll: usize,
    ) -> LibraryListRenderCtx {
        let mut ctx = LibraryListRenderCtx::from_items(self.items, cursor, scroll);
        ctx.total_count = self.total_count;
        ctx.library_total = self.library_total;
        ctx.letter_filter = self.letter_filter;
        ctx.loading = self.loading;
        ctx.search_query = self.search_query;
        ctx.search_loading = self.search_loading;
        ctx.group_pills = self.group_pills;
        ctx
    }
}

/// The browse identity a shell content push carries. `apply_position` runs only
/// when this changes since the last push for a browser (drill-in, go-back,
/// letter-filter reset, sort change, feed/home-video group switch).
#[derive(Clone, Default, PartialEq, Eq)]
pub(in crate::app) struct BrowserIdentity {
    pub(in crate::app) depth: usize,
    pub(in crate::app) parent_id: String,
    pub(in crate::app) letter_filter: Option<usize>,
    pub(in crate::app) sort_by: String,
    pub(in crate::app) sort_order: String,
    pub(in crate::app) unplayed_only: bool,
    pub(in crate::app) feed_group: Option<usize>,
}
