//! tower's own machine envelope, mirroring fufu's `machine` shape:
//! `{"tower": <version>, "cmd": <verb>, …}` with either `data` or `error`
//! and never both — principle 9, one model on every surface. Both forms
//! are struct-serialized here so field order stays `tower, cmd, payload`
//! and the two cannot drift apart. Every emitting surface — the CLI's
//! stdout, the server's responses — routes through these two functions,
//! which is what makes byte parity between them a property rather than a
//! test's good luck.

use serde::Serialize;

/// The JSON contract tower emits, checked by readers the way tower checks
/// fufu's.
pub const CONTRACT: u32 = 1;

#[derive(Serialize)]
struct Envelope<'a, T> {
    tower: u32,
    cmd: &'a str,
    data: &'a T,
}

#[derive(Serialize)]
struct ErrorEnvelope<'a> {
    tower: u32,
    cmd: &'a str,
    error: ErrorBody<'a>,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    id: &'a str,
    message: &'a str,
    exits: &'a [String],
}

/// One envelope, as a line of JSON.
pub fn emit<T: Serialize>(cmd: &str, data: &T) -> String {
    serde_json::to_string(&Envelope {
        tower: CONTRACT,
        cmd,
        data,
    })
    .expect("the board serializes")
}

/// The error form: `error` replaces `data`. One line even on failure, so
/// a machine caller always has an envelope to parse. The parts arrive
/// already resolved — id, message, exits — because which error carries
/// which id is each caller's own table, not this module's.
pub fn emit_error(cmd: &str, id: &str, message: &str, exits: &[String]) -> String {
    serde_json::to_string(&ErrorEnvelope {
        tower: CONTRACT,
        cmd,
        error: ErrorBody { id, message, exits },
    })
    .expect("the error serializes")
}
