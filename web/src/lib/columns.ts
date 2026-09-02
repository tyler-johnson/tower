// The cell vocabulary: what one showable field draws for one row. The
// list lays its rows out from the query's `show`, and the kanban card
// and the flight page draw from the same cells, so a column is a fact
// with a kind and the row decides how each kind looks — one renderer per
// kind, never per field.
//
// No runes, so it tests under vitest with no shims.

import type { Field } from './query';
import {
	age,
	ageColumn,
	priorityGlyph,
	subjectColumn,
	type FlightView
} from './tower';

/// What one column shows for one row. The row draws each kind one way,
/// so a column is a fact and not markup.
export type Cell =
	| { kind: 'glyph'; text: string; title: string }
	| { kind: 'ref'; text: string }
	| { kind: 'dot'; status: string }
	| { kind: 'subject'; text: string }
	| { kind: 'chips'; words: string[] }
	| { kind: 'dim'; text: string }
	| { kind: 'flag'; text: string; on: boolean };

/// The cell a field draws for a row. `refs` is the board's id map, `now`
/// the clock every age reads against, so a cell is a pure function of
/// its inputs.
export function cell(field: Field, view: FlightView, refs: Map<string, string>, now: number): Cell {
	switch (field) {
		case 'priority':
			return {
				kind: 'glyph',
				text: priorityGlyph(view.priority),
				title: `priority ${view.priority}`
			};
		case 'ref':
			return { kind: 'ref', text: refs.get(view.id) ?? '' };
		case 'status':
			return { kind: 'dot', status: view.status };
		case 'subject':
			return { kind: 'subject', text: subjectColumn(view) };
		case 'label':
			return { kind: 'chips', words: view.labels };
		case 'assignee':
			return { kind: 'chips', words: view.assignee !== null ? [view.assignee] : [] };
		case 'age':
			return { kind: 'dim', text: ageColumn(view, now) };
		case 'skill':
			return { kind: 'dim', text: view.skill ?? '' };
		case 'bay':
			return { kind: 'dim', text: view.bay ?? '' };
		case 'procedure':
			return { kind: 'dim', text: view.procedure ?? '' };
		case 'branch':
			// The sentinel printed as a branch name would read as a real
			// branch, the same rule `tipColumn` keeps.
			return { kind: 'dim', text: view.branch === '@detached' ? '(detached)' : (view.branch ?? '') };
		case 'filed':
			return { kind: 'dim', text: age(now, view.filed_at) };
		case 'moved':
			return { kind: 'dim', text: view.status_at !== null ? age(now, view.status_at) : '' };
		case 'changed':
			return { kind: 'dim', text: view.last_change !== null ? age(now, view.last_change) : '' };
		case 'comments':
			return { kind: 'dim', text: view.comments > 0 ? String(view.comments) : '' };
		case 'progress':
			return {
				kind: 'dim',
				text: view.progress !== null ? `${view.progress[0]}/${view.progress[1]}` : ''
			};
		case 'stale':
			return { kind: 'flag', text: 'stale', on: view.stale };
		case 'changed_since_ready':
			return { kind: 'flag', text: 'changed', on: view.changed_since_ready };
		case 'held':
			return { kind: 'flag', text: 'held', on: view.held };
		case 'body':
		case 'for':
			// Not showable, and the switch needs a leg.
			return { kind: 'dim', text: '' };
	}
}

/// The section's `grid-template-columns` for these columns: a glyph and
/// a dot are one character, the subject takes the slack, and everything
/// else sizes to its content. Without a subject a trailing `1fr` takes
/// the slack instead, so the row still spans the section and the hover
/// ground reaches the edge.
export function template(show: Field[]): string {
	const tracks: string[] = show.map((field) => {
		switch (field) {
			case 'priority':
			case 'status':
				return '1ch';
			case 'subject':
				return 'minmax(0,1fr)';
			default:
				return 'max-content';
		}
	});
	if (!show.includes('subject')) tracks.push('1fr');
	return tracks.join(' ');
}

/// The 1-based grid column the note line starts at: the subject's, or 1.
export function noteStart(show: Field[]): number {
	return show.indexOf('subject') + 1 || 1;
}
