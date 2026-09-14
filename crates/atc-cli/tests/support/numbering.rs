//! Numbering-aware fixture helpers.

use atc_core::{board, log::Store};
use std::path::Path;

/// Read the IDs actually emitted by the fixture rather than predicting event sequence offsets.
pub fn flight(repo: &Path, ordinal: u64) -> String {
    let store = Store::open(repo).expect("store");
    let fold = board::fold(&store.read_all().expect("events"));
    board::resolve(&fold, &format!("pi#{ordinal}"))
        .expect("filed alias")
        .to_string()
}

pub fn flights(repo: &Path, ordinals: &[u64]) -> Vec<String> {
    ordinals.iter().map(|n| flight(repo, *n)).collect()
}

pub fn event(repo: &Path, kind: &str, index: usize) -> String {
    Store::open(repo)
        .expect("store")
        .read_all()
        .expect("events")
        .iter()
        .filter(|event| event.kind.name() == kind)
        .nth(index)
        .expect("emitted event")
        .id
        .to_string()
}

/// Gesture assertions focus on authored changes; numbering has its own protocol tests.
pub fn gestures(brief: &serde_json::Value) -> Vec<serde_json::Value> {
    brief["history"]
        .as_array()
        .expect("history")
        .iter()
        .filter(|event| event["what"] != "numbered")
        .cloned()
        .collect()
}
