// The two lists read as one: the history's walk, each moment given its
// words, and a comment the history did not name still at its own time.

import { describe, expect, it } from 'vitest';
import { stream } from './stream';
import type { Brief, CommentView, Moment } from './tower';

function brief(comments: CommentView[], history: Moment[]): Brief {
	return {
		id: 'pi-8c2e.1',
		number: 1,
		procedure: null,
		subject: 'the work',
		body: '',
		filed_by: 'tyler',
		filed_at: 0,
		status: 'ready',
		status_by: null,
		status_at: null,
		status_reason: null,
		assignee: null,
		priority: 'none',
		labels: [],
		skill: null,
		bay: null,
		edited_by: null,
		edited_at: null,
		question: null,
		asked_by: null,
		asked_at: null,
		branch: null,
		tip: null,
		held: false,
		resolving: false,
		current: false,
		last_change: null,
		stale: false,
		changed_since_ready: false,
		progress: null,
		depends_on: [],
		blocks: [],
		comments,
		history,
		standing: 'ready',
		beat: []
	};
}

describe('the stream', () => {
	it('a comment carries its text and a gesture its phrase, in the log order', () => {
		const rows = stream(
			brief(
				[{ id: 'pi-8c2e.2', author: 'tyler', at: 2, text: 'a note' }],
				[
					{ id: 'pi-8c2e.1', at: 1, by: 'tyler', what: 'filed' },
					{ id: 'pi-8c2e.2', at: 2, by: 'tyler', what: 'commented' },
					{ id: 'pi-8c2e.3', at: 3, by: 'tyler', what: 'status', status: 'ready' }
				]
			)
		);
		expect(rows.map((row) => row.kind)).toEqual(['gesture', 'comment', 'gesture']);
		expect(rows[1]).toEqual({
			kind: 'comment',
			id: 'pi-8c2e.2',
			at: 2,
			by: 'tyler',
			text: 'a note'
		});
		expect(rows[2]).toEqual({
			kind: 'gesture',
			id: 'pi-8c2e.3',
			at: 3,
			by: 'tyler',
			what: 'status',
			line: ' ready',
			note: null
		});
	});

	it('a hold and an answer carry their own words', () => {
		const rows = stream(
			brief(
				[],
				[
					{ id: 'pi-8c2e.2', at: 2, by: 'tyler', what: 'held', question: 'which log?' },
					{ id: 'pi-8c2e.3', at: 3, by: 'tyler', what: 'answered', answer: "the writer's own" }
				]
			)
		);
		expect(rows.map((row) => row.kind === 'gesture' && row.note)).toEqual([
			'which log?',
			"the writer's own"
		]);
	});

	it('a comment the history did not name still lands at its own time', () => {
		const rows = stream(
			brief(
				[{ id: 'pi-8c2e.2', author: 'agent', at: 2, text: 'from another log' }],
				[
					{ id: 'pi-8c2e.1', at: 1, by: 'tyler', what: 'filed' },
					{ id: 'pi-8c2e.3', at: 3, by: 'tyler', what: 'done' }
				]
			)
		);
		expect(rows.map((row) => row.id)).toEqual(['pi-8c2e.1', 'pi-8c2e.2', 'pi-8c2e.3']);
		expect(rows[1].kind).toBe('comment');
	});
});
