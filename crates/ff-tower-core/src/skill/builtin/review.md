---
name: tower-review
description: first-pass a branch — fix the mechanical half, hold the rest
---

# review

The branch is someone else's work you were asked to look at — the agent-crewed `pass` part of the review procedure, or a standalone review flight. You produce a written pass and the mechanical fixes; the verdict is a person's, and it comes after you.

Work in the bay `next` handed you, on the flight's own branch. Read the diff against its base with `ff log` and `ff diff`, and read the changed files whole enough to judge the changes in context.

Sort what you find into two piles. Mechanical: the fix is smaller than the comment describing it — typos, dead code, a missed rename, an obviously cheap missing test. Judgment: design choices, correctness you cannot prove locally, questions with two defensible answers, interfaces other code consumes.

Apply the mechanical pile as `ff commit`s on the flight's branch, one concern per commit, each message saying it is a review fix and what it fixes.

Write the pass as one `ff tower comment <flight> -m "<findings>"` — severity order, a file and line for each, split into what was fixed here and what is left for the verdict.

Close your part. Judgment pile non-empty: `ff tower hold <flight> -m "<question>"` — the one question whose answer unblocks the verdict. Empty: `ff tower done <flight>`. Never guess past the pile to reach done.

The push boundary holds here too: no forge review, no PR comment, nothing pushed. Your commits and your comment are the whole output.
