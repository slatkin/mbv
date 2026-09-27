use mbv_ui_model::media_list::SelectionOrigin;

/// Read-only presentation projection of a `MediaList` selection. Membership is
/// deliberately private to the owner and cannot be reconstructed here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectionSummary {
    pub count: usize,
    pub origin: SelectionOrigin,
}
