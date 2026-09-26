//! Regression tests for Audiobookshelf browse states and browse-kind resolution.

use crate::app::render::FeedAgeGroup;
use mbv_core::audiobookshelf::{
    AudiobookshelfAudioFile, AudiobookshelfBook, AudiobookshelfDownloadedEpisode,
    AudiobookshelfLibrary, AudiobookshelfProgress, AudiobookshelfShow,
};

use super::books::{SURNAME_BUCKET_LABELS, SURNAME_BUCKET_UPPER};
use super::*;
use mbv_core::audiobookshelf::audiobook_author_sort_key;
use mbv_core::config::AudiobookshelfBookBucket;

fn library() -> AudiobookshelfLibrary {
    AudiobookshelfLibrary {
        id: "library".into(),
        name: "Podcasts".into(),
        media_type: "podcast".into(),
    }
}

fn show(id: &str, title: &str) -> AudiobookshelfShow {
    AudiobookshelfShow {
        library_item_id: id.into(),
        title: title.into(),
        author: None,
        description: None,
        cover_path: None,
    }
}

fn episode(show: &str, id: &str) -> AudiobookshelfDownloadedEpisode {
    AudiobookshelfDownloadedEpisode {
        library_item_id: show.into(),
        episode_id: id.into(),
        title: id.into(),
        description: None,
        published_at: None,
        duration_seconds: None,
    }
}

#[test]
fn episode_cache_fills_progressively_per_show_and_dedupes() {
    let mut state = AudiobookshelfBrowseState::new(library());
    state.append_page(1, 20, 2, vec![show("a", "A"), show("b", "B")]);

    // Each show's fetch lands separately and joins the flat view without
    // disturbing the other shows' cached entries.
    state.cache_detail(
        "a".into(),
        vec![episode("a", "a-one"), episode("a", "a-two")],
    );
    assert_eq!(
        state
            .visible_episodes(AudiobookshelfEpisodeFilter::All)
            .into_iter()
            .map(|episode| episode.episode_id.as_str())
            .collect::<Vec<_>>(),
        ["a-one", "a-two"]
    );

    state.cache_detail("b".into(), vec![episode("b", "b-one")]);
    assert_eq!(
        state
            .visible_episodes(AudiobookshelfEpisodeFilter::All)
            .into_iter()
            .map(|episode| episode.episode_id.as_str())
            .collect::<Vec<_>>(),
        ["a-one", "a-two", "b-one"],
        "the flat view concatenates the cached shows in show order"
    );
    assert_eq!(
        state.detail_cache["a"],
        vec![episode("a", "a-one"), episode("a", "a-two")]
    );

    // A re-arrival replaces the show's entry: append at most once, never
    // a duplicate.
    state.cache_detail("a".into(), vec![episode("a", "a-one")]);
    assert_eq!(
        state
            .visible_episodes(AudiobookshelfEpisodeFilter::All)
            .into_iter()
            .map(|episode| episode.episode_id.as_str())
            .collect::<Vec<_>>(),
        ["a-one", "b-one"]
    );
}

#[test]
fn cache_arrivals_keep_the_selected_episode_and_refresh_clears_it() {
    let mut state = AudiobookshelfBrowseState::new(library());
    state.append_page(1, 20, 2, vec![show("a", "A"), show("b", "B")]);
    state.selected_episode = Some(("a".into(), "a-one".into()));

    state.cache_detail("b".into(), vec![episode("b", "b-one")]);
    assert_eq!(
        state.selected_episode,
        Some(("a".into(), "a-one".into())),
        "a later show's cache arrival keeps the selected episode"
    );

    // Refresh: the cache reloads from the fan-out and the selected
    // episode identity goes with it.
    state.cache_detail("a".into(), vec![episode("a", "a-one")]);
    state.clear_episodes();
    assert!(state.detail_cache.is_empty());
    assert!(state.detail_loading_ids.is_empty());
    assert_eq!(state.selected_episode, None);
}

#[test]
fn episode_by_identity_resolves_across_shows_and_requires_both_ids() {
    let mut state = AudiobookshelfBrowseState::new(library());
    state.append_page(1, 20, 2, vec![show("a", "A"), show("b", "B")]);
    state.cache_detail("a".into(), vec![episode("a", "shared")]);
    state.cache_detail("b".into(), vec![episode("b", "shared")]);

    assert_eq!(
        state
            .episode_by_identity("b", "shared")
            .map(|episode| episode.library_item_id.as_str()),
        Some("b"),
        "the same episode id on two shows stays isolated by show identity"
    );
    assert!(state.episode_by_identity("a", "missing").is_none());
    assert!(state.episode_by_identity("c", "shared").is_none());
}

#[test]
fn filters_completed_progress_and_treats_partial_as_unplayed() {
    let mut state = AudiobookshelfBrowseState::new(library());
    state.append_page(0, 20, 1, vec![show("a", "A")]);
    state.cache_detail(
        "a".into(),
        vec![
            episode("a", "finished"),
            episode("a", "partial"),
            episode("a", "missing"),
        ],
    );
    state.progress.insert(
        ("a".into(), "finished".into()),
        AudiobookshelfProgress {
            library_item_id: "a".into(),
            episode_id: "finished".into(),
            current_time_seconds: 1.0,
            is_finished: true,
        },
    );
    state.progress.insert(
        ("a".into(), "partial".into()),
        AudiobookshelfProgress {
            library_item_id: "a".into(),
            episode_id: "partial".into(),
            current_time_seconds: 1.0,
            is_finished: false,
        },
    );

    assert_eq!(
        state.visible_episodes(AudiobookshelfEpisodeFilter::Played)[0].episode_id,
        "finished"
    );
    assert_eq!(
        state
            .visible_episodes(AudiobookshelfEpisodeFilter::Unplayed)
            .into_iter()
            .map(|episode| episode.episode_id.as_str())
            .collect::<Vec<_>>(),
        ["partial", "missing"]
    );
}

#[test]
fn visible_episodes_are_newest_first_with_undated_last() {
    let mut state = AudiobookshelfBrowseState::new(library());
    state.append_page(0, 20, 2, vec![show("a", "A"), show("b", "B")]);
    state.cache_detail(
        "a".into(),
        vec![
            episode_with_date("a", "old", Some(1_767_225_600)),
            episode_with_date("a", "new", Some(1_786_492_800)),
        ],
    );
    state.cache_detail("b".into(), vec![episode_with_date("b", "undated", None)]);

    assert_eq!(
        state
            .visible_episodes(AudiobookshelfEpisodeFilter::All)
            .into_iter()
            .map(|episode| episode.episode_id.as_str())
            .collect::<Vec<_>>(),
        ["new", "old", "undated"],
        "the flat union sorts newest-first globally, undated episodes last"
    );
}

#[test]
fn display_rows_insert_non_selectable_groups_without_changing_indices() {
    const DAY: u64 = 24 * 60 * 60;
    let now = 30 * DAY;
    // Deliberately interleaved and unsorted: the builder sorts the flat
    // slice newest-first globally before merging consecutive runs, and a
    // group with no episodes produces no heading.
    let episodes = vec![
        episode_with_date("a", "month", Some(now - 30 * DAY)),
        episode_with_date("a", "new", Some(now)),
        episode_with_date("a", "undated", None),
        episode_with_date("a", "recent", Some(now - 2 * DAY)),
    ];

    assert_eq!(
        podcast_display_rows(&episodes, now),
        vec![
            PodcastDisplayRow::Heading(FeedAgeGroup::New),
            PodcastDisplayRow::Entry(1),
            PodcastDisplayRow::Spacer,
            PodcastDisplayRow::Heading(FeedAgeGroup::Recent),
            PodcastDisplayRow::Entry(3),
            PodcastDisplayRow::Spacer,
            PodcastDisplayRow::Heading(FeedAgeGroup::OlderThanMonth),
            PodcastDisplayRow::Entry(0),
            PodcastDisplayRow::Spacer,
            PodcastDisplayRow::Heading(FeedAgeGroup::Unknown),
            PodcastDisplayRow::Entry(2),
        ],
        "undated episodes sort last and group as `Unknown date`; empty groups are omitted"
    );
}

#[test]
fn book_pages_group_by_author_surname_only() {
    let mut state = AudiobookshelfBookBrowseState::new(library());
    state.append_page_books(
        0,
        3,
        vec![
            book("c", "Title C", "Zelda Author"),
            book("a", "Title A", "Alpha Author"),
            book("b", "Title B", "Beta Author"),
        ],
    );
    assert_eq!(
        state
            .books
            .iter()
            .map(|b| b.library_item_id.as_str())
            .collect::<Vec<_>>(),
        ["a", "b", "c"],
        "books group and sort by author surname, not title"
    );
}

#[test]
fn visible_rows_fall_back_to_audio_files_when_chapters_empty() {
    let mut state = AudiobookshelfBookBrowseState::new(library());
    let id = "book-1";
    state.selected_id = Some(id.into());
    state.detail_cache.insert(
        id.into(),
        (
            Vec::new(),
            vec![
                AudiobookshelfAudioFile {
                    index: 1,
                    ino: "f1".into(),
                    duration: 100.0,
                },
                AudiobookshelfAudioFile {
                    index: 2,
                    ino: "f2".into(),
                    duration: 200.0,
                },
            ],
        ),
    );
    let rows = state.visible_rows(id);
    assert_eq!(rows.len(), 2);
    assert!(matches!(rows[0], BookRow::AudioFile { index: 1, .. }));
}

fn book(id: &str, title: &str, author: &str) -> AudiobookshelfBook {
    AudiobookshelfBook {
        library_item_id: id.into(),
        title: title.into(),
        author_display: Some(author.into()),
        author_sort_key: audiobook_author_sort_key(author),
        cover_path: None,
        duration_seconds: 0.0,
        narrator: None,
        published_year: None,
        genres: Vec::new(),
        description: None,
        series_name: None,
        chapters: Vec::new(),
        audio_files: Vec::new(),
    }
}

fn episode_with_date(
    show: &str,
    id: &str,
    published_at: Option<u64>,
) -> AudiobookshelfDownloadedEpisode {
    AudiobookshelfDownloadedEpisode {
        library_item_id: show.into(),
        episode_id: id.into(),
        title: id.into(),
        description: None,
        published_at,
        duration_seconds: None,
    }
}
