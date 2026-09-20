//! Small intent enums emitted by Interactive Components. Split from `msg.rs`
//! (task 8.3) to keep the central `Msg` file below the 800-line cap.
//!
//! These enums all share the same shape: a closed set of `Copy` variants
//! representing semantic user intent, with the component owning key
//! interpretation and the shell owning the corresponding `App` side effect.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeRowTarget {
    pub item_id: Option<String>,
    pub source: Option<String>,
    pub from_continue_watching: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodcastEpisodeTarget {
    pub(crate) library_item_id: String,
    pub(crate) episode_id: String,
}

impl PodcastEpisodeTarget {
    pub fn new(library_item_id: String, episode_id: String) -> Self {
        Self {
            library_item_id,
            episode_id,
        }
    }

    pub(crate) fn library_item_id(&self) -> &str {
        &self.library_item_id
    }

    pub(crate) fn episode_id(&self) -> &str {
        &self.episode_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookChapterTarget {
    pub(crate) book_library_item_id: String,
    /// The stable row discriminator from the Service data: the chapter number
    /// for chapter rows, or the audio-part index for the audio-file fallback
    /// (design.md D4). Never a display position.
    pub(crate) row_discriminator: usize,
}

impl BookChapterTarget {
    pub fn new(book_library_item_id: String, row_discriminator: usize) -> Self {
        Self {
            book_library_item_id,
            row_discriminator,
        }
    }

    pub(crate) fn book_library_item_id(&self) -> &str {
        &self.book_library_item_id
    }

    pub(crate) fn row_discriminator(&self) -> usize {
        self.row_discriminator
    }
}

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

/// Direct actions over a focused Grouped Music artist. The tree owner resolves
/// the artist root to ordered album items before this intent crosses the
/// component boundary; the artist identity itself is never a target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicTreeAction {
    Play,
    Enqueue,
    Shuffle,
}

/// Stable, component-resolved identity for one focused Grouped Music artist
/// (design D7). The tree owner resolves the root's settled identity, display
/// name, and leaf album targets before this crosses the boundary; the shell
/// uses the settled revision and the opaque targets to reject late
/// completions and to aggregate fallback tracks without re-reading a cursor.
/// A `None` artist ID is the explicit fallback arm: the root only has the
/// deterministic grouping key, so no provider-ID request may be invented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicArtistTarget {
    pub artist_id: Option<String>,
    pub artist_name: String,
    pub album_targets: Vec<String>,
    pub revision: u64,
}

impl MusicArtistTarget {
    /// Whether two targets address the same artist source, ignoring the
    /// settled revision. The shell binds a projection to the owner-resolved
    /// target's revision at push time, so the revision is a shell staleness
    /// axis; presentation identity is the artist identity, display name, and
    /// in-scope album set.
    pub(in crate::app) fn same_source(&self, other: &Self) -> bool {
        self.artist_id == other.artist_id
            && self.artist_name == other.artist_name
            && self.album_targets == other.album_targets
    }
}

/// Closed set of podcast episode action intents (task 5.3d.7). The component
/// emits the intent matched from Space/Enter/Ctrl+A; the shell runs the App
/// play/enqueue effect directly — episodes are the tab's leaf rows, so there
/// is no episode-selection or overlay stage (reorganize-podcast-pill-
/// navigation D6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PodcastEpisodeIntent {
    /// Space: App plays the carried target; a selectionless activation is a
    /// no-op (the episode-selection path left with the show browser,
    /// reorganize-podcast-pill-navigation 3.1).
    FocusOrPlay(Option<PodcastEpisodeTarget>),
    /// Enter or double-click: App plays the carried episode target
    /// immediately, in every geometry — podcast episodes are not
    /// hero-bearing rows and no overlay or inline detail opens (design D6).
    /// A selectionless activation is a no-op.
    OpenOrPlay(Option<PodcastEpisodeTarget>),
    /// Ctrl+A: enqueue the carried target; a selectionless activation is a
    /// no-op.
    Enqueue(Option<PodcastEpisodeTarget>),
}

/// Resolved-value Audiobookshelf book browser movements
/// (split-audiobookshelf-cursor-ownership D1/D3). The component resolves the
/// movement against its own content and geometry and carries the landed
/// value; the shell applies it through the matching index-taking entry point
/// without recomputing the movement from a delta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudiobookshelfBookMove {
    /// The book-list target the component landed on (arrows, page keys,
    /// Home/End) — applied via the stable-id resolver.
    Book(Option<String>),
    /// The surname-bucket pill position the component landed on (`[`/`]`) —
    /// applied via `App::select_audiobookshelf_book_bucket`.
    Bucket(usize),
    /// The resolved chapter focus (`Some(row)` focuses the hero chapter list,
    /// `None` returns focus to the browser) — applied via
    /// `App::set_audiobookshelf_book_chapter_focus`.
    ChapterFocus(Option<BookChapterTarget>),
}

/// Closed set of Audiobookshelf book actions (task 5.3d.13-R1). The shell
/// resolves narrow/wide activation from current App state as the legacy reader
/// did, while the component owns the mounted browser's interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudiobookshelfBookIntent {
    Play,
    Activate,
    Enqueue,
    /// Enter on a selected book: the shell focuses the wide chapter
    /// workspace (the media-selection list in the hero pane). Narrow keeps
    /// the Library Hero overlay, mirroring `PodcastEpisodeIntent::OpenOrPlay(None)`.
    FocusChapters,
    ActivateChapter(Option<BookChapterTarget>),
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
