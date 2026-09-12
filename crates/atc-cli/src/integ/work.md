---
name: work
description: claim, do, hold or commit, repeat — the loop that pairs with `atc next`
---

# work

You are the crew of a loop over `atc next`. The harness you run in is the scheduler; tower is the queue and the record. Each pass claims one flight, works it in its own tree, and ends it with a verb that says what happened.

The loop step is `atc next agent --json` — the shared pool, then the unassigned lane. Exit 0 is a pick: `picked[0]` carries `flight`, `subject`, and — when the flight's part names one — `skill`. Exit 1 is an empty pick, and `data.outcome` says which: `drained` is a board with nothing left, and `elsewhere` is work that exists in another lane — someone's own queue. Both stop the loop and are reported by their word. Those are the only exits. Never sleep and retry, never add a timeout, never invent a sentinel.

Fan-out: when the harness can run parallel subagents, `atc next agent -n 3` claims the next three in filed order. Hand each picked row to one worker in its own worktree, and rejoin the loop when all of them have ended their flight with a verb. Solo remains the default; fan out only when the board shows independent ready flights and the harness genuinely runs workers concurrently.

`next` hands out no tree, so choose a worktree of your own before touching anything, and never one another flight is using.

Read the brief before touching anything: `atc brief <flight> --json` — the body, the comments, the links and their done states, and the open question when one stands. The brief holds this flight's facts: the files it touches, the prior art, the verify command. This skill knows how to drive tower; the brief knows the flight.

When the pick carries a `skill`, run `atc skills <name>` and follow that markdown for this flight in place of the work step below, rejoining at the hold rule. The user never typed that name; the flight carried it.

Do the work in your worktree, and commit with `ff commit` as coherent pieces land. Run the brief's verify command before calling anything done; when the brief names none, run the checks the change plainly touches.

Questions are holds, never guesses. Nobody is here to ask: when the brief does not settle a decision, `atc hold <flight> -m "<question>"` and continue the loop. A held flight is parked with its question on the record, not the run's end.

Finish or give back. Verified done: `atc done <flight>`. Unworkable with no question worth holding on: `atc status <flight> ready` — the record decides between Ready and Waiting — and continue the loop, carrying the reason into the final report.

The push boundary: stop at committed on the branch. No push, no PR, no forge or tracker write. Wanting this loop to publish — or to behave differently in any other way — means editing this file — fork it to `.tower/skills/work.md` or `~/.config/tower/skills/work.md` — and the edit is visibly the operator's: `atc skills` names the layer every skill came from.

End the run with a report: which outcome ended it — `drained` or `elsewhere` — and the flights worked, held with their questions, and handed back with their reasons.
