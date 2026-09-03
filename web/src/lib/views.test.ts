// The chip row's three questions of a URL, against the built-ins and a
// custom view.

import { describe, expect, it } from 'vitest';
import { BUILTINS, canonical, isActive, unsaved, type View } from './views';

const mine: View = {
	id: 'pi.1',
	name: 'mine',
	query: 'assignee=me',
	shared: false,
	author: 'a@b.c',
	saved_by: 'a@b.c',
	saved_at: 0
};

const [all, forMe] = BUILTINS;

describe('the views', () => {
	it('a hand-typed default is All Flights', () => {
		expect(canonical('?closed=1d')).toBe('');
		expect(isActive(all, '?closed=1d')).toBe(true);
		expect(isActive(forMe, '?closed=1d')).toBe(false);
		expect(unsaved('?closed=1d', [mine])).toBe(false);
	});

	it("the CLI's three newest is a choice, not the default", () => {
		expect(canonical('closed=3')).toBe('closed=3');
		expect(isActive(all, 'closed=3')).toBe(false);
		expect(unsaved('closed=3', [mine])).toBe(true);
	});

	it('for=me is For Me whatever else is default', () => {
		expect(isActive(forMe, 'for=me&closed=1d')).toBe(true);
		expect(isActive(all, 'for=me&closed=1d')).toBe(false);
		expect(unsaved('for=me&closed=1d', [])).toBe(false);
	});

	it('a custom view holds its query and nothing else', () => {
		expect(isActive(mine, 'assignee=me')).toBe(true);
		expect(isActive(all, 'assignee=me')).toBe(false);
		expect(isActive(forMe, 'assignee=me')).toBe(false);
		expect(unsaved('assignee=me', [mine])).toBe(false);
	});

	it('a refused query is active nowhere and never saveable', () => {
		expect(canonical('bogus=1')).toBeNull();
		expect(isActive(all, 'bogus=1')).toBe(false);
		expect(isActive(mine, 'bogus=1')).toBe(false);
		expect(unsaved('bogus=1', [mine])).toBe(false);
	});

	it('a query no view names is unsaved', () => {
		expect(unsaved('assignee=me&group=assignee', [mine])).toBe(true);
		expect(unsaved('assignee=me&group=assignee', [])).toBe(true);
	});
});
