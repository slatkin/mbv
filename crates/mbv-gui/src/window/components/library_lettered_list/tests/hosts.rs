//! The four destinations' harness facts (`gui-tech-debt-simplification` design
//! D10): the sample rows, ids, texts, and commands each `Host` impl names, so a
//! destination's own facts stay in one place and the shared contracts in
//! [`super::common`] never restate them.

use super::*;

impl Host for EmbyList {
    const WHICH: Which = Which::Emby;
    // The plain Emby library's harness data (design D4).
    const TAB: usize = 3;
    const MARK: &'static str = "emby";
    const IDS: [&'static str; 3] = ["m1", "m2", "m3"];
    const SELECTED: &'static str = "m1";
    const FIRST_SUBTITLE: &'static str = "1979";
    const SECOND_DATE: &'static str = "1998";

    fn wrap_list(list: Self::List) -> GuiDestinationList {
        GuiDestinationList::Emby(list)
    }

    fn wrap_detail(detail: Self::Detail) -> GuiLibraryDetail {
        GuiLibraryDetail::Emby(detail)
    }

    fn rows() -> Vec<Self::Row> {
        emby_movies()
    }

    fn frame(rows: Vec<Self::Row>, loading: bool, failed: bool) -> Self::List {
        GuiEmbyList {
            loading,
            failed,
            ..emby_frame(false, rows)
        }
    }

    fn detail_of(selected: Option<&str>) -> Self::Detail {
        emby_detail(selected)
    }

    fn select_command(id: &str) -> crate::gui_core::GuiLibraryCommand {
        crate::gui_core::GuiLibraryCommand::EmbySelect {
            lib_idx: 3,
            row: id.to_owned(),
        }
    }

    fn play_command(id: &str) -> crate::gui_core::GuiLibraryCommand {
        crate::gui_core::GuiLibraryCommand::EmbyPlay {
            lib_idx: 3,
            item_id: id.to_owned(),
        }
    }
}

impl Host for AudiobooksList {
    const WHICH: Which = Which::Audiobooks;
    const TAB: usize = 4;
    const MARK: &'static str = "audiobookshelf";
    const IDS: [&'static str; 3] = ["b1", "b2", "b3"];
    const SELECTED: &'static str = "b3";
    const FIRST_SUBTITLE: &'static str = "Bill Wyman";
    const SECOND_DATE: &'static str = "9 May";

    fn wrap_list(list: Self::List) -> GuiDestinationList {
        GuiDestinationList::Audiobooks(list)
    }

    fn wrap_detail(detail: Self::Detail) -> GuiLibraryDetail {
        GuiLibraryDetail::Audiobooks(detail)
    }

    fn rows() -> Vec<Self::Row> {
        audiobooks_catalog()
    }

    fn frame(rows: Vec<Self::Row>, loading: bool, failed: bool) -> Self::List {
        GuiAudiobooksList {
            loading,
            failed,
            ..audiobooks_frame(rows)
        }
    }

    fn detail_of(selected: Option<&str>) -> Self::Detail {
        audiobooks_detail(selected)
    }

    fn select_command(id: &str) -> crate::gui_core::GuiLibraryCommand {
        crate::gui_core::GuiLibraryCommand::AbsSelect {
            lib_idx: 4,
            row: id.to_owned(),
        }
    }

    fn play_command(id: &str) -> crate::gui_core::GuiLibraryCommand {
        crate::gui_core::GuiLibraryCommand::AbsPlay {
            lib_idx: 4,
            item_id: id.to_owned(),
        }
    }
}

impl Host for PodcastsList {
    const WHICH: Which = Which::Podcasts;
    const TAB: usize = 2;
    const MARK: &'static str = "audiobookshelf";
    const IDS: [&'static str; 3] = ["e1:ep1", "e2:ep2", "e3:ep3"];
    const SELECTED: &'static str = "e1:ep1";
    const FIRST_SUBTITLE: &'static str = "Show One";
    const SECOND_DATE: &'static str = "9 May";

    fn wrap_list(list: Self::List) -> GuiDestinationList {
        GuiDestinationList::Podcasts(list)
    }

    fn wrap_detail(detail: Self::Detail) -> GuiLibraryDetail {
        GuiLibraryDetail::Podcasts(detail)
    }

    fn rows() -> Vec<Self::Row> {
        entry_rows(
            GuiRowMediaKindFrame::Audio,
            ["Show One", "Show Two", "Show One"],
        )
    }

    fn frame(rows: Vec<Self::Row>, loading: bool, failed: bool) -> Self::List {
        podcasts_frame(rows, loading, failed)
    }

    fn detail_of(selected: Option<&str>) -> Self::Detail {
        podcasts_detail(selected)
    }

    fn select_command(id: &str) -> crate::gui_core::GuiLibraryCommand {
        crate::gui_core::GuiLibraryCommand::PodcastsSelect {
            lib_idx: 0,
            row: id.to_owned(),
        }
    }

    fn play_command(id: &str) -> crate::gui_core::GuiLibraryCommand {
        crate::gui_core::GuiLibraryCommand::PodcastsPlay {
            lib_idx: 0,
            item_id: id.to_owned(),
        }
    }
}

impl Host for FeedsList {
    const WHICH: Which = Which::Feeds;
    const TAB: usize = 2;
    const MARK: &'static str = "rss";
    const IDS: [&'static str; 3] = ["s1::e1", "s2::e2", "s1::e3"];
    const SELECTED: &'static str = "s1::e1";
    const FIRST_SUBTITLE: &'static str = "Sub One";
    const SECOND_DATE: &'static str = "9 May";

    fn wrap_list(list: Self::List) -> GuiDestinationList {
        GuiDestinationList::Feeds(list)
    }

    fn wrap_detail(detail: Self::Detail) -> GuiLibraryDetail {
        GuiLibraryDetail::Feeds(detail)
    }

    fn rows() -> Vec<Self::Row> {
        let mut rows = entry_rows(
            GuiRowMediaKindFrame::Video,
            ["Sub One", "Sub Two", "Sub One"],
        );
        rows[0].id = "s1::e1".to_owned();
        rows[1].id = "s2::e2".to_owned();
        rows[2].id = "s1::e3".to_owned();
        rows
    }

    fn frame(rows: Vec<Self::Row>, loading: bool, failed: bool) -> Self::List {
        feeds_frame(rows, loading, failed)
    }

    fn detail_of(selected: Option<&str>) -> Self::Detail {
        feeds_detail(selected)
    }

    fn select_command(id: &str) -> crate::gui_core::GuiLibraryCommand {
        crate::gui_core::GuiLibraryCommand::FeedsSelect { row: id.to_owned() }
    }

    fn play_command(id: &str) -> crate::gui_core::GuiLibraryCommand {
        crate::gui_core::GuiLibraryCommand::FeedsPlay {
            item_id: id.to_owned(),
        }
    }
}
