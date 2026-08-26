//! tower's own machine envelope, mirroring fufu's `machine::emit` shape:
//! `{"tower": <version>, "cmd": <verb>, "data": <payload>}` — principle 9,
//! one model on every surface. No JSON *error* envelope yet; errors go to
//! stderr and exit 1, a scoped omission slice 4 fills deliberately.

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

/// One envelope, as a line of JSON.
pub fn emit<T: Serialize>(cmd: &str, data: &T) -> String {
    serde_json::to_string(&Envelope {
        tower: CONTRACT,
        cmd,
        data,
    })
    .expect("the board serializes")
}
