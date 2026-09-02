// The value picker's list: what a words-shaped field takes on a set of
// rows, with a count beside each. Pure, so the counts are a function of
// the rows handed in — the board in hand for a new chip, a probe with
// the chip removed for an edit — and the picker never asks for its own.

import type { Field } from './query';
import { section, type FlightView } from './tower';

export interface Facet {
	value: string;
	count: number;
}

const SECTIONS = ['triage', 'waiting', 'ready', 'in_progress', 'held', 'done', 'canceled'];
const PRIORITIES = ['urgent', 'high', 'medium', 'low', 'none'];
const FLAGS = ['true', 'false'];

/// The values a words-shaped field takes on these rows, with counts.
/// `status` counts by section and lists the seven status words in
/// lifecycle order, zeros kept;
/// `priority` lists its five in rank order, zeros kept; the three flags
/// list `true` then `false`; `for` offers `me` alone, zero kept; every
/// other field lists what the rows
/// carry, alphabetically, and nothing for a row with no value. An absent
/// value has no filter — core's `one()` never matches `None` — so there
/// is no "none" row to offer.
export function facets(field: Field, rows: FlightView[]): Facet[] {
	const counts = new Map<string, number>();
	const hit = (value: string | null) => {
		if (value !== null) counts.set(value, (counts.get(value) ?? 0) + 1);
	};
	for (const row of rows) {
		switch (field) {
			case 'status':
				hit(section(row.status));
				break;
			case 'priority':
				hit(row.priority);
				break;
			case 'assignee':
				hit(row.assignee);
				break;
			case 'skill':
				hit(row.skill);
				break;
			case 'bay':
				hit(row.bay);
				break;
			case 'procedure':
				hit(row.procedure);
				break;
			case 'branch':
				hit(row.branch);
				break;
			case 'label':
				for (const label of row.labels) hit(label);
				break;
			case 'stale':
				hit(String(row.stale));
				break;
			case 'changed_since_ready':
				hit(String(row.changed_since_ready));
				break;
			case 'held':
				hit(String(row.held));
				break;
			case 'for':
				// The rows only a person can handle: an open question in
				// any lane, or the `me` lane at any status.
				if (row.question !== null || row.assignee === 'me') hit('me');
				break;
			default:
				return [];
		}
	}
	const listed = (values: string[]) =>
		values.map((value) => ({ value, count: counts.get(value) ?? 0 }));
	switch (field) {
		case 'status':
			return listed(SECTIONS);
		case 'priority':
			return listed(PRIORITIES);
		case 'stale':
		case 'changed_since_ready':
		case 'held':
			return listed(FLAGS);
		case 'for':
			return listed(['me']);
		default:
			return listed([...counts.keys()].sort());
	}
}
