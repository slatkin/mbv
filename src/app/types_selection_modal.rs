#[derive(Clone, PartialEq, Eq)]
pub(crate) enum SelectionModalSource {
    Series { series_id: String },
    Album { album_id: String },
    Podcast { library_item_id: String },
    Book { book_id: String },
}

/// One row in the modal's list. `Header` is a non-selectable divider (the
/// Series modal's season name); every other surface's modal has no headers
/// and uses only `Item` rows. Cursor movement must skip `Header` rows.
#[derive(Clone)]
pub(crate) enum SelectionModalRow {
    Header(String),
    Item(SelectionModalItem),
}

impl SelectionModalRow {
    pub(crate) fn item_id(&self) -> Option<&str> {
        match self {
            Self::Item(item) => Some(&item.id),
            Self::Header(_) => None,
        }
    }
}

#[derive(Clone)]
pub(crate) struct SelectionModalItem {
    pub(crate) name: String,
    pub(crate) meta: String,
    /// Stable provider identity, kept separate from the visible name and
    /// position in the rendered list.
    pub(crate) id: String,
}

#[derive(Clone)]
pub(crate) enum SelectionModalListState {
    Loading,
    Empty,
    Ready(Vec<SelectionModalRow>),
}

impl SelectionModalListState {
    pub(crate) fn ready(rows: Vec<SelectionModalRow>) -> Self {
        if rows
            .iter()
            .any(|row| matches!(row, SelectionModalRow::Item(_)))
        {
            Self::Ready(rows)
        } else {
            Self::Empty
        }
    }

    pub(crate) fn normalize(self) -> Self {
        match self {
            Self::Ready(rows) => Self::ready(rows),
            state => state,
        }
    }

    pub(crate) fn rows(&self) -> &[SelectionModalRow] {
        match self {
            Self::Ready(rows) => rows,
            Self::Loading | Self::Empty => &[],
        }
    }

    pub(crate) fn status(&self) -> Option<&str> {
        match self {
            Self::Loading => Some("Loading…"),
            Self::Empty => Some("No items available"),
            Self::Ready(_) => None,
        }
    }
}

/// Pill filter shown at the top of the modal: played/unplayed for Podcast,
/// season number for Series.
#[derive(Clone)]
pub(crate) struct SelectionModalFilter {
    pub(crate) labels: Vec<String>,
    pub(crate) selected: usize,
}

#[derive(Clone)]
pub(crate) struct SelectionModal {
    pub(crate) source: SelectionModalSource,
    pub(crate) title: String,
    pub(crate) state: SelectionModalListState,
    /// Index into the Ready rows; always points at an Item, never a Header.
    pub(crate) cursor: usize,
    pub(crate) filter: Option<SelectionModalFilter>,
}

#[cfg(test)]
mod tests {
    use super::{
        SelectionModal, SelectionModalItem, SelectionModalListState, SelectionModalRow,
        SelectionModalSource,
    };

    #[test]
    fn modal_retains_typed_source_and_explicit_list_state() {
        let source = SelectionModalSource::Series {
            series_id: "series-1".into(),
        };
        let item = SelectionModalItem {
            name: "1. Pilot".into(),
            meta: "42m".into(),
            id: "episode-provider-id".into(),
        };
        let modal = SelectionModal {
            source,
            title: "Series".into(),
            state: SelectionModalListState::Ready(vec![SelectionModalRow::Item(item)]),
            cursor: 0,
            filter: None,
        };

        assert!(matches!(
            modal.source,
            SelectionModalSource::Series { ref series_id } if series_id == "series-1"
        ));
        assert!(matches!(modal.state, SelectionModalListState::Ready(_)));
    }

    #[test]
    fn list_state_distinguishes_statuses_and_ready_headers_from_items() {
        assert!(matches!(
            SelectionModalListState::Loading,
            SelectionModalListState::Loading
        ));
        assert!(matches!(
            SelectionModalListState::Empty,
            SelectionModalListState::Empty
        ));
        let rows = vec![
            SelectionModalRow::Header("Season 1".into()),
            SelectionModalRow::Item(SelectionModalItem {
                name: "1. Pilot".into(),
                meta: "42m".into(),
                id: "episode-provider-id".into(),
            }),
        ];
        let state = SelectionModalListState::Ready(rows);
        let SelectionModalListState::Ready(rows) = state else {
            panic!("expected ready state");
        };
        assert!(matches!(rows[0], SelectionModalRow::Header(_)));
        assert!(matches!(rows[1], SelectionModalRow::Item(_)));
        let SelectionModalRow::Item(item) = &rows[1] else {
            panic!("expected item row");
        };
        assert_eq!(item.id, "episode-provider-id");
        assert_ne!(item.id, item.name);
        assert_ne!(item.id, "1");

        let same_text = SelectionModalListState::Ready(vec![
            SelectionModalRow::Item(SelectionModalItem {
                name: "Same title".into(),
                meta: String::new(),
                id: "episode-a".into(),
            }),
            SelectionModalRow::Item(SelectionModalItem {
                name: "Same title".into(),
                meta: String::new(),
                id: "episode-b".into(),
            }),
        ]);
        let SelectionModalListState::Ready(rows) = same_text else {
            unreachable!();
        };
        assert_ne!(rows[0].item_id(), rows[1].item_id());
    }

    #[test]
    fn ready_normalizes_header_only_content_to_empty() {
        let state =
            SelectionModalListState::ready(vec![SelectionModalRow::Header("Season 1".into())]);
        assert!(matches!(state, SelectionModalListState::Empty));
    }
}
