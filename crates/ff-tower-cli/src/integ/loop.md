---
description: drain the board unattended — claim, work, hold or finish, repeat until it stops
---

# loop

You are the crew of a loop over `ff tower next`. The harness you run in is the scheduler; tower is the queue and the record. Each pass claims one flight, works it in its own tree, and ends it with a verb that says what happened.

The loop step is `ff tower next --json`. Exit 0 is a pick: `picked[0]` carries `flight`, `subject`, `branch`, `bay`, and — when the flight's part names one — `skill`. Exit 1 is a drained board: stop and report. Exit 3 is work that exists and needs a person: stop and report. Those are the only exits. Never sleep and retry, never add a timeout, never invent a sentinel.

Fan-out: when the harness can run parallel subagents, `ff tower next -n <k>` claims a set that collides with neither each other nor anything already flying. Hand each picked row to one worker in its own bay, and rejoin the loop when all of them have ended their flight with a verb. Solo remains the default; fan out only when the board shows independent ready flights and the harness genuinely runs workers concurrently.

Bay discipline: cd into the picked `bay` and work only there, on the picked `branch`. Never touch the main worktree or another bay. A pick without a bay stops the step — report the claim and the missing tree rather than improvising one.

Read the brief before touching anything: `ff tower brief <flight> --json` — the body, the comments, the links and their done states, and the open question when one stands. The brief holds this flight's facts: the files it touches, the prior art, the verify command. This command knows how to drive tower; the brief knows the flight.

When the pick carries a `skill`, run `ff tower skills <name>` and follow that markdown for this flight in place of the work step below, rejoining at the hold rule. The user never typed that name; the flight carried it.

Do the work in the bay, and commit with `ff commit` as coherent pieces land. Run the brief's verify command before calling anything done; when the brief names none, run the checks the change plainly touches.

Questions are holds, never guesses. Nobody is here to ask: when the brief does not settle a decision, `ff tower hold <flight> -m "<question>"` and continue the loop. A held flight is parked with its question on the record, not the run's end.

Finish or give back. Verified done: `ff tower done <flight>`. Unworkable with no question worth holding on: `ff tower requeue <flight>` and continue the loop, carrying the reason into the final report.

The push boundary: stop at committed on the branch. No push, no PR, no forge or tracker write. Wanting this loop to publish — or to behave differently in any other way — means writing your own command, using this one as reference.

End the run with a report: which exit ended it — drained, or needs-you — and the flights worked, held with their questions, and requeued with their reasons.
