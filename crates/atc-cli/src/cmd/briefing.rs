//! `atc briefing` — one line for fufu's session briefing.
//!
//! fufu runs it in the event's cwd with `FF_REPO`, `FF_CONTRACT`, and
//! `FF_SESSION` set, under a one-second box with stderr discarded, and
//! takes stdout verbatim: trimmed, one line, at most 240 characters, or
//! dropped whole. So the render is the line and nothing else — no color,
//! no `board: atc` tail — and the line is one short line by
//! construction: a count, never a subject.
//! A failure costs nothing: fufu discards it, and the ordinary `report()`
//! path says why to anyone running the verb by hand.
//!
//! The pipeline is the fold alone — no gather, no spawn beyond the
//! store — because the question is the board's: how much is ready.

use crate::error::CliError;
use crate::machine;
use atc_core::board;
use atc_core::log::Store;

pub fn run(json: bool) -> Result<(), CliError> {
    let ff = super::ff()?;
    let store = Store::open(ff.repo())?;
    let fold = board::fold(&store.read_all()?);
    let ready = fold
        .flights
        .iter()
        .filter(|flight| flight.status == "ready")
        .count();
    let line = match ready {
        0 => "tower: nothing ready".to_string(),
        1 => "tower: 1 flight ready — atc".to_string(),
        n => format!("tower: {n} flights ready — atc"),
    };

    if json {
        println!(
            "{}",
            machine::emit("briefing", &serde_json::json!({ "line": line }))
        );
    } else {
        println!("{line}");
    }
    Ok(())
}
