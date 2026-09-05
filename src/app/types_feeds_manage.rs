use mbv_core::config::{FeedKind, FeedSubscription};
use std::sync::mpsc;

/// Which field of the add/edit form currently has keyboard focus.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FeedFormField {
    Name,
    Url,
    Kind,
}

/// The add/edit text-entry form (§6.1/§6.3). `editing_index` is `Some` in
/// edit mode, where the URL field is read-only: editing changes only name
/// and kind, and a URL change requires removing the subscription and
/// adding a new one (design.md decision 10).
#[derive(Clone)]
pub(super) struct FeedForm {
    pub name: String,
    pub url: String,
    pub kind: FeedKind,
    pub focus: FeedFormField,
    pub editing_index: Option<usize>,
}

impl FeedForm {
    pub(super) fn new_add() -> Self {
        Self {
            name: String::new(),
            url: String::new(),
            kind: FeedKind::Video,
            focus: FeedFormField::Name,
            editing_index: None,
        }
    }

    pub(super) fn new_edit(index: usize, sub: &FeedSubscription) -> Self {
        Self {
            name: sub.name.clone(),
            url: sub.url.clone(),
            kind: sub.kind,
            focus: FeedFormField::Name,
            editing_index: Some(index),
        }
    }
}

/// Which sub-view of the management overlay is active.
#[derive(Clone)]
pub(super) enum FeedsManageStage {
    List,
    Form(FeedForm),
}

/// Result of a background add-feed fetch+parse (§6.2), carrying the
/// submitting attempt's id so a stale/cancelled result -- the add was
/// cancelled, or superseded by a later submission before this one arrived
/// -- can be told apart from the still-current one.
pub(super) struct FeedAddResult {
    pub id: u64,
    pub name: String,
    pub url: String,
    pub kind: FeedKind,
    pub result: Result<(), String>,
}

/// Shell-side state for the feeds management overlay (§6), opened from F2
/// Settings. The component owns the interaction state (stage/cursor/form
/// edits); this holds only the background add-feed channel, the in-flight
/// add marker and the add-attempt id counter — the piece that cannot live in
/// the component (task 5.3d).
pub(super) struct FeedsManagePopup {
    /// The id of an in-flight add submission, or `None` when nothing is
    /// being fetched. Set on submit, cleared on cancel (Esc) or once its
    /// result is applied/discarded by `drain_feed_add_results`.
    pub pending_add: Option<u64>,
    pub next_add_id: u64,
    pub add_tx: mpsc::Sender<FeedAddResult>,
    pub add_rx: mpsc::Receiver<FeedAddResult>,
}

impl FeedsManagePopup {
    pub(super) fn new() -> Self {
        let (add_tx, add_rx) = mpsc::channel();
        Self {
            pending_add: None,
            next_add_id: 0,
            add_tx,
            add_rx,
        }
    }
}
