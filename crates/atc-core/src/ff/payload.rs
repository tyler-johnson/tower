//! The payload tower reads, mirrored as tower's own type.
//!
//! Deliberately *not* fufu's struct. tower holds no fufu type — that is
//! the seam's whole point — so `data` is parsed into a shape declared
//! here, from the same JSON any other extension gets.
//!
//! Two rules keep the mirror from becoming a maintenance tax:
//!
//! **A field appears here when a tower caller reads it, and not before.**
//! `ff version --json` carries more than this — `commit`, `date`,
//! `update` — and mirroring them ahead of a reader would be signing up to
//! chase refactors in fields nothing consults.
//!
//! **Unknown fields are ignored.** fufu adds to its payloads between
//! releases and the contract version does not move for it. A tower that
//! failed a seam check because fufu grew a field would be broken by an
//! upgrade that broke nothing.

use serde::Deserialize;

/// `ff version --json` — what is actually installed. Repo-independent,
/// and the doctor's drift check: the call itself is what surfaces an
/// [`Error::Contract`](super::Error::Contract) when the `ff` on PATH and
/// tower have moved apart.
#[derive(Debug, Clone, Deserialize)]
pub struct Version {
    pub version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_payload_parses_and_ignores_unknown_fields() {
        let version: Version = serde_json::from_str(
            r#"{"version":"0.9.0","commit":"ae91532","date":"2026-08-27","update":{"status":"unofficial","latest":null}}"#,
        )
        .expect("parse");
        assert_eq!(version.version, "0.9.0");
    }
}
