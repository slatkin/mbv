use crate::app::ui_util::natural_sort_key;

/// For folder-based music libraries where albums are stored as directories named
/// "Artist (YYYY) Album Title", parse out the three components.
/// Returns `(artist, year, album_title)` on success.
pub(crate) fn parse_album_folder_name(name: &str) -> Option<(String, u32, String)> {
    let mut search_from = 0;
    while let Some(rel) = name[search_from..].find(" (") {
        let sp_pos = search_from + rel; // position of the space before '('
        let after_open = sp_pos + 2; // position of first char after '('
        if let Some(close_rel) = name[after_open..].find(')') {
            let year_str = &name[after_open..after_open + close_rel];
            if year_str.len() == 4 {
                if let Ok(year) = year_str.parse::<u32>() {
                    let close_pos = after_open + close_rel; // position of ')'
                    if name[close_pos..].starts_with(") ") {
                        let artist = name[..sp_pos].to_string();
                        let album = name[close_pos + 2..].to_string();
                        return Some((artist, year, album));
                    }
                }
            }
        }
        search_from = sp_pos + 2;
    }
    None
}

/// Strips a leading article ("The ", "A ", "An ") from `s` (case-insensitive).
/// Returns a slice of the original string starting after the article.
pub(crate) fn strip_article(s: &str) -> &str {
    for prefix in &["the ", "a ", "an "] {
        // `s.get(..prefix.len())` returns `None` (rather than panicking, as a
        // byte-index slice would) when `prefix.len()` doesn't land on a UTF-8
        // char boundary — e.g. an accented artist name where the boundary
        // falls inside a multi-byte character.
        if let Some(head) = s.get(..prefix.len()) {
            if head.eq_ignore_ascii_case(prefix) {
                return &s[prefix.len()..];
            }
        }
    }
    s
}

/// Best-effort natural sort key for an album's display artist, computed
/// synchronously (Emby tag or folder-name heuristic only — no network fetch,
/// no cache lookup). Used to pick a sane initial cursor position when a
/// music-group album level first loads (see `handle_lib_event`'s
/// `LibEvent::Loaded` arm in `actions.rs`), before its grouping candidate
/// has settled. Mirrors `derive_album_artist`'s synchronous fallback chain
/// (Emby tag → folder-name-parsed artist → literal "Unknown Artist"), minus
/// the cache/fetch steps, since nothing is cached yet at initial load.
pub(crate) fn initial_group_artist_sort_key(item: &mbv_core::api::EmbyItem) -> String {
    let artist = if !item.artist.is_empty() {
        item.artist.clone()
    } else if let Some((artist, _, _)) = parse_album_folder_name(&item.name) {
        artist
    } else {
        "Unknown Artist".to_string()
    };
    natural_sort_key(strip_article(&artist))
}

/// Returns the effective sort key for an item: `sort_name` when Emby provides it,
/// otherwise the item's display name with any leading article stripped.
pub(crate) fn effective_sort_str(item: &mbv_core::api::EmbyItem) -> &str {
    if !item.sort_name.is_empty() {
        &item.sort_name
    } else {
        strip_article(&item.name)
    }
}

/// Returns the letter-group bucket label for `item` given `total` items in the list.
/// Uses `sort_name` when available (so "The Wire" → 'W'), otherwise the article-stripped
/// name. "#" for titles starting with a digit or non-letter; ranges for 50–999 items;
/// individual letters for 250+ items.
pub(crate) fn letter_bucket(item: &mbv_core::api::EmbyItem, total: usize) -> String {
    let key = effective_sort_str(item);
    let first = key
        .chars()
        .next()
        .map(|c| c.to_ascii_uppercase())
        .unwrap_or('\0');
    // KNOWN LIMITATION: any non-ASCII-alphabetic first character (accented
    // letters like "Æon"/"Élan" included, codepoint > 'Z') buckets here as
    // "#". But the "#" *pill*'s Emby fetch bounds are `NameLessThan("A")`
    // -- only titles that SORT BEFORE "A" -- so an accented title with a
    // codepoint after 'Z' is actually fetched by the `V–Z` pill
    // (`name_ge = "V"`, no upper bound) yet renders under this "#" header,
    // making it unreachable from the "#" pill's scoped fetch. Fixing this
    // would mean either teaching the "#" pill to also request `V–Z`-range
    // items with a non-ASCII-alphabetic first char (an Emby-side filter
    // that doesn't exist), or bucketing accented letters under their
    // unaccented equivalent instead of "#" (a bigger behavior change than
    // this pass intends). Left as-is; flagged for a follow-up.
    if !first.is_ascii_alphabetic() {
        return "#".to_string();
    }
    if total >= 250 {
        return first.to_string();
    }
    match first {
        'A'..='C' => "A\u{2013}C",
        'D'..='F' => "D\u{2013}F",
        'G'..='I' => "G\u{2013}I",
        'J'..='L' => "J\u{2013}L",
        'M'..='O' => "M\u{2013}O",
        'P'..='R' => "P\u{2013}R",
        'S'..='U' => "S\u{2013}U",
        _ => "V\u{2013}Z",
    }
    .to_string()
}

/// Library size above which the library list shows the
/// letter-range pill row (see `LetterFilter`), scoping the server fetch to
/// one range at a time. Unrelated to the 50-item in-list header threshold
/// used by `use_letter_groups` in `list.rs`.
pub(crate) const LIBRARY_PILL_THRESHOLD: usize = 300;

/// The letter-range pill buckets, in display order. Single source of truth
/// for both the pill labels and the Emby `NameStartsWithOrGreater` /
/// `NameLessThan` fetch bounds, so they can't drift apart. Mirrors the range
/// boundaries used by `letter_bucket` above.
///
/// KNOWN LIMITATION (see `letter_bucket`'s doc comment): the `"#"` pill's
/// bounds (`NameLessThan("A")`) only reach titles that sort *before* "A".
/// An accented title whose SortName starts with a codepoint after 'Z'
/// (e.g. "Æon Flux") is fetched by the `V–Z` pill but rendered under a
/// `"#"` in-list header, and so is unreachable from the `"#"` pill itself.
const LETTER_FILTER_BUCKETS: &[(&str, Option<&str>, Option<&str>)] = &[
    ("A\u{2013}C", Some("A"), Some("D")),
    ("D\u{2013}F", Some("D"), Some("G")),
    ("G\u{2013}I", Some("G"), Some("J")),
    ("J\u{2013}L", Some("J"), Some("M")),
    ("M\u{2013}O", Some("M"), Some("P")),
    ("P\u{2013}R", Some("P"), Some("S")),
    ("S\u{2013}U", Some("S"), Some("V")),
    ("V\u{2013}Z", Some("V"), None),
    ("#", None, Some("A")),
];

/// A selected letter-range pill: which bucket, its display label, and the
/// Emby name-range bounds to fetch. Constructed only via `for_index`/`default`
/// so it always matches a row in `LETTER_FILTER_BUCKETS`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LetterFilter {
    pub index: usize,
    pub label: &'static str,
    pub name_ge: Option<&'static str>,
    pub name_lt: Option<&'static str>,
}

impl LetterFilter {
    /// Number of pill buckets (`A–C` … `V–Z`, `#`).
    pub(crate) fn count() -> usize {
        LETTER_FILTER_BUCKETS.len()
    }

    /// Builds the `LetterFilter` for bucket `index`, or `None` if out of range.
    pub(crate) fn for_index(index: usize) -> Option<Self> {
        LETTER_FILTER_BUCKETS
            .get(index)
            .map(|&(label, name_ge, name_lt)| LetterFilter {
                index,
                label,
                name_ge,
                name_lt,
            })
    }

    /// The default pill selected when a large library is first opened: the
    /// first range, `A–C`.
    pub(crate) fn default_filter() -> Self {
        Self::for_index(0).expect("LETTER_FILTER_BUCKETS is non-empty")
    }

    /// All pill labels in bucket order, for building a `PillBar`.
    pub(crate) fn labels() -> Vec<String> {
        LETTER_FILTER_BUCKETS
            .iter()
            .map(|&(label, _, _)| label.to_string())
            .collect()
    }
}
