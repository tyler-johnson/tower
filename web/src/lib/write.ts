// The write table: one field, one target, one value, and the verb and
// body that land it. The six fields a flight's record edits — status,
// assignee, priority, label, skill, bay — and nothing else, because
// nothing else is a field a person sets.
//
// Shared on purpose. The kanban writes a field by dragging a card into a
// column and the flight page writes the same field from a control on the
// rail; those are two gestures over one table, and a table kept twice
// drifts. `drag.ts` keeps what is the drag's own — the label set
// arithmetic, the from/to rules, which cards pick up — and calls this for
// the mapping.
//
// No runes, so it tests under vitest with no shims.

import type { Field } from "./query";

/// The verb one write takes, and the body the route takes. `status`,
/// `assign` and the rest key the flight as `flight`; `edit` keys it as
/// `target`, since an edit's target may be a comment.
export type Write =
  | { verb: "status"; body: { flight: string; status: string; message?: string } }
  | { verb: "assign"; body: { flight: string; assignee: string } }
  | {
      verb: "edit";
      body: { target: string; priority?: string; labels?: string[]; skill?: string; bay?: string };
    };

/// What setting `field` on `target` to `value` writes, or null when the
/// value is not one the field can take.
///
/// `null` clears the lane — `none` is the wire word, the CLI's own — and
/// clears nothing else: priority, skill and bay have no clearing on the
/// wire, and an empty `labels` means *unchanged* rather than *emptied*, so
/// the last label cannot be removed. Each of those answers null rather
/// than sending a write the route would ignore.
export function write(field: Field, target: string, value: string | string[] | null): Write | null {
  if (field === "label") {
    const labels = Array.isArray(value) ? value : value === null ? [] : [value];
    return labels.length === 0 ? null : { verb: "edit", body: { target, labels } };
  }
  if (Array.isArray(value)) return null;
  switch (field) {
    case "status":
      return value === null ? null : { verb: "status", body: { flight: target, status: value } };
    case "assignee":
      return { verb: "assign", body: { flight: target, assignee: value ?? "none" } };
    case "priority":
      return value === null ? null : { verb: "edit", body: { target, priority: value } };
    case "skill":
      return value === null ? null : { verb: "edit", body: { target, skill: value } };
    case "bay":
      return value === null ? null : { verb: "edit", body: { target, bay: value } };
    default:
      return null;
  }
}
