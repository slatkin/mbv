//! The GUI Library's one pill-scoped entry-list projection (row 7C.4, from
//! the /simplify review of PR 993): the pill option, entry row, and
//! projection frame the Feeds (row 11.1) and Podcasts (row 10.1)
//! destinations share, the row-state arm both of their row builders apply,
//! and the select-by-id arm the Audiobooks destination (row 9.1) shares with
//! them. Each destination's own module keeps what is truly its own — the
//! pill scopes' sources, the row builder, the Hero, and the loads — so the
//! share carries only the vocabulary every list paints and the fallback
//! rules every list applies. Audiobooks has no pill row (its letter rail
//! replaces the TUI's surname buckets) and a book row of its own fields, so
//! it takes the selection arm only.

use super::home::GuiHomeRowState;

/// One pill of a pill-scoped destination's pill row (rows 10.1, 11.1):
/// Latest first, then one pill per scope in the destination's order. The
/// pill row is single-select, as the TUI's one-valued `PillSelection` is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiPillOption<P> {
    /// The pill's selection: the set-pill command carries it back.
    pub pill: P,
    /// The pill's label: `Latest`, or the scope's own name.
    pub name: String,
    /// Whether the core's stored pill is this one.
    pub selected: bool,
}

/// One row of a pill-scoped destination's entry list (rows 10.1, 11.1). The
/// identity is the entry's stable Service identity, encoded as one opaque
/// string a select, play, or enqueue command sends, never a row position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiEntryRow {
    /// The entry identity, as the destination's id encoding writes it.
    pub id: String,
    /// The entry's display title, the row's primary label.
    pub title: String,
    /// The row's secondary label — the fact that names the row's scope (the
    /// subscription, the show) in the merged Latest view.
    pub subtitle: Option<String>,
    /// The gutter date: the TUI row's fixed right-aligned `day month` text
    /// (`fmt_publish_date_short` over the publish instant). `None` on an
    /// undated entry, and `Some("")` never rides here — the TUI's rows drop
    /// an empty gutter too.
    pub date: Option<String>,
    /// The row's played state: the TUI row's arm — `Played` for a finished
    /// entry, `Active` with the resume percent while some of it is played,
    /// otherwise `Ordinary`.
    pub state: GuiHomeRowState,
}

/// The pill-scoped entry-list projection a destination paints (rows 10.1,
/// 11.1): the single-select pill row, the active pill's entry rows, the
/// core's selected entry, and its Hero. No letter rail rides here — the
/// lists are sorted by date — and no Workspace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiEntryListProjection<P, H> {
    /// The pill row in paint order: Latest first, then each scope in the
    /// destination's order — the subscriptions for Feeds, the state pills and
    /// the shows for Podcasts. The selected pill is the core's effective one;
    /// a scope the destination no longer holds falls back to Latest.
    pub pills: Vec<GuiPillOption<P>>,
    /// Whether the active view's content is still landing: the feed tab's
    /// fetch for Feeds, the show catalog's pages and the active view's own
    /// episode fetches for Podcasts.
    pub loading: bool,
    /// Whether the active view's fetch failed: always `false` for Feeds — the
    /// tab keeps no failure state, a failed subscription flashes the TUI's
    /// toast and keeps its previous entries — and the browse state's error
    /// for Podcasts, whose published episodes keep serving when they can.
    pub failed: bool,
    /// The active pill's entry rows: a Latest pill merges every scope's
    /// entries, and a scoped pill holds that one scope's, both newest first
    /// with undated entries last.
    pub rows: Vec<GuiEntryRow>,
    /// The core's selected entry (rows 10.1, 11.1), as the frame painted it:
    /// the pick the select command last named while the active list still
    /// carries it, else that list's first row — the Hero default, so the Hero
    /// paints as soon as a list lands. `None` while the list holds no row.
    pub selected: Option<String>,
    /// The selected entry's Hero facts; `None` while no entry is selected.
    pub hero: Option<H>,
}

impl<P, H> Default for GuiEntryListProjection<P, H> {
    fn default() -> Self {
        Self {
            pills: Vec::new(),
            loading: false,
            failed: false,
            rows: Vec::new(),
            selected: None,
            hero: None,
        }
    }
}

/// The pill row (rows 10.1, 11.1): `latest` first, then each scoped pill
/// `scoped` yields in the destination's order, with the effective pill
/// `selected` marked.
pub(in crate::app::gui_library) fn pill_options<P: Clone + PartialEq>(
    latest: &P,
    selected: &P,
    scoped: impl IntoIterator<Item = (P, String)>,
) -> Vec<GuiPillOption<P>> {
    let mut pills = vec![GuiPillOption {
        pill: latest.clone(),
        name: "Latest".to_string(),
        selected: selected == latest,
    }];
    pills.extend(scoped.into_iter().map(|(pill, name)| GuiPillOption {
        selected: *selected == pill,
        pill,
        name,
    }));
    pills
}

/// The row the Hero shows (rows 9.1, 10.1, 11.1): the core's pick while the
/// list still carries it, else the list's first row — the Hero default, so
/// the Hero paints as soon as a list lands. `None` while the list holds no
/// row. `listed` is the list's membership test over a row id, and `first`
/// yields the list's first row's id.
pub(in crate::app::gui_library) fn picked_or_first(
    picked: Option<String>,
    listed: impl Fn(&str) -> bool,
    first: impl FnOnce() -> Option<String>,
) -> Option<String> {
    picked.filter(|picked| listed(picked)).or_else(first)
}
