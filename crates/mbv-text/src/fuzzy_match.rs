//! The one acceptance rule shared by the library's client-side searches.
//!
//! Scoring and ordering stay `SkimMatcherV2`'s (the inline-search spec's
//! "matching SHALL be fuzzy, scored ..., ordered by descending match score"):
//! this module only decides *what* a fuzzy score is allowed to match.
//!
//! Skim matches a query as a *subsequence* of the whole candidate text, so a
//! short query can be spelled out of letters scattered across several words
//! and surface a row the user never typed. Both real reports are that shape,
//! from the user's music library:
//!
//! - `tang` matched "King Shit and the Golden Boys" (`t` from "Shi**t**",
//!   `a`/`n` from "**an**d", `g` from "**G**olden"),
//! - `devil` matched "The Velvet Underground Live With Lou Reed" (`d` from
//!   "un**d**erground", `e` after it, `v` from "li**v**e", `i` from
//!   "w**i**th", `l` from "**l**ou").
//!
//! So the rule is word-local: **each word of the query must match inside one
//! word of the candidate**, in order, with no letter taken from a different
//! word. Within that one word the match stays skim's fuzzy subsequence, so
//! `tang` finds "**Tang**led", `evil` finds "D**evil**", and `Vu` inside one
//! word still works.
//!
//! What this deliberately gives up: a query word built from the initials of
//! several words (`vu` for "Velvet Underground") no longer matches, because
//! those letters live in two words. Words are the unit; that is the point.

pub use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

/// Scores `text` against `query` by matching every word of `query` inside a
/// single word of `text`, in order, or `None` when that is impossible.
///
/// The score is the sum of the per-word scores, so it stays comparable across
/// candidates for one query (every accepted candidate matched the same words).
/// `matcher` is the caller's own case-insensitive matcher, so every caller
/// keeps one construction site and the same scoring config.
pub fn word_match_score(matcher: &SkimMatcherV2, text: &str, query: &str) -> Option<i64> {
    let mut words = text.split_whitespace();
    let mut matched_a_query_word = false;
    let mut total = 0i64;
    for query_word in query.split_whitespace() {
        matched_a_query_word = true;
        let score = words
            .by_ref()
            .find_map(|word| matcher.fuzzy_match(word, query_word))?;
        total += score;
    }
    matched_a_query_word.then_some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matcher() -> SkimMatcherV2 {
        SkimMatcherV2::default().ignore_case()
    }

    fn score(text: &str, query: &str) -> Option<i64> {
        word_match_score(&matcher(), text, query)
    }

    /// The two reported false positives: one query word spelled out of single
    /// letters taken from different words.
    #[test]
    fn a_query_word_never_spans_two_words() {
        assert_eq!(score("King Shit and the Golden Boys", "tang"), None);
        assert_eq!(
            score("The Velvet Underground Live With Lou Reed", "devil"),
            None
        );
    }

    /// A match inside one word keeps working, including a fuzzy subsequence
    /// that skips letters there.
    #[test]
    fn query_words_match_words_in_order() {
        let album = "The Velvet Underground Live With Lou Reed";
        assert!(score(album, "velvet").is_some());
        assert!(score(album, "lou reed").is_some());
        assert!(score(album, "velvet lou").is_some());
        assert!(score(album, "live with lou").is_some());
        // Out of order: the later word cannot be matched first.
        assert_eq!(score(album, "lou velvet"), None);
        // A word that is not in the candidate ends the match.
        assert_eq!(score(album, "velvet devil"), None);
    }
}
