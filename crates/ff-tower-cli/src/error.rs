//! What the binary can fail with. Both halves already say what went wrong
//! in their own words, so this wraps transparently and adds nothing; main
//! prefixes `ff-tower:` and exits 1.

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error(transparent)]
    Log(#[from] ff_tower_core::log::Error),
    #[error(transparent)]
    Ff(#[from] ff_tower_core::ff::Error),
}
