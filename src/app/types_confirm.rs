/// What happens when the shared confirmation modal (`ConfirmModal`) is
/// answered "yes". Each variant carries whatever state its effect needs, so
/// the modal's trigger site can hand off the whole decision to the single
/// `handle_key_confirm_modal` dispatcher instead of a bespoke bool/option
/// field per confirmation.
#[derive(Clone)]
pub(super) enum ConfirmAction {
    ClearQueue,
    RemoveActiveQueueItem(usize),
    RescanLibrary(usize),
    SaveOverwritePlaylist { existing_id: String, name: String },
    DiscardOrSaveDirtyPlaylist,
    DeletePlaylist { id: String, name: String },
}

/// State for the shared confirmation-modal overlay: a centered, bordered
/// dialog with a title, a message, and a key-binding hint line. Only one can
/// be active at a time (`App::confirm_modal: Option<ConfirmModal>`); setting
/// a new one replaces whatever was showing.
pub(super) struct ConfirmModal {
    pub(super) title: String,
    pub(super) message: String,
    pub(super) hint: String,
    pub(super) on_confirm: ConfirmAction,
}
