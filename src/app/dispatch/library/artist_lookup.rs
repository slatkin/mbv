//! The App half of the artist-name lookup (`decouple-client-core` design D4a
//! (b)): `ArtistNameLookup`. A folder-based Music library groups its artist
//! rows by name and holds no Service artist id for them, so the id found for a
//! name feeds the row's image source only. This module keeps the answer: the
//! track-named id map and the search state, and the write each library event
//! makes into it. The GUI Library owns the search itself and the rows it
//! unblocks (an in-flight search, its dedupe, and its row fetches), so only
//! this half moves here.

use crate::app::state::client_capabilities::ClientCapability;
use crate::app::{App, LibEvent, MusicEvent};

/// Candidates one artist search asks the Service for.
pub(in crate::app) const ARTIST_LOOKUP_LIMIT: usize = 10;

/// What a name's artist search knows while it has no resolved id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum ArtistNameLookupEntry {
    /// The search is in flight.
    Loading,
    /// The Service answered and no artist name matched. Final.
    Unmatched,
}

/// A name with its case and punctuation removed and its leading article
/// dropped, so `AC/DC` and `ac dc`, or `The Smiths` and `Smiths`, meet.
fn normalized(name: &str) -> String {
    let words: Vec<String> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect();
    match words.split_first() {
        Some((first, rest)) if first == "the" && !rest.is_empty() => rest.join(" "),
        _ => words.join(" "),
    }
}

/// The App's record of the Service artist item id known for an artist name,
/// and what a name's artist search knows while it has no resolved id.
#[derive(Debug, Default)]
pub struct ArtistNameLookup;

impl ArtistNameLookup {
    /// The Service artist item id known for an artist name, if any.
    pub(in crate::app) fn item_id<'a>(app: &'a App, name: &str) -> Option<&'a str> {
        app.artist_name_item_ids
            .get(&normalized(name))
            .map(String::as_str)
    }

    /// Record the artist item id a track named for `name`. A name already known
    /// keeps its first id.
    pub(in crate::app) fn record(app: &mut App, name: &str, id: String) {
        app.artist_name_item_ids
            .entry(normalized(name))
            .or_insert(id);
    }

    /// Record one artist search's answer (design D4a (b)). The answer is gated
    /// on the Client that presents the search: a Client with no
    /// `ShownQueueEntries` capability never starts one, so its answer is
    /// dropped rather than written. No answer (the request failed) forgets the
    /// name so a later pass retries: no answer is not no image (invariant 27).
    /// A name the search matched takes its id; a name it did not match stays
    /// terminal. The record is all this half does: the event arm that calls
    /// it re-queues the rows the answer unblocks, through the Client's stores.
    pub(in crate::app) fn searched(
        app: &mut App,
        name: &str,
        candidates: Option<&[(String, String)]>,
    ) {
        if !app
            .client_capabilities
            .has(ClientCapability::ShownQueueEntries)
        {
            return;
        }
        let key = normalized(name);
        let Some(candidates) = candidates else {
            app.artist_name_lookups.remove(&key);
            return;
        };
        match best_artist_match(name, candidates) {
            Some(id) => {
                let id = id.to_owned();
                app.artist_name_lookups.remove(&key);
                Self::record(app, name, id);
            }
            None => {
                app.artist_name_lookups
                    .insert(key, ArtistNameLookupEntry::Unmatched);
            }
        }
    }

    /// Start the artist search for `name` once: a name with an id, or already
    /// searched or in flight, starts nothing. Only a Client that presents the
    /// search asks, so a Client without `ShownQueueEntries` starts none.
    pub(in crate::app) fn request(app: &mut App, name: &str) {
        if !app
            .client_capabilities
            .has(ClientCapability::ShownQueueEntries)
        {
            return;
        }
        let key = normalized(name);
        if key.is_empty()
            || app.artist_name_item_ids.contains_key(&key)
            || app.artist_name_lookups.contains_key(&key)
        {
            return;
        }
        let Some(client) = app.emby_client() else {
            return;
        };
        let (server_url, token, agent) = {
            let c = client.lock().unwrap();
            (c.config.server_url.clone(), c.token.clone(), c.http_agent())
        };
        app.artist_name_lookups
            .insert(key, ArtistNameLookupEntry::Loading);
        let tx = app.channels.lib_tx.clone();
        let name = name.to_owned();
        std::thread::spawn(move || {
            let url = format!(
                "{server_url}/Artists?SearchTerm={}&Limit={ARTIST_LOOKUP_LIMIT}&api_key={token}",
                mbv_net::encode_path_segment(&name)
            );
            let candidates = agent
                .get(&url)
                .call()
                .ok()
                .and_then(|response| response.into_body().read_json::<serde_json::Value>().ok())
                .map(|body| artist_pairs(&body));
            let _ = tx.send(LibEvent::Music(MusicEvent::ArtistNameSearched {
                name,
                candidates,
            }));
        });
    }
}

/// The id of the candidate `(name, id)` that best matches `name`: an equal
/// normalized name first, else the best score of a match both ways, so a folder
/// `Yes` never takes `Yes Sir`.
fn best_artist_match<'a>(name: &str, candidates: &'a [(String, String)]) -> Option<&'a str> {
    let wanted = normalized(name);
    if let Some((_, id)) = candidates
        .iter()
        .find(|(candidate, _)| normalized(candidate) == wanted)
    {
        return Some(id);
    }
    let matcher = mbv_text::fuzzy_match::WordMatcher::new();
    candidates
        .iter()
        .filter_map(|(candidate, id)| {
            let forward = mbv_text::fuzzy_match::word_match_score(&matcher, candidate, name)?;
            let backward = mbv_text::fuzzy_match::word_match_score(&matcher, name, candidate)?;
            Some((forward + backward, id.as_str()))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, id)| id)
}

/// The `(name, id)` artists of one `/Artists` reply.
fn artist_pairs(body: &serde_json::Value) -> Vec<(String, String)> {
    let items = body["Items"].as_array().map_or(&[][..], Vec::as_slice);
    items
        .iter()
        .filter_map(|item| Some((item["Name"].as_str()?, item["Id"].as_str()?)))
        .map(|(name, id)| (name.to_owned(), id.to_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(name, id)| ((*name).to_owned(), (*id).to_owned()))
            .collect()
    }

    /// Contract: an equal name after case, punctuation, and article folding
    /// wins over a fuzzier candidate listed first.
    #[test]
    fn an_equal_normalized_name_wins() {
        let found = candidates(&[("Smiths Tribute Band", "1"), ("Smiths", "2")]);

        assert_eq!(best_artist_match("The Smiths", &found), Some("2"));
    }

    /// Contract: a fuzzy match must hold both ways, so a short folder name never
    /// takes a longer artist that merely contains it as a prefix word.
    #[test]
    fn a_name_that_matches_one_way_only_resolves_nothing() {
        let found = candidates(&[("Yes Sir", "1")]);

        assert_eq!(best_artist_match("Yes", &found), None);
    }
}
