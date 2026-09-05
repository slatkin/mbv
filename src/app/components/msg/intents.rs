//! Small intent enums emitted by Interactive Components. Split from `msg.rs`
//! (task 8.3) to keep the central `Msg` file below the 800-line cap.
//!
//! These enums all share the same shape: a closed set of `Copy` variants
//! representing semantic user intent, with the component owning key
//! interpretation and the shell owning the corresponding `App` side effect.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsIntent {
    Back,
    OpenSessions,
    OpenPlaylists,
    Quit,
    Activate(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumCursorKind {
    Move,
    Jump,
    Page,
}

/// Closed set of podcast episode-mode transitions (task 5.3d.6). The
/// component performs its local episode/cursor/filter mutation and emits the
/// matching variant while episode selection is active; the shell maps it onto
/// the legacy App episode-move / filter-cycle / exit operations preserving
/// the current App episode target (D17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PodcastEpisodeTransition {
    PreviousEpisode,
    NextEpisode,
    PreviousFilter,
    NextFilter,
    Exit,
}

/// Closed set of podcast episode action intents (task 5.3d.7). The component
/// emits the intent matched from Space/Enter/Ctrl+A; the shell resolves the
/// episode-selection and wide/narrow conditions from current App state/layout
/// at the Model boundary and runs the existing App effect (D17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PodcastEpisodeIntent {
    /// Space: App enters episode selection when its episode selection is
    /// `None`; otherwise App plays its selected episode.
    FocusOrPlay,
    /// Enter: when App selection is `None`, wide podcast enters inline episode
    /// selection and narrow podcast opens the selection modal; otherwise App
    /// plays its selected episode.
    OpenOrPlay,
    /// Ctrl+A: enqueue only when App episode selection is active; otherwise
    /// no-op.
    Enqueue,
}

/// Resolved-value Audiobookshelf book browser movements
/// (split-audiobookshelf-cursor-ownership D1/D3). The component resolves the
/// movement against its own content and geometry and carries the landed
/// value; the shell applies it through the matching index-taking entry point
/// without recomputing the movement from a delta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudiobookshelfBookMove {
    /// The book-list cursor the component landed on (arrows, page keys,
    /// Home/End) — applied via `App::select_audiobookshelf_book`.
    Book(usize),
    /// The surname-bucket pill position the component landed on (`[`/`]`) —
    /// applied via `App::select_audiobookshelf_book_bucket`.
    Bucket(usize),
    /// The resolved chapter focus (`Some(row)` focuses the hero chapter list,
    /// `None` returns focus to the browser) — applied via
    /// `App::set_audiobookshelf_book_chapter_focus`.
    ChapterFocus(Option<usize>),
}

/// Closed set of Audiobookshelf book actions (task 5.3d.13-R1). The shell
/// resolves narrow/wide activation from current App state as the legacy reader
/// did, while the component owns the mounted browser's interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudiobookshelfBookIntent {
    Play,
    Activate,
    Enqueue,
    ActivateChapter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmIntent {
    Accept,
    Cancel,
    Save,
    Discard,
    Dismiss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonLostIntent {
    RestartWithTray,
    RestartWithoutTray,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteReanchorIntent {
    MoveUp,
    MoveDown,
    Accept,
    Dismiss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMenuIntent {
    MoveUp,
    MoveDown,
    Select,
    Dismiss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedsManageIntent {
    Dismiss,
    Add,
    Edit,
    Remove,
    Cancel,
    Submit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavePlaylistIntent {
    Dismiss,
    Submit,
}
