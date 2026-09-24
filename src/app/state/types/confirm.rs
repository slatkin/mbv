/// What happens when the shared confirmation modal (`ConfirmModal`) is
/// answered "yes". Each variant carries whatever state its effect needs, so
/// the modal's trigger site can hand off the whole decision to the single
/// the shell's confirm-action dispatcher instead of a bespoke bool/option
/// field per confirmation.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) enum ConfirmAction {
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
    /// `pending_queue_replacement` slot, so confirming executes exactly that
    /// action through the existing playback/admission executor and cannot
    /// re-resolve into a second one. That slot is private to this arm; the
    /// save-deferral `pending_queue_action` slot stays with its own callers.
    ReplacePopulatedQueue,
}

/// State for the shared confirmation-modal overlay: a centered, bordered
/// dialog with a title, a message, and a key-binding hint line. Only one can
/// be active at a time (`App::confirm_modal: Option<ConfirmModal>`); setting
/// a new one replaces whatever was showing.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct ConfirmModal {
    pub(in crate::app) title: String,
    pub(in crate::app) message: String,
    pub(in crate::app) hint: String,
    pub(in crate::app) on_confirm: ConfirmAction,
}
