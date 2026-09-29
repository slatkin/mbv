/// What happens when the shared confirmation modal (`ConfirmModal`) is
/// accepted. Each variant carries whatever state its effect needs, so
/// the modal's trigger site can hand off the whole decision to the single
/// the shell's confirm-action dispatcher instead of a bespoke bool/option
/// field per confirmation.
#[derive(Clone, Debug, PartialEq)]
pub enum ConfirmAction {
    ClearQueue,
    RemoveActiveQueueItem(usize),
    RescanLibrary(usize),
    SaveOverwritePlaylist {
        existing_id: String,
        name: String,
    },
    DiscardOrSaveDirtyPlaylist,
    PlayLocallyInstead,
    DeletePlaylist {
        id: String,
        name: String,
    },
    RemoveFeedSubscription(usize),
    RemoveEmby,
    ReplaceEmby(mbv_core::service_runtime::SetupGeneration),
    RemoveAudiobookshelf,
    ReplaceAudiobookshelf(mbv_core::service_runtime::SetupGeneration),
    /// Design D6: a resolved queue replacement that would discard a populated
    /// target queue. The payload is the already-resolved
    /// `QueueDeferrals::take_confirmed_replacement` transition, so confirming
    /// executes exactly that action through the existing playback/admission
    /// executor and cannot re-resolve into a second one. Save ownership stays
    /// with `QueueDeferrals::bind_to_save` and
    /// `QueueDeferrals::take_on_save_complete`.
    ReplacePopulatedQueue,
}

/// How a modal button reads: the affirmative choice in the accent green, the
/// cancel/destructive choices (including `Discard`/`Back`) in the error red
/// (issue #855).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmButtonTone {
    Affirmative,
    Cancel,
}

/// One button on the shared confirmation modal's button row (issue #855).
///
/// `keys` is the key-binding hint shown inside the pill (`"y/Enter"`, `"Esc"`)
/// and is also what a mouse click resolves through: the click presses the
/// button's first key, so the pill and the keyboard share one intent mapping
/// (`confirm_intent_for_key`).
#[derive(Clone, Debug, PartialEq)]
pub struct ConfirmButton {
    pub keys: String,
    pub label: String,
    pub tone: ConfirmButtonTone,
}

impl ConfirmButton {
    /// The affirmative button (green text on the ink pill).
    #[must_use]
    pub fn affirmative(keys: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            keys: keys.into(),
            label: label.into(),
            tone: ConfirmButtonTone::Affirmative,
        }
    }

    /// A cancel/destructive button (red text on the ink pill): `Cancel`,
    /// `Back`, `Discard`.
    #[must_use]
    pub fn cancel(keys: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            keys: keys.into(),
            label: label.into(),
            tone: ConfirmButtonTone::Cancel,
        }
    }
}

/// State for the shared confirmation-modal overlay: a centered dialog with a
/// title, a message, and a row of keypress buttons. Only one can be active at
/// a time (`App::confirm_modal: Option<ConfirmModal>`); setting a new one
/// replaces whatever was showing.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfirmModal {
    pub title: String,
    pub message: String,
    pub buttons: Vec<ConfirmButton>,
    pub on_confirm: ConfirmAction,
}
