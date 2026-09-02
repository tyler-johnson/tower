// The codec's cases, ported from query.rs's own by name: the web codec
// writes what the server parses, so the round trips core proves have to
// hold here byte for byte.

import { describe, expect, it } from 'vitest';
import { DEFAULT_SHOW, defaultQuery, parse, render, withColumn, type Query } from './query';

const DAY = 86_400;

describe('the query codec', () => {
	it('a query round trips through its own codec', () => {
		const raw =
			'status=ready,in_progress&priority=high&label=infra&subject=contains:parser' +
			'&filed=after:3d&group=assignee&sub=priority&order=-changed&closed=10d' +
			'&empty=true&mode=board&show=ref,status,assignee,label,age';
		const query = parse(raw);
		expect(query).not.toBeNull();
		if (query === null) return;

		expect(query.filters).toHaveLength(5);
		expect(query.filters[0]).toEqual({
			field: 'status',
			op: 'is',
			value: { words: ['ready', 'in_progress'] }
		});
		expect(query.filters[3]).toEqual({
			field: 'subject',
			op: 'contains',
			value: { text: 'parser' }
		});
		expect(query.filters[4]).toEqual({
			field: 'filed',
			op: 'after',
			value: { when: { ago: 3 * DAY } }
		});
		expect(query.group).toBe('assignee');
		expect(query.subgroup).toBe('priority');
		expect(query.order).toEqual({ field: 'changed', descending: true });
		expect(query.closed).toEqual({ span: 10 * DAY });
		expect(query.emptyGroups).toBe(true);
		expect(query.mode).toBe('board');
		expect(query.show).toEqual(['ref', 'status', 'assignee', 'label', 'age']);

		expect(render(query)).toBe(raw);
		expect(parse(render(query))).toEqual(query);

		// The default is the empty string in both directions.
		expect(render(defaultQuery())).toBe('');
		expect(parse('')).toEqual(defaultQuery());
		expect(parse('?')).toEqual(defaultQuery());

		// A span's spelling normalizes to the largest unit that divides
		// it; the value it stands for is what round-trips.
		const week = parse('closed=7d');
		expect(week).not.toBeNull();
		if (week === null) return;
		expect(render(week)).toBe('closed=1w');
		expect(parse(render(week))).toEqual(week);
	});

	it('a label carrying a comma survives the round trip', () => {
		// Every structural character at once: the separator, the
		// operator's colon, the pair's equals, the param's ampersand,
		// the escape itself, and a space.
		const awkward = 'needs, maybe: a & b = 50% off/on';
		const query: Query = {
			...defaultQuery(),
			filters: [{ field: 'label', op: 'is', value: { words: [awkward, 'plain'] } }]
		};
		const rendered = render(query);
		expect(rendered).not.toContain('needs, maybe');
		expect(parse(rendered)).toEqual(query);
	});

	it('a relative date stays relative', () => {
		const query = parse('filed=after:3d&changed=before:2w');
		expect(query).not.toBeNull();
		if (query === null) return;
		expect(query.filters[0].value).toEqual({ when: { ago: 3 * DAY } });
		expect(query.filters[1].value).toEqual({ when: { ago: 14 * DAY } });
		expect(render(query)).toBe('filed=after:3d&changed=before:2w');

		const epoch = parse('filed=after:@1700000000');
		expect(epoch).not.toBeNull();
		if (epoch === null) return;
		expect(epoch.filters[0].value).toEqual({ when: { at: 1_700_000_000 } });
		expect(render(epoch)).toBe('filed=after:@1700000000');
	});

	it('refusals answer null', () => {
		for (const raw of [
			'stauts=ready',
			'status=contains:x',
			'subject=before:3d',
			'filed=3d',
			'ref=1',
			'group=subject',
			'order=label',
			'show=body',
			'show=for',
			'closed=soon',
			'empty=maybe',
			'mode=grid',
			'status'
		]) {
			expect(parse(raw), raw).toBeNull();
		}
	});

	it('for=me round trips as a filter alone', () => {
		const query = parse('for=me');
		expect(query).not.toBeNull();
		if (query === null) return;
		expect(query.filters).toEqual([{ field: 'for', op: 'is', value: { words: ['me'] } }]);
		expect(render(query)).toBe('for=me');
	});

	it('an unknown value parses', () => {
		// A view saved by a newer tower must stay parseable here: field
		// names refuse, values never do.
		expect(parse('status=parked')).not.toBeNull();
		expect(parse('priority=whenever')).not.toBeNull();
	});

	it('a column turns on where the layout puts it', () => {
		const without = withColumn(DEFAULT_SHOW, 'status', false);
		expect(without).toEqual(['priority', 'ref', 'subject', 'label', 'assignee', 'age']);
		// Back on, it lands in its slot rather than at the end, so the
		// default comes back and the show key leaves the URL.
		expect(withColumn(without, 'status', true)).toEqual(DEFAULT_SHOW);
		expect(withColumn(DEFAULT_SHOW, 'comments', true)).toEqual([...DEFAULT_SHOW, 'comments']);
		// A show the URL holds in some other order is re-sorted by the
		// first toggle.
		expect(withColumn(['age', 'ref'], 'skill', true)).toEqual(['ref', 'age', 'skill']);
		expect(withColumn(DEFAULT_SHOW, 'age', true)).toEqual(DEFAULT_SHOW);
	});
});
