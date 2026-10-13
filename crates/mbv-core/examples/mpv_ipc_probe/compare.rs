//! Design D3: normalization, comparison and the per-scenario report.

use crate::RUNS;
use crate::scenarios::Scenario;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const RULES: [&str; 6] = [
    "1: drop what libmpv2 cannot deliver (playlist_* fields, property changes with no data)",
    "2: map file_error text to an mpv error code",
    "3: collapse consecutive time-pos changes and drop their values",
    "4: drop log-message events",
    "5: drop command replies",
    "6: add the property-change id that mpv's JSON omits when it is 0",
];

/// Applies the rules. Returns the sequence and the rule numbers used.
fn normalize(trace: &[Value]) -> (Vec<Value>, BTreeSet<usize>) {
    let mut out: Vec<Value> = Vec::new();
    let mut rules = BTreeSet::new();
    for event in trace {
        let Some(mut map) = event.as_object().cloned() else {
            continue;
        };
        if !map.contains_key("event") {
            rules.insert(5);
            continue;
        }
        if map["event"] == "log-message" {
            rules.insert(4);
            continue;
        }
        if is_undeliverable(&map) {
            rules.insert(1);
            continue;
        }
        if map["event"] == "property-change" && !map.contains_key("id") {
            rules.insert(6);
            map.insert("id".to_owned(), json!(0));
        }
        if map.contains_key("playlist_entry_id") || map.contains_key("playlist_insert_id") {
            rules.insert(1);
        }
        for field in [
            "playlist_entry_id",
            "playlist_insert_id",
            "playlist_insert_num_entries",
        ] {
            map.remove(field);
        }
        if let Some(text) = map.remove("file_error") {
            rules.insert(2);
            map.insert("error_code".to_owned(), error_code(&text));
        }
        if is_time_pos(&map) {
            rules.insert(3);
            map.remove("data");
            if out
                .last()
                .is_some_and(|last| last.as_object().is_some_and(is_time_pos))
            {
                continue;
            }
        }
        out.push(Value::Object(map));
    }
    (out, rules)
}

/// A property change with no data never reaches a libmpv2 client.
fn is_undeliverable(map: &Map<String, Value>) -> bool {
    map["event"] == "property-change" && !map.contains_key("data")
}

fn is_time_pos(map: &Map<String, Value>) -> bool {
    map.get("event").is_some_and(|e| e == "property-change")
        && map.get("name").is_some_and(|n| n == "time-pos")
}

fn error_code(text: &Value) -> Value {
    (-30..0)
        .find(|code| text.as_str() == Some(libmpv2_sys::mpv_error_str(*code)))
        .map_or_else(|| text.clone(), |code| json!(code))
}

/// The order of the events that are not property changes, and the value
/// sequence of each property. A property change can land before or after a
/// nearby event from run to run, so this view ignores that interleaving.
type Split = (Vec<Value>, BTreeMap<String, Vec<Value>>);

fn split(seq: &[Value]) -> Split {
    let mut events = Vec::new();
    let mut properties: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for event in seq {
        match event["name"].as_str() {
            Some(name) if event["event"] == "property-change" => properties
                .entry(name.to_owned())
                .or_default()
                .push(event["data"].clone()),
            _ => events.push(event.clone()),
        }
    }
    // Rule 3 collapses only adjacent time-pos changes, so interleaving can
    // change how many survive. Repeated equal values carry no information.
    for values in properties.values_mut() {
        values.dedup();
    }
    (events, properties)
}

fn same_sets<T: PartialEq>(a: &[(T, usize)], b: &[(T, usize)]) -> bool {
    a.len() == b.len() && a.iter().all(|(x, _)| b.iter().any(|(y, _)| x == y))
}

/// Distinct values with the number of runs that produced each, most common first.
fn distinct<T: PartialEq>(runs: Vec<T>) -> Vec<(T, usize)> {
    let mut out: Vec<(T, usize)> = Vec::new();
    for run in runs {
        match out.iter_mut().find(|(seq, _)| *seq == run) {
            Some((_, count)) => *count += 1,
            None => out.push((run, 1)),
        }
    }
    out.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    out
}

pub(crate) struct Outcome {
    libmpv: Vec<(Vec<Value>, usize)>,
    ipc: Vec<(Vec<Value>, usize)>,
    rules: BTreeSet<usize>,
}

impl Outcome {
    pub(crate) fn new(libmpv_runs: &[Vec<Value>], ipc_runs: &[Vec<Value>]) -> Self {
        let mut rules = BTreeSet::new();
        let mut side = |runs: &[Vec<Value>]| {
            distinct(
                runs.iter()
                    .map(|run| {
                        let (seq, used) = normalize(run);
                        rules.extend(used);
                        seq
                    })
                    .collect(),
            )
        };
        let libmpv = side(libmpv_runs);
        let ipc = side(ipc_runs);
        Self { libmpv, ipc, rules }
    }

    fn splits(sequences: &[(Vec<Value>, usize)]) -> Vec<(Split, usize)> {
        distinct(
            sequences
                .iter()
                .flat_map(|(seq, count)| vec![split(seq); *count])
                .collect(),
        )
    }

    pub(crate) fn print(&self, scenario: &Scenario) {
        let exact = same_sets(&self.libmpv, &self.ipc);
        let interleaving_only = same_sets(&Self::splits(&self.libmpv), &Self::splits(&self.ipc));
        let verdict = match (exact, interleaving_only, self.rules.is_empty()) {
            (true, _, true) => "MATCH".to_owned(),
            (true, _, false) => format!("MATCH-AFTER-NORMALIZATION (rules {:?})", self.rules),
            (false, true, _) => {
                format!(
                    "MATCH-IGNORING-PROPERTY-INTERLEAVING (rules {:?})",
                    self.rules
                )
            }
            (false, false, _) => format!("DIFF (rules {:?})", self.rules),
        };
        println!("{} {}: {verdict}", scenario.id, scenario.summary);
        if !exact {
            print_first_difference(&self.libmpv[0].0, &self.ipc[0].0);
        }
        print_side("libmpv", &self.libmpv);
        print_side("ipc", &self.ipc);
    }
}

fn print_first_difference(libmpv: &[Value], ipc: &[Value]) {
    let index = libmpv
        .iter()
        .zip(ipc)
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| libmpv.len().min(ipc.len()));
    let show = |seq: &[Value]| seq.get(index).map_or("<end>".to_owned(), Value::to_string);
    println!("  first difference at index {index}");
    println!("    libmpv: {}", show(libmpv));
    println!("    ipc:    {}", show(ipc));
}

fn print_side(label: &str, sequences: &[(Vec<Value>, usize)]) {
    for (number, (seq, count)) in sequences.iter().enumerate() {
        println!("  {label} sequence {} ({count}/{RUNS} runs):", number + 1);
        for event in seq {
            println!("    {event}");
        }
    }
}
