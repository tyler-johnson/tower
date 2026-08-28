//! `ff tower requeue <flight>` — hand the flight back to the pool.
//!
//! `take`'s reverse, and the recovery path an agent loop needs: a claim
//! nothing ever takes back keeps its flight out of the pool permanently,
//! so an agent that dies mid-flight would strand it. The requeue clears
//! the claim and the take together, which is what makes the pair exact
//! inverses — a flight you took goes back to the agent pool, and one an
//! agent merely claimed goes back untouched.
//!
//! A flight with an open question requeues fine: `answer` does not clear
//! a claim, so forcing an answer-then-requeue ordering would buy nothing.
//! The question stands and keeps the flight out of the pool until
//! answered, which is correct. A flight fufu holds requeues too — that is
//! a branch verdict, not tower's, and the pool reads it separately.

use crate::error::CliError;
use crate::{machine, render};
use ff_tower_core::board;
use ff_tower_core::log::Kind;

pub fn run(json: bool, flight: &str) -> Result<(), CliError> {
    super::parse_ref(flight)?;

    let store = super::store()?;
    let fold = board::fold(&store.read_all()?);
    let flight = super::resolve(&fold, flight)?;
    let filed = super::ensure_active(&fold, &flight)?;
    if filed.claim.is_none() && filed.taken.is_none() {
        return Err(CliError::coded(
            "requeue/unclaimed",
            format!(
                "`{}` is not claimed — nothing to hand back",
                super::display(&fold, &flight)
            ),
            Vec::new(),
        ));
    }
    let subject = filed.subject.clone();

    let ids = store.append(vec![Kind::Requeued {
        flight: flight.clone(),
    }])?;
    let id = ids.into_iter().next().expect("one requeued event");

    if json {
        let event = super::appended(&store, &id)?;
        println!(
            "{}",
            machine::emit("requeue", &serde_json::json!({ "requeued": event }))
        );
    } else {
        let colored = render::colored();
        println!(
            "requeued {}: {subject}",
            render::paint_id(&super::display(&fold, &flight), colored)
        );
        println!("{}", super::tail(colored));
    }
    Ok(())
}
