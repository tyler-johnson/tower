# Shared board implementation protocol

This records the shared-board transaction rules implemented for flight #139.

## Coordination and lock order

All counter allocation and board synchronization in one repository takes an advisory lock on `<common Git directory>/tower/coordination`. Linked worktrees and different configured writers share this file. The operating system releases the lock on process exit, including a crash. Keep the file in place: removing a locked file would let another process lock a different inode at the same path.

The only allowed nested order is coordination lock, then writer append lock. Appending ordinary events never takes the coordination lock. Release the append lock before any network operation. Coordination lock acquisition consumes the caller's existing deadline rather than starting another timeout. A reservation that has moved the counter must be recorded or left recoverable even if the caller's deadline expires.

## Counter format and lineage

`refs/tower/seq` points to a single-parent commit chain. Each commit has tower's fixed author and committer, a decimal counter value on the first message line, and `tower-counter: 1`. The root has value zero and a unique nonce. Its object ID is the numbering domain's durable identity. Every successor increases the counter by its reservation's length. A reservation contains the flight wire IDs in allocation order in `reservation.json`; its first number is its parent's value plus one. The reservation is coordination/recovery metadata. The authoritative assignment remains a `numbered` event on the reserving writer's log.

Validate the entire counter ancestry before accepting a fetched tip: one parent except at the root, monotonic values, a consistent format and identity, no repeated flight IDs within a reservation, and a range length matching its payload. Never derive lineage from a remote's name or URL. Renaming a remote or reconnecting to the same root does not create a new numbering domain.

## Reservation and recovery

Each new `numbered` event also carries the reservation commit ID. Recovery deduplicates by this identity, flight, and number, so an equal number from another domain cannot suppress the new record and an already recorded reservation cannot overwrite a later claim. Older numbered events without a reservation field remain readable.

A replacement claim's timestamp is clamped past the claims it explicitly supersedes, including another writer's faster clock during migration or enrollment. The union still uses the timestamp and writer tiebreak; no clock is derived from the sequence counter.

Write a reservation commit before attempting the counter update. Root its object with a pending ref under `refs/tower/pending/` before moving the counter or sending the remote lease push. Pending refs are machine-local and never pushed. Each pending reservation records exact flight IDs and numbers, so recovery never repeats a filing.

A local reservation takes effect when its counter ref update succeeds. Recovery checks that the pending commit is on the current counter ancestry, appends only assignments not already present, and removes the pending ref after the log append succeeds. A crash after recording but before removing the pending ref is idempotent. A crash before the counter update can leave an uncommitted pending reservation, which can be discarded once the counter state is known. A reserved range is never reused.

A remote reservation takes effect when an explicit expected-tip lease push succeeds, or when a subsequent fetch proves the reservation is an ancestor of the remote counter. A transport timeout alone proves neither success nor failure. Keep uncertain reservations rooted until a later fetch can decide. Never print an unconfirmed reservation as claimed. A confirmed claim remains claimed if publishing the writer log fails.

## Enrollment and publication

`<common>/tower/sync.json` stores the endpoint last attempted, cadence timestamp, last result, persistent refusal, enrolled counter root, enrollment target and flight list, and the endpoint owning any uncertain pending reservation. Confirmation authorizes that target root durably. Recovery compares standing assignments with reservations on the remote ancestry before allocating unfinished enrollment work. A pending reservation cannot be recovered as local work after changing remote configuration; reconnect to its original endpoint first. Leaving a previously shared board marks its local domain detached so reconnecting gates any locally allocated numbers that the remote does not know.

Fetch into a staging namespace. Validate before fast-forwarding canonical writer refs; never replace a locally advanced or divergent chain. Publish only this writer's log and explicit counter updates. Inspect the remote counter and complete enrollment before publishing local logs.

An empty remote adopts the local counter root. An established remote with a different root requires confirmation before local claims move. Persist the target root and the exact flight IDs requiring replacement before reserving their range. Persist completed enrollment against the counter root, not the transport endpoint. Resume interrupted enrollment using pending reservations and the standing numbered events rather than reserving a second range for completed work.

## Deadlines and references

Ordinary synchronization has one total deadline, three seconds by default. Filing and decomposition have one total allocation deadline, ten seconds by default, independent of cadence. Lock waiting, fetch, contention retries, push, and owned transport cleanup all consume that budget. No transport continues detached after return. Failed ordinary attempts also update the cadence stamp; the default interval is thirty seconds.

Resolve command arguments and prose against a local snapshot before synchronizing. Carry wire IDs through the operation. Tilde references resolve only to flights provisional in that snapshot; historical guesses are not aliases. The pure fold receives the locally observed counter as explicit context and never performs I/O.
