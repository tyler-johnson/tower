// The query codec, ported from crates/ff-tower-core/src/board/query.rs
// so the two stay comparable — one function here per function there,
// same names in camelCase. All seven axes and not only the filters: the
// display menu edits the other six against this same parser, and a
// second one must not grow.
//
// The server stays the parser of record. `parse` answers `null` wherever
// core refuses and never words a refusal of its own — the shell's alert
// already carries core's message for a query the URL holds and the server
// refused. What this side has to get exactly right is the other
// direction: a URL `render` writes is what the server parses, so every
// rule here matches core's byte for byte.
//
// No runes and no `$app` imports, so it tests under vitest with no
// shims.

import { statusWord } from './tower';

export type Field =
	| 'status'
	| 'assignee'
	| 'priority'
	| 'label'
	| 'skill'
	| 'bay'
	| 'subject'
	| 'body'
	| 'procedure'
	| 'branch'
	| 'filed'
	| 'moved'
	| 'changed'
	| 'stale'
	| 'changed_since_ready'
	| 'held'
	| 'for'
	| 'ref'
	| 'age'
	| 'comments'
	| 'progress';

/// Every field, in the order the refusals list them.
export const FIELDS: Field[] = [
	'status',
	'assignee',
	'priority',
	'label',
	'skill',
	'bay',
	'subject',
	'body',
	'procedure',
	'branch',
	'filed',
	'moved',
	'changed',
	'stale',
	'changed_since_ready',
	'held',
	'for',
	'ref',
	'age',
	'comments',
	'progress'
];

/// The five operators, spelled as the wire spells them (`Op::name`):
/// `not` is core's `IsNot`.
export type Op = 'is' | 'not' | 'contains' | 'before' | 'after';

const OPS: Op[] = ['is', 'not', 'contains', 'before', 'after'];

/// A moment, either relative to the caller's clock or absolute. A saved
/// view filed `after:3d` means the last three days every day it is
/// opened, which is why `ago` is kept as seconds and never resolved.
export type When = { ago: number } | { at: number };

/// What a filter compares against, in the shape its operator takes.
export type Value = { words: string[] } | { text: string } | { when: When };

/// One predicate. Filters AND together.
export interface Filter {
	field: Field;
	op: Op;
	value: Value;
}

/// How rows sort inside a group.
export interface Order {
	field: Field;
	descending: boolean;
}

export type Mode = 'list' | 'board';

/// How much of the closed record the fold carries.
export type ClosedWindow = 'all' | 'none' | { count: number } | { span: number };

export interface Query {
	filters: Filter[];
	group: Field | null;
	subgroup: Field | null;
	order: Order;
	/// The web's default is the past day, not core's three newest: the
	/// two surfaces differ on purpose, see `defaultQuery`.
	closed: ClosedWindow;
	emptyGroups: boolean;
	mode: Mode;
	show: Field[];
}

/// The columns today's row renders, which is what a query that names
/// none asks for.
export const DEFAULT_SHOW: Field[] = [
	'priority',
	'ref',
	'status',
	'subject',
	'label',
	'assignee',
	'age'
];

/// `Query::default`: today's board — grouped by status, ordered priority
/// then age, the past day of closed flights, empty groups dropped, list
/// mode, and the columns a row renders now. The closed window is where
/// the web parts from core: the CLI keeps the three newest, a count that
/// holds its size on a quiet Monday, and the web opens on the past day.
/// The fork is by surface and on purpose.
export function defaultQuery(): Query {
	return {
		filters: [],
		group: 'status',
		subgroup: null,
		order: { field: 'priority', descending: false },
		closed: { span: 86_400 },
		emptyGroups: false,
		mode: 'list',
		show: [...DEFAULT_SHOW]
	};
}

/// What a field compares as, which is what decides the operators it
/// takes and the shape of the value beside them.
export type Shape = 'words' | 'text' | 'time' | 'column';

export function shape(field: Field): Shape {
	switch (field) {
		case 'status':
		case 'assignee':
		case 'priority':
		case 'label':
		case 'skill':
		case 'bay':
		case 'procedure':
		case 'branch':
		case 'stale':
		case 'changed_since_ready':
		case 'held':
		case 'for':
			return 'words';
		case 'subject':
		case 'body':
			return 'text';
		case 'filed':
		case 'moved':
		case 'changed':
			return 'time';
		case 'ref':
		case 'age':
		case 'comments':
		case 'progress':
			return 'column';
	}
}

/// Whether a filter can be written against it.
export function filterable(field: Field): boolean {
	return shape(field) !== 'column';
}

/// The six a board can be grouped into columns by.
export function groupable(field: Field): boolean {
	switch (field) {
		case 'status':
		case 'assignee':
		case 'priority':
		case 'label':
		case 'skill':
		case 'bay':
			return true;
		default:
			return false;
	}
}

/// The axes rows sort along. Every one is a stored fact with a total
/// order; a set-valued field like `label` has none, so it is absent.
export function orderable(field: Field): boolean {
	switch (field) {
		case 'status':
		case 'assignee':
		case 'priority':
		case 'subject':
		case 'filed':
		case 'moved':
		case 'changed':
			return true;
		default:
			return false;
	}
}

/// Whether a row can carry it as a column. Everything but the body and
/// `for`, a predicate over two facts with no cell of its own.
export function showable(field: Field): boolean {
	return field !== 'body' && field !== 'for';
}

/// The columns in the order a row lays them out: the default seven in
/// their order, then every other showable field in FIELDS order. The
/// chips draw in this order, and a column turned on lands where this
/// list puts it rather than at the end, so turning the seven on again
/// gives the default back.
export const COLUMNS: Field[] = [
	...DEFAULT_SHOW,
	...FIELDS.filter((field) => showable(field) && !DEFAULT_SHOW.includes(field))
];

/// `show` with `field` on or off, kept in COLUMNS order.
export function withColumn(show: Field[], field: Field, on: boolean): Field[] {
	const rest = show.filter((column) => column !== field);
	if (!on) return rest;
	return [...rest, field].sort((a, b) => COLUMNS.indexOf(a) - COLUMNS.indexOf(b));
}

export function accepts(field: Field, op: Op): boolean {
	switch (shape(field)) {
		case 'words':
			return op === 'is' || op === 'not';
		case 'text':
			return op === 'contains';
		case 'time':
			return op === 'before' || op === 'after';
		case 'column':
			return false;
	}
}

/// The operator an unprefixed value means. A time has none — a moment on
/// its own says nothing about which side of it to keep.
export function defaultOp(field: Field): Op | null {
	switch (shape(field)) {
		case 'words':
			return 'is';
		case 'text':
			return 'contains';
		case 'time':
		case 'column':
			return null;
	}
}

/// The operators a field takes, in the order a menu lists them.
export function operators(field: Field): Op[] {
	switch (shape(field)) {
		case 'words':
			return ['is', 'not'];
		case 'text':
			return ['contains'];
		case 'time':
			return ['after', 'before'];
		case 'column':
			return [];
	}
}

/// The wire name back to the field; `null` for a word that names no
/// axis. Unlike a value, a field name is closed and refuses.
function fieldFromName(name: string): Field | null {
	return FIELDS.find((field) => field === name) ?? null;
}

function opFromName(name: string): Op | null {
	return OPS.find((op) => op === name) ?? null;
}

// ---- the codec ------------------------------------------------------

/// `Query::parse`. An empty string is the default, and a leading `?` is
/// tolerated so a browser's own `location.search` can be handed over
/// whole. Any key that is not one of the seven axes is a field name.
export function parse(raw: string): Query | null {
	const query = defaultQuery();
	raw = raw.trim();
	if (raw.startsWith('?')) raw = raw.slice(1);
	for (const part of raw.split('&')) {
		if (part === '') continue;
		const eq = part.indexOf('=');
		if (eq === -1) return null;
		const key = decode(part.slice(0, eq));
		const value = part.slice(eq + 1);
		switch (key) {
			case 'group': {
				const group = grouping(value);
				if (group === undefined) return null;
				query.group = group;
				break;
			}
			case 'sub': {
				const sub = grouping(value);
				if (sub === undefined) return null;
				query.subgroup = sub;
				break;
			}
			case 'order': {
				const order = ordering(value);
				if (order === null) return null;
				query.order = order;
				break;
			}
			case 'closed': {
				const closed = parseClosed(decode(value));
				if (closed === null) return null;
				query.closed = closed;
				break;
			}
			case 'empty': {
				const empty = flag(decode(value));
				if (empty === null) return null;
				query.emptyGroups = empty;
				break;
			}
			case 'mode': {
				const mode = parseMode(decode(value));
				if (mode === null) return null;
				query.mode = mode;
				break;
			}
			case 'show': {
				const show = columns(value);
				if (show === null) return null;
				query.show = show;
				break;
			}
			default: {
				const parsed = filter(key, value);
				if (parsed === null) return null;
				query.filters.push(parsed);
			}
		}
	}
	return query;
}

/// `Query::render`. Only what differs from the default is spelled, so
/// the default renders to the empty string and a filtered board's link
/// stays hand-editable. Values are percent-encoded against the
/// structural characters, so a label carrying any of them round-trips.
export function render(query: Query): string {
	const base = defaultQuery();
	const parts = query.filters.map(renderFilter);
	if (query.group !== base.group) parts.push(`group=${query.group ?? ''}`);
	if (query.subgroup !== base.subgroup) parts.push(`sub=${query.subgroup ?? ''}`);
	if (query.order.field !== base.order.field || query.order.descending !== base.order.descending) {
		parts.push(`order=${query.order.descending ? '-' : ''}${query.order.field}`);
	}
	if (!sameWindow(query.closed, base.closed)) parts.push(`closed=${renderWindow(query.closed)}`);
	if (query.emptyGroups !== base.emptyGroups) parts.push(`empty=${query.emptyGroups}`);
	if (query.mode !== base.mode) parts.push(`mode=${query.mode}`);
	if (!sameList(query.show, base.show)) parts.push(`show=${query.show.join(',')}`);
	return parts.join('&');
}

/// The search as the wire has to carry it. Serve's default closed window
/// is the CLI's three newest, and `render` elides the web's own default,
/// so a URL silent on `closed` would fold under the CLI's window unless
/// the wire says `closed=1d` itself. A search that already names a
/// window is returned as it is.
export function withDefaultWindow(search: string): string {
	if (search.split('&').some((part) => part.startsWith('closed='))) return search;
	return search === '' ? 'closed=1d' : `${search}&closed=1d`;
}

function sameList(a: Field[], b: Field[]): boolean {
	return a.length === b.length && a.every((field, i) => field === b[i]);
}

function sameWindow(a: ClosedWindow, b: ClosedWindow): boolean {
	if (typeof a === 'string' || typeof b === 'string') return a === b;
	if ('count' in a) return 'count' in b && a.count === b.count;
	return 'span' in b && a.span === b.span;
}

/// A grouping axis: an empty value is ungrouped (`null`), anything else
/// a field that has to be groupable. `undefined` is the refusal.
function grouping(value: string): Field | null | undefined {
	const text = decode(value);
	if (text === '') return null;
	const field = fieldFromName(text);
	if (field === null || !groupable(field)) return undefined;
	return field;
}

/// An ordering: a field name, optionally `-`-prefixed for descending.
function ordering(value: string): Order | null {
	const text = decode(value);
	const descending = text.startsWith('-');
	const field = fieldFromName(descending ? text.slice(1) : text);
	if (field === null || !orderable(field)) return null;
	return { field, descending };
}

/// The column list. An empty value is no columns at all.
function columns(value: string): Field[] | null {
	const show: Field[] = [];
	for (const piece of value.split(',')) {
		if (piece === '') continue;
		const field = fieldFromName(decode(piece));
		if (field === null || !showable(field)) return null;
		show.push(field);
	}
	return show;
}

function flag(text: string): boolean | null {
	if (text === 'true') return true;
	if (text === 'false') return false;
	return null;
}

function parseMode(text: string): Mode | null {
	if (text === 'list' || text === 'board') return text;
	return null;
}

/// One filter, from its param key and its still-encoded value. The
/// operator prefix is split before anything is decoded, so a value
/// carrying a literal `:` — which the encoder writes as `%3A` — is never
/// mistaken for one. A prefix that names no operator refuses.
function filter(key: string, value: string): Filter | null {
	const field = fieldFromName(key);
	if (field === null || !filterable(field)) return null;
	let op: Op | null;
	let rest: string;
	const colon = value.indexOf(':');
	if (colon !== -1) {
		op = opFromName(value.slice(0, colon));
		rest = value.slice(colon + 1);
	} else {
		op = defaultOp(field);
		rest = value;
	}
	if (op === null || !accepts(field, op)) return null;
	switch (op) {
		case 'is':
		case 'not':
			return {
				field,
				op,
				value: { words: rest.split(',').filter((piece) => piece !== '').map(decode) }
			};
		case 'contains':
			return { field, op, value: { text: decode(rest) } };
		case 'before':
		case 'after': {
			const when = parseMoment(decode(rest));
			if (when === null) return null;
			return { field, op, value: { when } };
		}
	}
}

/// A moment: `@<epoch>` absolute, anything else the duration grammar,
/// read as seconds before now.
export function parseMoment(text: string): When | null {
	if (text.startsWith('@')) {
		const digits = text.slice(1);
		if (!/^[+-]?\d+$/.test(digits)) return null;
		return { at: Number(digits) };
	}
	const ago = parseDuration(text);
	return ago === null ? null : { ago };
}

/// `config::parse_duration`: digits with a trailing `s`, `m`, `h`, `d`,
/// or `w`; bare digits are days.
export function parseDuration(text: string): number | null {
	if (text === '') return null;
	const last = text[text.length - 1];
	const units: Record<string, number> = { s: 1, m: 60, h: 3_600, d: 86_400, w: 604_800 };
	const unit = last in units ? last : 'd';
	const digits = last in units ? text.slice(0, -1) : text;
	if (!/^\+?\d+$/.test(digits)) return null;
	return Number(digits) * units[unit];
}

function renderFilter(filter: Filter): string {
	const name = filter.field;
	const value = filter.value;
	if ('words' in value) {
		// `is` is the unprefixed form, which is what makes the common
		// filter read as `status=ready`.
		const body = value.words.map(encode).join(',');
		return filter.op === 'is' ? `${name}=${body}` : `${name}=${filter.op}:${body}`;
	}
	if ('text' in value) return `${name}=${filter.op}:${encode(value.text)}`;
	return `${name}=${filter.op}:${renderMoment(value.when)}`;
}

function renderMoment(when: When): string {
	return 'at' in when ? `@${when.at}` : renderSpan(when.ago);
}

/// Seconds back to the largest unit that divides them exactly, so a
/// relative filter stays words rather than becoming a count of seconds.
/// The spelling normalizes — `7d` comes back `1w` — and the value it
/// stands for does not.
export function renderSpan(secs: number): string {
	const units: [string, number][] = [
		['w', 604_800],
		['d', 86_400],
		['h', 3_600],
		['m', 60]
	];
	for (const [unit, size] of units) {
		if (secs !== 0 && secs % size === 0) return `${secs / size}${unit}`;
	}
	return `${secs}s`;
}

/// `model::parse_closed`: `true` or `all` for everything, `false` or
/// `none` for nothing, a suffixed duration for a span, a bare integer
/// for a count.
export function parseClosed(text: string): ClosedWindow | null {
	text = text.trim();
	switch (text.toLowerCase()) {
		case 'true':
		case 'all':
			return 'all';
		case 'false':
		case 'none':
			return 'none';
	}
	if (/[smhdw]$/.test(text)) {
		const span = parseDuration(text);
		return span === null ? null : { span };
	}
	if (!/^\+?\d+$/.test(text)) return null;
	return { count: Number(text) };
}

export function renderWindow(closed: ClosedWindow): string {
	if (typeof closed === 'string') return closed;
	if ('count' in closed) return `${closed.count}`;
	return renderSpan(closed.span);
}

/// The closed window presets the select offers: the rendered spelling
/// each stands for, and its words. A window compares through its
/// spelling, so a URL's `closed=7d` matches `1w` here.
export const WINDOWS: { value: string; label: string }[] = [
	{ value: 'none', label: 'none' },
	{ value: '3', label: 'newest 3' },
	{ value: '10', label: 'newest 10' },
	{ value: '1d', label: 'past day' },
	{ value: '1w', label: 'past week' },
	{ value: '4w', label: 'past four weeks' },
	{ value: 'all', label: 'all' }
];

/// The unreserved set, plus `/` because branch names are full of it and
/// a query string is where it is legal unescaped. Everything else is
/// percent-encoded over the UTF-8 bytes.
export function encode(raw: string): string {
	let out = '';
	for (const byte of new TextEncoder().encode(raw)) {
		if (
			(byte >= 0x41 && byte <= 0x5a) ||
			(byte >= 0x61 && byte <= 0x7a) ||
			(byte >= 0x30 && byte <= 0x39) ||
			byte === 0x2d ||
			byte === 0x5f ||
			byte === 0x2e ||
			byte === 0x7e ||
			byte === 0x2f
		) {
			out += String.fromCharCode(byte);
		} else {
			out += `%${byte.toString(16).toUpperCase().padStart(2, '0')}`;
		}
	}
	return out;
}

/// The other half. `+` decodes to a space too, and a `%` that begins no
/// escape is carried through as itself rather than refused: a value is
/// open. Lossy at the end, like `from_utf8_lossy`.
export function decode(raw: string): string {
	const bytes = new TextEncoder().encode(raw);
	const out: number[] = [];
	let at = 0;
	while (at < bytes.length) {
		const byte = bytes[at];
		if (byte === 0x2b) {
			out.push(0x20);
			at += 1;
		} else if (byte === 0x25 && at + 2 < bytes.length) {
			const high = nibble(bytes[at + 1]);
			const low = nibble(bytes[at + 2]);
			if (high !== null && low !== null) {
				out.push((high << 4) | low);
				at += 3;
			} else {
				out.push(0x25);
				at += 1;
			}
		} else {
			out.push(byte);
			at += 1;
		}
	}
	return new TextDecoder().decode(new Uint8Array(out));
}

function nibble(byte: number): number | null {
	if (byte >= 0x30 && byte <= 0x39) return byte - 0x30;
	if (byte >= 0x61 && byte <= 0x66) return byte - 0x61 + 10;
	if (byte >= 0x41 && byte <= 0x46) return byte - 0x41 + 10;
	return null;
}

// ---- the words a chip reads ------------------------------------------

/// A field as a person reads it: `changed_since_ready` prints as
/// `changed since ready`, through the same underscore rule a status
/// takes.
export function fieldLabel(field: Field): string {
	return statusWord(field);
}

/// The operator's word on a chip. `is` and `not` take a set, so a set of
/// several reads as `any of` or `none of`.
export function opLabel(filter: Filter): string {
	const several = 'words' in filter.value && filter.value.words.length > 1;
	switch (filter.op) {
		case 'is':
			return several ? 'is any of' : 'is';
		case 'not':
			return several ? 'is none of' : 'is not';
		case 'contains':
			return 'contains';
		case 'before':
			return 'before';
		case 'after':
			return 'after';
	}
}

/// The value's word on a chip: the words joined, the text verbatim, a
/// span as an age, an epoch as a date.
export function valueLabel(filter: Filter): string {
	const value = filter.value;
	if ('words' in value) {
		const words = filter.field === 'status' ? value.words.map(statusWord) : value.words;
		return words.join(', ');
	}
	if ('text' in value) return value.text;
	if ('at' in value.when) return new Date(value.when.at * 1000).toLocaleDateString();
	return `${renderSpan(value.when.ago)} ago`;
}
