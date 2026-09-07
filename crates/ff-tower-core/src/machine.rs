//! tower's own machine envelope, which is fufu's `machine` shape:
//! `{"ff": <contract>, "cmd": "tower <verb>", …}` with either `data` or
//! `error` and never both — principle 9, one model on every surface.
//! Both forms are struct-serialized here so field order stays
//! `ff, cmd, payload` and the two cannot drift apart. Every emitting
//! surface — the CLI's stdout, the server's responses — routes through
//! these functions, which is what makes byte parity between them a
//! property rather than a test's good luck.
//!
//! tower is a served extension, so the envelope is fufu's: the key, the
//! `tower <verb>` spelling of `cmd`, and the `tower/` namespace on every
//! id tower prints. The callers pass a bare verb and a bare id and the
//! spelling is applied here, so no call site can drift out of the
//! contract on its own.

use serde::Serialize;

/// The name the extension answers to: the `cmd` prefix, the id
/// namespace, and what the manifest declares itself as.
pub const NAME: &str = "tower";

/// The JSON contract tower emits, which is the one tower reads. tower
/// prints fufu's envelope now, so the number it prints and the number it
/// checks fufu's payloads against are the same number — two independent
/// `1`s could only ever disagree by mistake.
pub const CONTRACT: u32 = crate::ff::CONTRACT;

#[derive(Serialize)]
struct Envelope<'a, T> {
    ff: u32,
    cmd: &'a str,
    data: &'a T,
}

#[derive(Serialize)]
struct ErrorEnvelope<'a> {
    ff: u32,
    cmd: &'a str,
    error: ErrorBody<'a>,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    id: &'a str,
    message: &'a str,
    exits: &'a [String],
}

/// A bare id as the wire spells it: `tower/<id>`. The registry, the
/// exit-code derivation and the status table all read the bare id, so
/// the namespace goes on at the surfaces a person or an agent reads —
/// the envelope here, and `explain`'s own render and catalog.
pub fn namespaced(id: &str) -> String {
    format!("{NAME}/{id}")
}

/// One envelope, as a line of JSON.
pub fn emit<T: Serialize>(cmd: &str, data: &T) -> String {
    serde_json::to_string(&Envelope {
        ff: CONTRACT,
        cmd: &qualified(cmd),
        data,
    })
    .expect("the board serializes")
}

/// The error form: `error` replaces `data`. One line even on failure, so
/// a machine caller always has an envelope to parse. The parts arrive
/// already resolved — id, message, exits — because which error carries
/// which id is each caller's own table, not this module's.
///
/// The id is namespaced here — `tower/<id>` is what the wire carries,
/// and the registry, the exit-code derivation and the status table all
/// go on reading the bare id.
pub fn emit_error(cmd: &str, id: &str, message: &str, exits: &[String]) -> String {
    error_line(cmd, &namespaced(id), message, exits)
}

/// A refusal fufu shaped itself: its id is fufu's and passes through
/// unnamespaced, because `ff explain` routes it back to fufu's registry
/// and tower has no entry for it.
pub fn emit_forwarded(cmd: &str, id: &str, message: &str, exits: &[String]) -> String {
    error_line(cmd, id, message, exits)
}

/// The one place an error envelope is serialized, so the two spellings
/// of the id cannot become two shapes of envelope.
fn error_line(cmd: &str, id: &str, message: &str, exits: &[String]) -> String {
    serde_json::to_string(&ErrorEnvelope {
        ff: CONTRACT,
        cmd: &qualified(cmd),
        error: ErrorBody { id, message, exits },
    })
    .expect("the error serializes")
}

/// The verb as fufu names it: `tower board`, `tower bay list`. Every
/// call site passes the bare verb, so the extension's name is written
/// once.
fn qualified(cmd: &str) -> String {
    format!("{NAME} {cmd}")
}
