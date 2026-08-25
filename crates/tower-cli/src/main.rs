//! `tower` — the board, and the verbs around it.
//!
//! Installed under two names. `tower` is the one you type; `ff-tower` is
//! what fufu's `ff-<name>` dispatch finds when someone types `ff tower`.
//! One binary linked twice at install rather than two builds of the same
//! code, and nothing reads `argv[0]` — the two names behave identically on
//! purpose, so neither becomes the real one.

fn main() {
    println!("tower: scaffold only — nothing is built yet. See DESIGN.md.");
}
