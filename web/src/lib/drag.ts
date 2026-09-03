// The kanban's drag, as three pure functions: which columns a grouping
// stands up before the wire's, whether a card can be picked up, and what
// one drop writes. A drag is a verb or it is not offered — DESIGN's
// rule — so `drop` answers the verb and the body a route takes, or null,
// and the board offers nothing where it answers null.
//
// The field-to-verb mapping is `write.ts`'s, shared with the flight
// page's rail; what stays here is the drag's own — the label set
// arithmetic, and the from/to rules. The guards the routes hold are
// mirrored here where a drop would otherwise be offered and refused every
// time: `waiting` and `held` are derived and take no write, a closed
// flight refuses `status` and `assign`, and a flight with an open question
// moves only to done or canceled.
//
// No runes, so it tests under vitest with no shims.

import type { Field } from "./query";
import { closedRow, type FlightView } from "./tower";
import { write, type Write } from "./write";

/// The verb one drop writes, and the body the route takes: the shared
/// table's, and the two closing verbs, which take a route of their own
/// rather than a field's value.
export type Drop = Write | { verb: "done" | "cancel"; body: { flight: string } };

const STATUSES = ["triage", "waiting", "ready", "in_progress", "held", "done", "canceled"];
const PRIORITIES = ["urgent", "high", "medium", "low", "none"];
const LANES = ["me", "agent", null];

/// The columns a grouping stands up before the wire's, so a drop target
/// exists with nothing in it: status its seven words, priority its five,
/// assignee the two lanes and none. Label, skill and bay seed nothing —
/// their values are whatever the rows named.
export function seeded(field: Field | null): (string | null)[] {
  switch (field) {
    case "status":
      return STATUSES;
    case "priority":
      return PRIORITIES;
    case "assignee":
      return LANES;
    default:
      return [];
  }
}

/// Whether a card can be picked up under this grouping: a groupable
/// field, and for status and assignee a live flight, since a closed one
/// refuses both; edit takes a closed flight.
export function draggable(field: Field | null, view: FlightView): boolean {
  switch (field) {
    case "status":
    case "assignee":
      return !closedRow(view);
    case "priority":
    case "label":
    case "skill":
    case "bay":
      return true;
    default:
      return false;
  }
}

/// What a drop on `to` writes, or null when the column takes none. A
/// drop back on `from` is nothing; every other answer is per field.
export function drop(
  field: Field | null,
  view: FlightView,
  from: string | null,
  to: string | null,
): Drop | null {
  if (from === to || !draggable(field, view)) return null;
  const flight = view.id;
  switch (field) {
    case "status":
      return statusDrop(view, to);
    case "assignee":
    case "priority":
    case "skill":
    case "bay":
      return write(field, flight, to);
    case "label": {
      // The set the flight would carry: `from` out, `to` in, deduped.
      // The same set is no edit at all; an empty one `write` refuses,
      // since an empty array means unchanged on the wire.
      const labels = view.labels.filter((label) => label !== from);
      if (to !== null && !labels.includes(to)) labels.push(to);
      if (sameSet(labels, view.labels)) return null;
      return write("label", flight, labels);
    }
    default:
      return null;
  }
}

/// The status column's verb: `done` and `canceled` are routes of their
/// own, `waiting` and `held` are derived and take no drop, and a flight
/// with an open question drops only on the two closed columns — the
/// route's `status/held` guard, mirrored so the offer is honest.
function statusDrop(view: FlightView, to: string | null): Drop | null {
  const flight = view.id;
  switch (to) {
    case "done":
      return { verb: "done", body: { flight } };
    case "canceled":
      return { verb: "cancel", body: { flight } };
    case "triage":
    case "ready":
    case "in_progress":
      if (view.question !== null) return null;
      return write("status", flight, to);
    default:
      return null;
  }
}

function sameSet(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((word) => b.includes(word));
}
