//! Baseline bench for the GUI core's step (design D12 of
//! `gui-change-driven-sections`, `docs/architecture/performance.md`).
//!
//! Each fixture builds one `CoreThread` over an `mbv` `test`-feature stub,
//! seeds the stores the fixture names outside the timed loop, and times one
//! `CoreThread::step` after one input. The batch the step sent is measured
//! once, outside the timed loop, as its `serde_json` size and reported as
//! `Throughput::Bytes`. Benches are manual runs, never part of nextest.

use std::hint::black_box;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use criterion::measurement::WallTime;
use criterion::{BenchmarkGroup, Criterion, Throughput, criterion_group, criterion_main};
use mbv::ClientTickHost;
use mbv::{
    App, stub_app_with_completed_emby_startup_and_contents, stub_app_with_playing_queue,
    stub_app_with_queue,
};
use mbv_config::TestStateDirGuard;
use mbv_emby::EmbyClient;
use mbv_emby_model::EmbyItem;
use mbv_emby_model::test_support::make_item;
use mbv_gui::bridge::library::GuiLibraryCommand;
use mbv_gui::bridge::sections::GuiSectionBatch;
use mbv_gui::core_thread::{CoreInput, CoreThread, QuitSink, SectionSink};
use mbv_net::mock_http::MockHttp;
use mbv_queue::{AudiobookshelfItem, AudiobookshelfQueueItem, QueueItem};
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};

/// Queue slots in the queue fixtures (design D12, fixtures 1 and 2).
const QUEUE_SLOTS: usize = 50;
/// Artist groups in the Music fixture; each holds `ALBUMS_PER_ARTIST` albums.
const MUSIC_ARTISTS: usize = 2_000;
const ALBUMS_PER_ARTIST: usize = 3;
/// Series in the Shows fixture (All mode).
const SERIES: usize = 300;
/// Continue Watching and recent rows in the Home fixture.
const CONTINUE_ROWS: usize = 50;
const RECENT_ROWS: usize = 100;
/// Search results in the Search fixture.
const SEARCH_RESULTS: usize = 200;
/// The query the Search fixture types.
const QUERY: &str = "bench";
/// A key with no default binding: the timed input whose step runs no command
/// and runs no command.
const TIMED_KEY: Key = Key::Function(12);

/// One core thread over a stub `App`, driven as the GUI's tick loop drives it
/// with the wait cadence zeroed, so a step with an input queued is
/// non-blocking.
struct Harness {
    /// Points the state directory at the bench's scratch path for the core's
    /// construction, so the real machine's saved launch location is never read.
    _state_dir: TestStateDirGuard,
    /// The Emby mock the seeded client sends its requests through; kept alive
    /// so the client keeps its transport.
    _http: Option<MockHttp>,
    core: CoreThread,
    tx: Sender<CoreInput>,
    batches: Receiver<GuiSectionBatch>,
    _quits: Receiver<()>,
}

impl Harness {
    fn new(app: App, http: Option<MockHttp>) -> Self {
        let state_dir = TestStateDirGuard::new_at(env!("CARGO_TARGET_TMPDIR"));
        let (tx, rx) = mpsc::channel();
        let (batch_tx, batches) = mpsc::channel();
        let sink: SectionSink = Box::new(move |batch| {
            let _ = batch_tx.send(batch);
        });
        let (quit_tx, quits) = mpsc::channel();
        let quit: QuitSink = Box::new(move || {
            let _ = quit_tx.send(());
        });
        let mut core = CoreThread::new(app, rx, sink, quit);
        core.set_idle_cadence_for_test(Duration::ZERO);
        core.install_waker(tx.clone());
        Self {
            _state_dir: state_dir,
            _http: http,
            core,
            tx,
            batches,
            _quits: quits,
        }
    }

    /// The App the fixture seeds, after the core exists: the construction's
    /// launch load then sees no Service, as the GUI library tests seed.
    fn app_mut(&mut self) -> &mut App {
        self.core.app_mut()
    }

    fn send(&self, input: CoreInput) {
        self.tx
            .send(input)
            .expect("the core thread's channel stays connected");
    }

    /// Run one step, asserting the core keeps running, and return the batch it
    /// sent. A step that changes no section sends none, so the default batch
    /// (empty) is the answer.
    fn step(&mut self) -> GuiSectionBatch {
        assert!(self.core.step(), "the core thread quit during the bench");
        self.batches.try_iter().last().unwrap_or_default()
    }

    /// One input, then one step, then the batch that step sent: the timed unit.
    fn input_then_step(&mut self) -> GuiSectionBatch {
        self.send(CoreInput::Key(KeyEvent::new(TIMED_KEY, KeyModifiers::NONE)));
        self.step()
    }

    /// The emitted payload's serialized size in bytes at steady state,
    /// measured outside the timed loop. A library section rides a frame only
    /// when its content changed, so the step after the seeding carries the
    /// whole section; the step after that, the one the timed loop repeats,
    /// carries none of it. The bytes describe the timed unit.
    fn payload_bytes(&mut self) -> u64 {
        let _settled = self.input_then_step();
        let batch = self.input_then_step();
        let len = serde_json::to_vec(&batch)
            .expect("the frame serializes")
            .len();
        u64::try_from(len).expect("a payload length fits in u64")
    }
}

/// An Emby client over the in-memory mock transport, so no request leaves the
/// process (design D13 of `gui-change-driven-sections`' test seams).
fn mock_emby_client(http: &MockHttp) -> EmbyClient {
    EmbyClient::new(mbv::Config {
        server_url: "http://127.0.0.1:1".into(),
        ..mbv::Config::default()
    })
    .with_test_agent(http.agent())
}

/// A folder item with the id the fixture names it by.
fn folder(id: String, name: &str, item_type: &str) -> EmbyItem {
    let mut item = make_item(name, item_type);
    item.id = id;
    item.is_folder = true;
    item
}

fn artist_id(index: usize) -> String {
    format!("artist-{index:04}")
}

fn album_id(artist: usize, album: usize) -> String {
    format!("album-{artist:04}-{album}")
}

/// Fixture 3: the Grouped Music library's group level, one row per artist.
fn artist_groups() -> Vec<EmbyItem> {
    (0..MUSIC_ARTISTS)
        .map(|index| {
            folder(
                artist_id(index),
                &format!("Artist {index:04}"),
                "MusicArtist",
            )
        })
        .collect()
}

/// Fixture 4: the Shows library's root level, one row per series.
fn series_items() -> Vec<EmbyItem> {
    (0..SERIES)
        .map(|index| {
            folder(
                format!("series-{index:04}"),
                &format!("Series {index:04}"),
                "Series",
            )
        })
        .collect()
}

/// Fixture 5: one movie row per Continue Watching slot.
fn continue_items() -> Vec<EmbyItem> {
    (0..CONTINUE_ROWS)
        .map(|index| {
            folder(
                format!("cw-{index:04}"),
                &format!("Continue {index:04}"),
                "Movie",
            )
        })
        .collect()
}

/// Fixture 5: one Audiobookshelf episode row per recent slot, the shape the
/// Home `Recently added` fetch delivers.
fn recent_items() -> Vec<QueueItem> {
    (0..RECENT_ROWS)
        .map(|index| {
            QueueItem::Audiobookshelf(AudiobookshelfItem::Episode(AudiobookshelfQueueItem {
                library_item_id: "show-bench".into(),
                episode_id: format!("episode-{index:04}"),
                title: format!("Episode {index:04}"),
                show_title: None,
                author: None,
                description: None,
                duration_ticks: None,
                position_ticks: 0,
                played: false,
                pub_date_secs: None,
                is_finished: false,
                cover_path: None,
            }))
        })
        .collect()
}

/// Fixture 6: the movie results a search returns.
fn search_items() -> Vec<EmbyItem> {
    (0..SEARCH_RESULTS)
        .map(|index| {
            folder(
                format!("movie-{index:04}"),
                &format!("Movie {index:04}"),
                "Movie",
            )
        })
        .collect()
}

/// Fixture 1: an idle step over a 50-slot queue, nothing playing.
fn idle_queue() -> Harness {
    Harness::new(stub_app_with_queue(QUEUE_SLOTS), None)
}

/// Fixture 2: a playing step over a 50-slot queue.
fn playing_queue() -> Harness {
    Harness::new(stub_app_with_playing_queue(QUEUE_SLOTS), None)
}

/// Fixture 3: a Grouped Music library with 2,000 artists and 6,000 albums,
/// with one album picked in the selected (first) group.
fn music_grouped() -> Harness {
    let http = MockHttp::new();
    let mut harness = Harness::new(stub_app_with_queue(0), Some(http.clone()));
    let app = harness.app_mut();
    app.seed_gui_music_groups_for_test(mock_emby_client(&http), artist_groups());
    for artist in 0..MUSIC_ARTISTS {
        let albums: Vec<(String, String)> = (0..ALBUMS_PER_ARTIST)
            .map(|album| (album_id(artist, album), format!("Album {album}")))
            .collect();
        let refs: Vec<(&str, &str)> = albums
            .iter()
            .map(|(id, name)| (id.as_str(), name.as_str()))
            .collect();
        app.seed_gui_music_albums_for_test(0, &artist_id(artist), &refs);
    }
    app.pick_gui_album_for_test(0, &artist_id(0), &album_id(0, 1));
    harness
}

/// Fixture 4: the Shows library in All mode with 300 series.
fn shows_all() -> Harness {
    let http = MockHttp::new();
    // Seeded after the core exists, as `music_grouped` seeds: a Ready Emby
    // client seeded before the construction sends the launch Home load into
    // the mock, which stalls the construction for the mock's hour-long stall.
    let mut harness = Harness::new(stub_app_with_queue(0), Some(http.clone()));
    harness
        .app_mut()
        .seed_gui_tv_series_for_test(mock_emby_client(&http), series_items());
    harness.step();
    harness
}

/// Fixture 5: Home with 50 Continue Watching rows and 100 recent rows, opened
/// at its destination position.
fn home_rows() -> Harness {
    let mut app = stub_app_with_completed_emby_startup_and_contents(Vec::new(), continue_items());
    app.queue_home_recently_added_for_test(recent_items());
    let mut harness = Harness::new(app, None);
    harness.send(CoreInput::Library(
        GuiLibraryCommand::OpenDestination { position: 0 }.into(),
    ));
    harness.step();
    harness
}

/// Fixture 6: Search open with 200 results for `QUERY`.
fn search_open() -> Harness {
    let http = MockHttp::new();
    let mut harness = Harness::new(stub_app_with_queue(0), Some(http.clone()));
    harness.send(CoreInput::Library(
        GuiLibraryCommand::SearchQuery {
            query: QUERY.into(),
        }
        .into(),
    ));
    harness.step();
    harness
        .app_mut()
        .seed_gui_music_library_for_test(mock_emby_client(&http));
    harness
        .app_mut()
        .send_gui_search_result_for_test(QUERY, search_items());
    harness.step();
    harness
}

/// Fixture 7: the Settings screen open on its F2 section.
fn settings_f2() -> Harness {
    let mut harness = Harness::new(stub_app_with_queue(0), None);
    harness.send(CoreInput::Key(KeyEvent::new(
        Key::Function(2),
        KeyModifiers::NONE,
    )));
    harness.step();
    harness
}

/// Time one fixture: measure its payload once, then time the input-and-step
/// unit with the payload size as the group's throughput.
fn bench_fixture(group: &mut BenchmarkGroup<'_, WallTime>, name: &str, mut harness: Harness) {
    let bytes = harness.payload_bytes();
    group.throughput(Throughput::Bytes(bytes));
    group.bench_function(name, |b| {
        b.iter(|| {
            let frame = harness.input_then_step();
            black_box(frame);
        });
    });
}

fn sections(c: &mut Criterion) {
    let mut group = c.benchmark_group("sections");
    bench_fixture(&mut group, "idle_queue", idle_queue());
    bench_fixture(&mut group, "playing_queue", playing_queue());
    bench_fixture(&mut group, "music_grouped", music_grouped());
    bench_fixture(&mut group, "shows_all", shows_all());
    bench_fixture(&mut group, "home_rows", home_rows());
    bench_fixture(&mut group, "search_open", search_open());
    bench_fixture(&mut group, "settings_f2", settings_f2());
    group.finish();
}

criterion_group!(benches, sections);
criterion_main!(benches);
