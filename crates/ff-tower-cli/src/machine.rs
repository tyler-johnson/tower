//! The machine envelope, emitted from core so the CLI and the server
//! cannot drift apart. What stays here is the binary's half of the error
//! form: `CliError` is this crate's type, and so is the prose registry
//! that answers when a raise site carried no exits of its own.

pub use ff_tower_core::machine::{emit, namespaced};

use crate::error::CliError;

/// The error form: `error` replaces `data`. One line for stdout even on
/// failure, so a `--json` caller always has an envelope to parse.
///
/// tower's own ids go out namespaced `tower/<id>`; a refusal fufu shaped
/// itself keeps fufu's id verbatim, because `ff explain` routes that one
/// back to fufu's registry.
pub fn emit_error(cmd: &str, err: &CliError) -> String {
    let emit = if err.forwarded() {
        ff_tower_core::machine::emit_forwarded
    } else {
        ff_tower_core::machine::emit_error
    };
    emit(
        cmd,
        err.id(),
        &err.to_string(),
        &crate::explain::exits_for(err),
    )
}
