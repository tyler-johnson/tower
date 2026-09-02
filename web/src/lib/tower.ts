// The wire types and the pure render helpers, ported from
// crates/ff-tower-cli's render.rs and cmd/brief.rs so the two stay
// comparable — one function here per function there, same names in
// camelCase, same order of phrases.

export interface Envelope<T> {
	tower: number;
	cmd: string;
	data?: T;
	error?: TowerError;
}

export interface TowerError {
	id: string;
	message: string;
	/// The `try:` hints the raise site or the registry gave — commands to
	/// run, not an exit code.
	exits: string[];
}

/// A query's answer: `/api/board?<query>` and every feed frame. The
/// groups are keyed on whatever the query grouped by — status under the
/// default query, so the keys are the six status words — and the two
/// counts are disjoint: `hidden` is what the closed window cut, `filtered`
/// what the filters rejected.
export interface Folded {
	groups: Group[];
	hidden: number;
	filtered: number;
}

/// One group of rows, and the groups nested inside it. A group holds
/// rows or subgroups, never both; `null` keys the rows carrying no value
/// for the grouped field, and the single group of an ungrouped fold.
export interface Group {
	key: string | null;
	count: number;
	rows: FlightView[];
	subgroups: Group[];
}

/// The rows of the group keyed `key`, or none when the fold has no such
/// group — an empty status group is dropped from the wire.
export function rowsOf(folded: Folded, key: string | null): FlightView[] {
	return folded.groups.find((group) => group.key === key)?.rows ?? [];
}

/// Every row outside `closed`, flattened: the live board under the
/// default query, where a closed flight is on the record rather than on
/// the board.
export function liveRows(folded: Folded): FlightView[] {
	return folded.groups.filter((group) => group.key !== 'closed').flatMap((group) => group.rows);
}

/// The inbox, derived here the way core's `enrich` derives it: live rows
/// with an open question, oldest ask first, and live rows Ready in the
/// `me` lane — the todo list. A view of the same rows, so a flight here
/// still stands in its status group.
export function inbox(folded: Folded): { questions: FlightView[]; yours: FlightView[] } {
	const live = liveRows(folded);
	const questions = live
		.filter((row) => row.question !== null)
		.sort((a, b) => (a.asked_at ?? 0) - (b.asked_at ?? 0));
	const yours = live.filter(
		(row) => row.question === null && row.status === 'ready' && row.assignee === 'me'
	);
	return { questions, yours };
}

export interface FlightView {
	id: string;
	number: number;
	/// Provenance only: the procedure the filing was minted under, or the
	/// pass routed it under.
	procedure: string | null;
	subject: string;
	filed_by: string;
	filed_at: number;
	comments: number;
	depends_on: string[];
	blocks: string[];
	/// The stored status, verbatim.
	status: string;
	/// Who last moved the status, and when — `null` while the flight still
	/// stands where it was filed.
	status_by: string | null;
	status_at: number | null;
	assignee: string | null;
	priority: string;
	labels: string[];
	skill: string | null;
	branch: string | null;
	tip: string | null;
	/// The freshest session-tagged capture on this flight's branch — the
	/// repository's fact, and nothing the record itself did.
	last_change: number | null;
	/// In Progress, and the branch has not changed for the threshold.
	stale: boolean;
	/// Ready, and the branch changed after the flight was set Ready.
	changed_since_ready: boolean;
	/// Closed children over total, a JSON array — Rust's `(usize, usize)`
	/// serializes as one, not as an object.
	progress: [number, number] | null;
	held: boolean;
	resolving: boolean;
	current: boolean;
	question: string | null;
	asked_at: number | null;
	collides: CollideView[];
	unanswered: string[];
}

export interface CollideView {
	with: string;
	paths: string[];
}

/// The staleness threshold's rendering, `config::DEFAULT_STALE_FLIGHT`
/// through `render::span`. The board envelope carries neither a clock nor
/// the config, so a repo that set `tower.staleFlightThreshold` to
/// something else will read wrong here until the envelope carries it —
/// the flag itself is the server's, and only the word is guessed.
const STALE_AFTER = '2d';

/// The writer half of a wire id — everything before the last `.`.
export function writerOf(id: string): string {
	const dot = id.lastIndexOf('.');
	return dot === -1 ? id : id.slice(0, dot);
}

/// Whether the given full ids span at most one writer, so `#<n>` alone
/// names a flight unambiguously.
export function shortIds(ids: string[]): boolean {
	const writers = ids.map(writerOf);
	return writers.every((writer) => writer === writers[0]);
}

/// The display form of a flight's number: `#3` when short, `pi-8c2e#3`
/// otherwise — the long form takes no leading `#`.
export function flightRef(writer: string, number: number, short: boolean): string {
	return short ? `#${number}` : `${writer}#${number}`;
}

/// The collide path phrase: the one path, or a count.
export function pathsPhrase(paths: string[]): string {
	return paths.length === 1 ? paths[0] : `${paths.length} paths`;
}

/// `4m ago`, `2d ago` — s/m/h/d/w. `now` is an argument so a render is a
/// pure function of its inputs.
export function age(now: number, then: number): string {
	const delta = Math.max(now - then, 0);
	if (delta < 60) return `${delta}s ago`;
	if (delta < 3_600) return `${Math.floor(delta / 60)}m ago`;
	if (delta < 86_400) return `${Math.floor(delta / 3_600)}h ago`;
	if (delta < 604_800) return `${Math.floor(delta / 86_400)}d ago`;
	return `${Math.floor(delta / 604_800)}w ago`;
}

/// The tip column: the branch tip short, `—` for a flight with no tip, and
/// the literal `(detached)` for `@detached` — printing the sentinel as a
/// branch name would read as a real branch.
export function tipColumn(view: FlightView): string {
	if (view.branch === '@detached') return '(detached)';
	return view.tip ? view.tip.slice(0, 8) : '—';
}

/// The priority glyph, urgent first. A word this build has never heard of
/// falls to `·` rather than being given a rung of its own — the same
/// posture `rank()` takes when it sorts an unknown priority last.
export function priorityGlyph(priority: string): string {
	switch (priority) {
		case 'urgent':
			return '!';
		case 'high':
			return '↑';
		case 'medium':
			return '→';
		case 'low':
			return '↓';
		default:
			return '·';
	}
}

/// The status dot's daisyUI modifier. A status this build has never heard
/// of gets the neutral dot: the row still renders, and the word beside it
/// is the honest answer about what the status is.
export function statusDot(status: string): string {
	switch (status) {
		case 'waiting':
			return 'status-warning';
		case 'ready':
			return 'status-info';
		case 'in_progress':
			return 'status-primary';
		case 'held':
			return 'status-error';
		case 'done':
			return 'status-success';
		case 'triage':
		case 'canceled':
		default:
			return 'status-neutral';
	}
}

/// The status as a person reads it — `in_progress` is the only stored
/// word with an underscore in it, and cmd/brief.rs spells it out the same
/// way.
export function statusWord(status: string): string {
	return status.replaceAll('_', ' ');
}

/// The right-aligned age column, on `note()`'s own precedence: the ask if
/// there is one, else the branch's last change, else the filing.
export function ageColumn(view: FlightView, now: number): string {
	if (view.asked_at !== null) return age(now, view.asked_at);
	if (view.last_change !== null) return age(now, view.last_change);
	return age(now, view.filed_at);
}

/// The subject column: the subject, then the progress mark for a flight
/// that has children. On a flat board the mark is the whole of what says
/// a row is a family.
export function subjectColumn(view: FlightView): string {
	if (view.progress === null) return view.subject;
	return `${view.subject} (${view.progress[0]}/${view.progress[1]})`;
}

/// Wire id to display form, over every group at once: a verdict partner
/// is always a live flight, so the map answers for `collides` and
/// `unanswered` entries too. Also the live flight count, for the footer —
/// every row outside `closed`, which on a flat board is every live flight
/// exactly once; a closed flight is on the record rather than on the
/// board.
export function buildRefs(folded: Folded): { refs: Map<string, string>; flights: number } {
	const views = folded.groups.flatMap((group) => group.rows);
	const short = shortIds(views.map((view) => view.id));
	const refs = new Map(
		views.map((view) => [view.id, flightRef(writerOf(view.id), view.number, short)])
	);
	return { refs, flights: liveRows(folded).length };
}

export interface NotePhrase {
	text: string;
	tone: 'warn' | 'dim';
}

/// The note line's phrases, in render.rs's urgency order: question, held,
/// resolving, collides, no-verdicts, the two audits, the pilot, branch,
/// comments.
///
/// The one deliberate divergence from `note()`: the trailing age phrase is
/// omitted, because the web row has a column for the age and the CLI has
/// no room for one. `ageColumn` is that phrase's other half.
export function notePhrases(view: FlightView, refs: Map<string, string>): NotePhrase[] {
	const phrases: NotePhrase[] = [];
	const warn = (text: string) => phrases.push({ text, tone: 'warn' });
	const dim = (text: string) => phrases.push({ text, tone: 'dim' });
	if (view.question !== null) warn(view.question);
	if (view.held) warn('held');
	if (view.resolving) warn('resolving');
	for (const collide of view.collides) {
		warn(`collides ${show(refs, collide.with)} on ${pathsPhrase(collide.paths)}`);
	}
	for (const with_ of view.unanswered) {
		dim(`no verdict vs ${show(refs, with_)}`);
	}
	// The two audits, each its own phrase and neither under a shared word:
	// one says the branch has forgotten a flight that claims to be flying,
	// the other says a branch moved under one that claims not to be.
	if (view.stale) warn(`no changes on the branch for ${STALE_AFTER}`);
	if (view.changed_since_ready) warn('changes on the branch since it was set ready');
	// The pilot, ahead of the branch: the stored In Progress and who set
	// it — the byline is the pilot, the field is the chip.
	if (view.status === 'in_progress') {
		dim(view.status_by !== null ? `in progress — ${view.status_by}` : 'in progress');
	}
	if (view.branch !== null && view.branch !== '@detached') dim(`on ${view.branch}`);
	if (view.comments > 0) {
		dim(`${view.comments} ${view.comments === 1 ? 'comment' : 'comments'}`);
	}
	return phrases;
}

/// Where one flight stands, `Standing`'s tag — flattened onto the brief
/// beside the facts it arbitrates, so the variant's own keys (`on`,
/// `with`, `paths`) sit at the top level too.
export type StandingTag =
	| 'done'
	| 'question'
	| 'held'
	| 'in-progress'
	| 'yours'
	| 'ready'
	| 'waiting'
	| 'collides'
	| 'no-verdict';

/// One linked flight, as the brief carries it. `status` is the stored
/// word; `closed` is the arbitrated fact, since done and canceled are two
/// words for one end.
export interface LinkView {
	flight: string;
	subject: string;
	status: string;
	closed: boolean;
}

/// A note on the record. `id` is the wire id — a comment's only name, and
/// what `edit` takes.
export interface CommentView {
	id: string;
	author: string;
	at: number;
	text: string;
}

/// One gesture on the record: who did what, when, and the words the verb
/// took, flat beside `what` and present only where the kind carries them.
/// Deliberately thin — the subject, the body, the question, and a comment's
/// text already sit elsewhere on the brief.
export interface Moment {
	id: string;
	at: number;
	by: string;
	what: string;
	/// `status`: the word used, verbatim; `reason` a cancel's `-m`.
	status?: string;
	reason?: string;
	/// `assigned`: the lane; `null` is the clearing.
	assignee?: string | null;
	/// `edited`: the fields touched; `comment` the comment's event id when
	/// the target was a comment rather than the flight.
	fields?: string[];
	comment?: string;
	/// `linked` and `unlinked`: both ends, wire ids, `from` depends on `to`.
	from?: string;
	to?: string;
	/// `routed`: which procedure and rule fired, and why.
	procedure?: string;
	rule?: string;
	because?: string;
}

/// The words after a moment's verb — a leading space and the words, or
/// `''` when the kind carries none — and the free text that follows on its
/// own line: a move's reason, a routing's because. Link endpoints print as
/// wire ids: the panel has no number map.
export function momentPhrase(moment: Moment, briefId: string): { line: string; note?: string } {
	switch (moment.what) {
		case 'status':
			return moment.status === undefined
				? { line: '' }
				: { line: ` ${moment.status}`, note: moment.reason };
		case 'assigned':
			return moment.assignee === undefined
				? { line: '' }
				: { line: ` ${moment.assignee ?? 'none'}` };
		case 'edited':
			if (moment.comment !== undefined) return { line: ` comment ${moment.comment}` };
			return moment.fields === undefined ? { line: '' } : { line: ` ${moment.fields.join(', ')}` };
		case 'linked':
		case 'unlinked':
			if (moment.from === undefined || moment.to === undefined) return { line: '' };
			return moment.from === briefId
				? { line: ` depends on ${moment.to}` }
				: { line: ` blocks ${moment.from}` };
		case 'routed':
			return moment.procedure === undefined
				? { line: '' }
				: { line: ` ${moment.procedure}`, note: moment.because || undefined };
		default:
			return { line: '' };
	}
}

/// One examined-and-skipped flight from `next`'s walk, `Skip` flattened
/// under `reason`.
export interface Passed {
	flight: string;
	reason: 'waiting' | 'collides' | 'no-verdict';
	on?: string[];
	with?: string;
	paths?: string[];
}

export interface Brief {
	id: string;
	number: number;
	procedure: string | null;
	subject: string;
	body: string;
	filed_by: string;
	filed_at: number;
	/// The stored fields, read here because the brief is the read surface
	/// for one flight.
	status: string;
	status_by: string | null;
	status_at: number | null;
	assignee: string | null;
	priority: string;
	labels: string[];
	skill: string | null;
	bay: string | null;
	/// The last edit touching the record — the flight's own fields or a
	/// comment's text — flat like the status mark.
	edited_by: string | null;
	edited_at: number | null;
	question: string | null;
	asked_by: string | null;
	asked_at: number | null;
	branch: string | null;
	tip: string | null;
	held: boolean;
	resolving: boolean;
	current: boolean;
	last_change: number | null;
	stale: boolean;
	changed_since_ready: boolean;
	progress: [number, number] | null;
	depends_on: LinkView[];
	blocks: LinkView[];
	comments: CommentView[];
	history: Moment[];
	standing: StandingTag;
	on?: string[];
	with?: string;
	paths?: string[];
	beat: Passed[];
}

/// One worktree in the pool.
export interface BayView {
	id: string;
	path: string;
	branch: string | null;
	flight: string | null;
	subject: string | null;
	current: boolean;
}

export interface Pool {
	bays: BayView[];
}

/// One procedure as the registry holds it, mirroring
/// ff-tower-core/src/procedure/mod.rs. This is the *definition* — a file
/// on disk, and nothing a flight carries: a filing keeps only the
/// procedure's name, as provenance.
export interface Definition {
	name: string;
	/// What the flight's subject resolves against later; `branch` on
	/// `review`. Nothing derives from it yet.
	subject: string | null;
	matches: ProcedureMatch[];
	flights: FlightDef[];
	source: Source;
}

/// One intake rule: a name — what the routing event records as having
/// fired — and the predicates, which all AND; an absent predicate is
/// null. Named ProcedureMatch to avoid the DOM's Match.
export interface ProcedureMatch {
	name: string;
	source: string | null;
	event: string | null;
	label: string | null;
	priority: string | null;
	skill: string | null;
	assignee: string | null;
}

/// One flight a definition declares. `done` stays a free string: a newer
/// tower's completion word must not fail an older tower's parse.
export interface FlightDef {
	id: string;
	assignee: 'me' | 'agent';
	skill: string | null;
	after: string[];
	done: string;
	bay: string | null;
	/// The priority and labels the flight is born with — free here because
	/// they are free on the flight.
	priority: string | null;
	labels: string[];
}

/// Which layer a definition was read from, and the file it came from.
/// Both layers are directories, so every definition has a path.
export interface Source {
	layer: 'user' | 'repo';
	path: string;
}

export interface Listing {
	procedures: Definition[];
}

/// A flight's display form, or its wire id when the board has no entry —
/// a linked or beat flight outside the closed window has left the board,
/// and its wire id is still a name that resolves.
function show(refs: Map<string, string>, id: string): string {
	return refs.get(id) ?? id;
}

/// The brief's note line, ported from cmd/brief.rs's `note()`: the status
/// ahead of everything, because a reader must know first where the flight
/// stands and who put it there, then the question, the holds, the
/// standing, the audits, the branch, and the age. Precedence makes the
/// standing exclusive with the mark phrases, so the line never says a
/// thing twice.
export function briefNote(brief: Brief, refs: Map<string, string>, now: number): NotePhrase[] {
	const phrases: NotePhrase[] = [];
	const warn = (text: string) => phrases.push({ text, tone: 'warn' });
	const dim = (text: string) => phrases.push({ text, tone: 'dim' });
	const status = statusWord(brief.status);
	dim(
		brief.status_by !== null && brief.status_at !== null
			? `${status} — ${brief.status_by} ${age(now, brief.status_at)}`
			: status
	);
	if (brief.question !== null) warn(brief.question);
	if (brief.held) warn('held');
	if (brief.resolving) warn('resolving');
	switch (brief.standing) {
		// Said above, from the brief's own flat facts.
		case 'done':
		case 'question':
		case 'held':
		case 'in-progress':
			break;
		case 'yours':
			dim(brief.assignee !== null ? `yours — assigned ${brief.assignee}` : 'yours — unassigned');
			break;
		case 'ready':
			dim('ready');
			break;
		case 'waiting':
			dim(`waiting on ${(brief.on ?? []).map((dep) => show(refs, dep)).join(', ')}`);
			break;
		case 'collides':
			warn(
				`collides with ${show(refs, brief.with ?? '')} on ${pathsPhrase(brief.paths ?? [])}`
			);
			break;
		case 'no-verdict':
			warn(`no verdict vs ${show(refs, brief.with ?? '')}`);
			break;
	}
	if (brief.stale) warn(`no changes on the branch for ${STALE_AFTER}`);
	if (brief.changed_since_ready) warn('changes on the branch since it was set ready');
	if (brief.branch === '@detached') dim('(detached)');
	else if (brief.branch !== null) {
		dim(`on ${brief.branch}${brief.tip !== null ? ` ${brief.tip.slice(0, 8)}` : ''}`);
	}
	if (brief.asked_at !== null) dim(`asked ${age(now, brief.asked_at)}`);
	else if (brief.last_change !== null) dim(`changed ${age(now, brief.last_change)}`);
	else dim(`filed ${age(now, brief.filed_at)}`);
	return phrases;
}

/// The stored fields, one line, ported from cmd/brief.rs's `fields_line()`:
/// lane, priority, labels, skill, bay, and the procedure the filing was
/// minted under. Its own line rather than phrases in the note — the note
/// is urgency ordered, and a field is not urgency.
export function fieldsLine(brief: Brief): string {
	const phrases = [brief.assignee !== null ? `assignee ${brief.assignee}` : 'unassigned'];
	if (brief.priority !== 'none') phrases.push(`priority ${brief.priority}`);
	if (brief.labels.length > 0) phrases.push(brief.labels.join(', '));
	if (brief.skill !== null) phrases.push(`skill ${brief.skill}`);
	if (brief.bay !== null) phrases.push(`bay ${brief.bay}`);
	if (brief.procedure !== null) phrases.push(`under ${brief.procedure}`);
	return phrases.join(' · ');
}

/// One beat row — a candidate this flight kept out of `next`'s walk.
/// Waiting rows name dependencies rather than competitors, so they never
/// reach `beat`.
export function beatLine(beaten: Passed, refs: Map<string, string>): string {
	const reason =
		beaten.reason === 'collides'
			? `collides on ${pathsPhrase(beaten.paths ?? [])}`
			: 'no verdict';
	return `beat ${show(refs, beaten.flight)} · ${reason}`;
}

/// A refusal as lines, in main.rs's `report()` shape minus the
/// `ff-tower:` prefix — a terminal artifact, and this is not a terminal.
export function refusalLines(error: TowerError): string[] {
	const lines = [error.message];
	if (error.exits.length > 0) {
		lines.push('  try:');
		for (const hint of error.exits) lines.push(`    ${hint}`);
	}
	return lines;
}

export type Verb = 'assign' | 'status' | 'hold' | 'answer' | 'done' | 'cancel' | 'comment';

/// The verbs this flight's state accepts, from the guards in
/// ff-tower-core/src/verb/.
///
/// A closed flight is what `ensure_active` refuses on, so it keeps only
/// `comment` — a note on a closed record is fine, and comment.rs runs no
/// `ensure_active` for exactly that reason. An open question closes two
/// more: `status` refuses with `status/held` for any target but done or
/// canceled, and `hold` refuses with `hold/exists`. `assign` re-lanes a
/// held flight freely, which is how a question gets handed to someone.
///
/// Derived from a fold that may be a frame stale, so this decides what to
/// offer and never what is allowed: the server's refusal is still the
/// word that counts.
export function allowedVerbs(brief: Brief): Verb[] {
	if (brief.status === 'done' || brief.status === 'canceled') return ['comment'];
	if (brief.question !== null) return ['assign', 'answer', 'done', 'cancel', 'comment'];
	return ['assign', 'status', 'hold', 'done', 'cancel', 'comment'];
}

/// Every top-level key this build does not know, as labelled rows.
///
/// A newer tower's brief carries fields this page has never heard of, and
/// showing them badly beats dropping them silently — the same promise
/// `Kind::Unknown` makes the fold. `standing`'s own variant keys are
/// known, not unknown: `Standing` is flattened onto the payload, so `on`,
/// `with` and `paths` arrive at the top level too.
const KNOWN_BRIEF_KEYS = new Set([
	'id',
	'number',
	'procedure',
	'subject',
	'body',
	'filed_by',
	'filed_at',
	'status',
	'status_by',
	'status_at',
	'assignee',
	'priority',
	'labels',
	'skill',
	'bay',
	'edited_by',
	'edited_at',
	'question',
	'asked_by',
	'asked_at',
	'branch',
	'tip',
	'held',
	'resolving',
	'current',
	'last_change',
	'stale',
	'changed_since_ready',
	'progress',
	'depends_on',
	'blocks',
	'comments',
	'history',
	'standing',
	'on',
	'with',
	'paths',
	'beat'
]);

export function unknownRows(brief: Brief): { label: string; value: string }[] {
	return Object.entries(brief as unknown as Record<string, unknown>)
		.filter(([key]) => !KNOWN_BRIEF_KEYS.has(key))
		.map(([label, value]) => ({
			label,
			// A scalar reads as itself; anything else is shown as the JSON
			// it arrived as, which is at least honest about its shape.
			value:
				value === null || typeof value !== 'object' ? String(value) : JSON.stringify(value)
		}));
}
