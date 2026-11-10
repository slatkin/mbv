//! The Library section's Search (add-gui-library-panel design D12, row 12.1):
//! the query the top bar's field is answering, whether its fetch is still out
//! or failed, and the rows it found.
//!
//! Search is cross-destination — its rows name items in any destination and
//! the activate command opens the one the row names — so it rides the
//! `GuiLibraryFrame` itself, beside the revision-gated content and never
//! inside it: a result that lands between two content changes still reaches
//! the window, and a closed field rides as the section's `null`.

use mbv::{GuiSearchProjection, GuiSearchRow};
use serde::Serialize;

/// The Search facts one frame carries (design D12, row 12.1). `None` on the
/// frame while the store holds no query and no row: no field text is up, or
/// the last query closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiSearchFrame {
    /// The query the section answers, as the last `searchQuery` command
    /// carried it.
    pub query: String,
    /// Whether the query's fetch has not landed yet.
    pub loading: bool,
    /// Whether the query's fetch failed.
    pub failed: bool,
    /// The found rows in the provider's order.
    pub results: Vec<GuiSearchRowFrame>,
}

/// One found row (design D12, row 12.1). The identity is the item's stable
/// Service id and type an activate command sends back, never a row position,
/// so a command built from a stale frame addresses the row that frame painted
/// or nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiSearchRowFrame {
    /// The Emby item id the activate command names.
    pub id: String,
    /// The Emby item type: the second half of the activate identity, and the
    /// fact that names the destination the activation opens.
    pub item_type: String,
    /// The row's primary label.
    pub title: String,
    /// The row's secondary label, empty when the item has none: the window
    /// paints a plain string, so the core's absent subtitle rides as `""`.
    pub subtitle: String,
}

impl GuiSearchFrame {
    /// The Search section `projection` describes, or `None` while the store
    /// holds no query and no row (design D12, row 12.1): a closed field must
    /// ride as the section's `null`, not as an empty one.
    pub(crate) fn carried(projection: &GuiSearchProjection) -> Option<Self> {
        (!projection.query.is_empty() || !projection.rows.is_empty())
            .then(|| Self::from_projection(projection))
    }

    /// The section for `projection`.
    pub(crate) fn from_projection(projection: &GuiSearchProjection) -> Self {
        Self {
            query: projection.query.clone(),
            loading: projection.loading,
            failed: projection.failed,
            results: projection
                .rows
                .iter()
                .map(GuiSearchRowFrame::from)
                .collect(),
        }
    }
}

impl From<&GuiSearchRow> for GuiSearchRowFrame {
    fn from(row: &GuiSearchRow) -> Self {
        Self {
            id: row.id.clone(),
            item_type: row.item_type.clone(),
            title: row.title.clone(),
            subtitle: row.subtitle.clone().unwrap_or_default(),
        }
    }
}
