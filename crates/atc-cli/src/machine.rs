//! The machine envelope, emitted from core so the CLI and the server
//! cannot drift apart. What stays here is the binary's half of the error
//! form: `CliError` is this crate's type, and so is the prose registry
//! that answers when a raise site carried no exits of its own.

pub use atc_core::machine::emit;

use crate::error::CliError;

/// The error form: `error` replaces `data`. One line for stdout even on
/// failure, so a `--json` caller always has an envelope to parse. The id
/// goes out bare — tower's own, or fufu's when the refusal was fufu's.
pub fn emit_error(cmd: &str, err: &CliError) -> String {
    atc_core::machine::emit_error(
        cmd,
        err.id(),
        &err.to_string(),
        &crate::explain::exits_for(err),
    )
}

/// False when `ATC_NONINTERACTIVE` is set to a non-empty value, or when
/// stdin is not a terminal. Nothing may prompt when this is false.
/// `var_os` rather than `var`: a non-UTF-8 value is still a value, and the
/// rest of the tool reads its environment the same way.
pub fn interactive() -> bool {
    let forced_off = std::env::var_os("ATC_NONINTERACTIVE").is_some_and(|v| !v.is_empty());
    !forced_off && std::io::IsTerminal::is_terminal(&std::io::stdin())
}

/// `[Y/n]` on stdin. No new dependency, and no selector: this is one
/// question with a default, and a TUI for it would be a TUI to maintain.
/// Callers gate on [`interactive`] first — nothing may prompt when it is false.
pub fn confirm(question: &str) -> Result<bool, CliError> {
    use std::io::Write;
    print!("\n{question} [Y/n] ");
    std::io::stdout()
        .flush()
        .map_err(|err| CliError::coded("hook/failed", format!("stdout: {err}"), vec![]))?;
    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return Ok(false);
    }
    let answer = answer.trim().to_ascii_lowercase();
    Ok(answer.is_empty() || answer == "y" || answer == "yes")
}
