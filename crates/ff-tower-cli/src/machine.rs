//! tower's own machine envelope, mirroring fufu's `machine` shape:
//! `{"tower": <version>, "cmd": <verb>, …}` with either `data` or `error`
//! and never both — principle 9, one model on every surface. Both forms
//! are struct-serialized here so field order stays `tower, cmd, payload`
//! and the two cannot drift apart.

use serde::Serialize;

use crate::error::CliError;

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
    message: String,
    exits: Vec<String>,
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

/// The error form: `error` replaces `data`. One line for stdout even on
/// failure, so a `--json` caller always has an envelope to parse.
pub fn emit_error(cmd: &str, err: &CliError) -> String {
    serde_json::to_string(&ErrorEnvelope {
        tower: CONTRACT,
        cmd,
        error: ErrorBody {
            id: err.id(),
            message: err.to_string(),
            exits: crate::explain::exits_for(err),
        },
    })
    .expect("the error serializes")
}
