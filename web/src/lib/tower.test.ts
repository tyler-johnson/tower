// The fold's counting: a flight dealt into several groups, or nested
// under a subgroup, is still one flight to the ref map and the footer.

import { describe, expect, it } from 'vitest';
import { buildRefs, foldRows, liveRows, type FlightView, type Folded, type Group } from './tower';

function flight(number: number, status: string, labels: string[] = []): FlightView {
	return {
		id: `pi-8c2e.${number}`,
		number,
		procedure: null,
		subject: `flight ${number}`,
		body: '',
		filed_by: 'tyler',
		filed_at: 0,
		comments: 0,
		depends_on: [],
		blocks: [],
		status,
		status_by: null,
		status_at: null,
		assignee: null,
		priority: 'none',
		labels,
		skill: null,
		bay: null,
		branch: null,
		tip: null,
		last_change: null,
		stale: false,
		changed_since_ready: false,
		progress: null,
		held: false,
		resolving: false,
		current: false,
		question: null,
		asked_at: null,
		collides: [],
		unanswered: []
	};
}

function group(key: string | null, rows: FlightView[], subgroups: Group[] = []): Group {
	return { key, count: rows.length, rows, subgroups };
}

describe('the fold', () => {
	it('a fold is counted once however it is grouped', () => {
		const both = flight(1, 'ready', ['web', 'ui']);
		const one = flight(2, 'in_progress', ['web']);
		const done = flight(3, 'done', ['ui']);
		const canceled = flight(4, 'canceled', ['web']);
		const byLabel: Folded = {
			groups: [group('web', [both, one, canceled]), group('ui', [both, done])],
			hidden: 0,
			filtered: 0
		};
		expect(foldRows(byLabel)).toHaveLength(4);
		expect(liveRows(byLabel)).toHaveLength(2);
		expect(buildRefs(byLabel).flights).toBe(2);
		expect(buildRefs(byLabel).refs.get(done.id)).toBe('#3');

		const nested: Folded = {
			groups: [
				group('tyler', [], [group('high', [both]), group('none', [one])]),
				group(null, [], [group('none', [done, canceled])])
			],
			hidden: 0,
			filtered: 0
		};
		expect(foldRows(nested)).toHaveLength(4);
		expect(liveRows(nested)).toHaveLength(2);
		expect(buildRefs(nested).flights).toBe(2);
		expect(buildRefs(nested).refs.size).toBe(4);
	});
});
