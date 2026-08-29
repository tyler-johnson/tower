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

export interface Board {
	waiting_on_you: FlightView[];
	in_the_air: FlightView[];
	holding: FlightView[];
	open: FlightView[];
	unrouted: TowerEvent[];
}

export interface FlightView {
	id: string;
	number: number;
	procedure: string;
	part: PartStamp | null;
	subject: string;
	filed_by: string;
	filed_at: number;
	comments: number;
	depends_on: string[];
	blocks: string[];
	branch: string | null;
	tip: string | null;
	last_motion: number | null;
	held: boolean;
	resolving: boolean;
	current: boolean;
	claimed_by: string | null;
	taken: boolean;
	requeued_at: number | null;
	question: string | null;
	asked_at: number | null;
	collides: CollideView[];
	unanswered: string[];
}

export interface CollideView {
	with: string;
	paths: string[];
}

export interface PartStamp {
	id: string;
	crew: string;
	skill?: string;
	/// A free string on the wire, not a flag: a newer tower's completion
	/// word must not fail an older tower's parse.
	done: string;
	bay?: string;
	branch?: string;
}

// The unrouted rows are raw log events; the board renders only their count,
// so the shape stays loose. Named TowerEvent to avoid the DOM's Event.
export interface TowerEvent {
	writer: string;
	author: string;
	time: number;
	id: string;
	kind: unknown;
}

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

/// Wire id to display form, over every section at once: a verdict partner
/// is always a live flight, so the map answers for `collides` and
/// `unanswered` entries too. Also the total flight count, for the footer.
export function buildRefs(board: Board): { refs: Map<string, string>; flights: number } {
	const views = [board.waiting_on_you, board.in_the_air, board.holding, board.open].flat();
	const short = shortIds(views.map((view) => view.id));
	const refs = new Map(
		views.map((view) => [view.id, flightRef(writerOf(view.id), view.number, short)])
	);
	return { refs, flights: views.length };
}

export interface NotePhrase {
	text: string;
	tone: 'warn' | 'dim';
}

/// The note line's phrases, in render.rs's urgency order: question, held,
/// resolving, collides, no-verdicts, waiting-on, ownership, branch,
/// comments, age. The age phrase means the line is never empty.
export function notePhrases(
	view: FlightView,
	refs: Map<string, string>,
	now: number
): NotePhrase[] {
	const phrases: NotePhrase[] = [];
	const warn = (text: string) => phrases.push({ text, tone: 'warn' });
	const dim = (text: string) => phrases.push({ text, tone: 'dim' });
	if (view.question !== null) warn(view.question);
	if (view.held) warn('held');
	if (view.resolving) warn('resolving');
	for (const collide of view.collides) {
		warn(`collides ${refs.get(collide.with)} on ${pathsPhrase(collide.paths)}`);
	}
	for (const with_ of view.unanswered) {
		dim(`no verdict vs ${refs.get(with_)}`);
	}
	// A dependency absent from `refs` is done — done flights leave the
	// board — so the phrase covers only the live ones and clears itself as
	// they land.
	const waiting = view.depends_on.filter((dep) => refs.has(dep));
	if (waiting.length === 1) dim(`waiting on ${refs.get(waiting[0])}`);
	else if (waiting.length > 1) dim(`waiting on ${waiting.length} flights`);
	// Ownership, ahead of the branch. `taken` prints whether or not a
	// branch exists; a bare `claimed` needs no branch; `requeued` stands
	// where neither does.
	if (view.taken) dim('taken');
	else if (view.claimed_by !== null) {
		if (view.branch === null) dim('claimed');
	} else if (view.requeued_at !== null) dim('requeued');
	if (view.branch !== null && view.branch !== '@detached') dim(`on ${view.branch}`);
	if (view.comments > 0) {
		dim(`${view.comments} ${view.comments === 1 ? 'comment' : 'comments'}`);
	}
	if (view.asked_at !== null) dim(`asked ${age(now, view.asked_at)}`);
	else if (view.last_motion !== null) dim(`moved ${age(now, view.last_motion)}`);
	else dim(`filed ${age(now, view.filed_at)}`);
	return phrases;
}

/// Where one flight stands, `Standing`'s tag — flattened onto the brief
/// beside the facts it arbitrates, so the variant's own keys (`on`,
/// `with`, `paths`) sit at the top level too.
export type StandingTag =
	| 'done'
	| 'question'
	| 'held'
	| 'claimed'
	| 'yours'
	| 'ready'
	| 'waiting'
	| 'collides'
	| 'no-verdict';

/// One linked flight, as the brief carries it.
export interface LinkView {
	flight: string;
	subject: string;
	done: boolean;
}

/// A note on the record. `id` is the wire id — a comment's only name, and
/// what `edit` takes.
export interface CommentView {
	id: string;
	author: string;
	at: number;
	text: string;
}

/// One gesture on the record: who did what, when. Deliberately thin — the
/// words behind a gesture already sit elsewhere on the brief.
export interface Moment {
	id: string;
	at: number;
	by: string;
	what: string;
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
	procedure: string;
	part: PartStamp | null;
	subject: string;
	body: string;
	filed_by: string;
	filed_at: number;
	claimed_by: string | null;
	claimed_at: number | null;
	taken_by: string | null;
	taken_at: number | null;
	requeued_at: number | null;
	routed_by: string | null;
	routed_at: number | null;
	because: string | null;
	edited_by: string | null;
	edited_at: number | null;
	question: string | null;
	asked_by: string | null;
	asked_at: number | null;
	done_by: string | null;
	done_at: number | null;
	branch: string | null;
	tip: string | null;
	held: boolean;
	resolving: boolean;
	current: boolean;
	last_motion: number | null;
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

/// A flight's display form, or its wire id when the board has no entry —
/// a linked or beat flight that is done has left the board, and its wire
/// id is still a name that resolves.
function show(refs: Map<string, string>, id: string): string {
	return refs.get(id) ?? id;
}

/// The brief's note line, ported from cmd/brief.rs's `note()`: the done
/// mark ahead of everything, because a reader must know first that the
/// flight is over, then the question, the holds, the ownership, the
/// standing, the branch, and the age. Precedence makes the standing
/// exclusive with the mark phrases, so the line never says a thing twice.
export function briefNote(brief: Brief, refs: Map<string, string>, now: number): NotePhrase[] {
	const phrases: NotePhrase[] = [];
	const warn = (text: string) => phrases.push({ text, tone: 'warn' });
	const dim = (text: string) => phrases.push({ text, tone: 'dim' });
	if (brief.done_by !== null && brief.done_at !== null) {
		dim(`done by ${brief.done_by} ${age(now, brief.done_at)}`);
	}
	if (brief.question !== null) warn(brief.question);
	if (brief.held) warn('held');
	if (brief.resolving) warn('resolving');
	// A take is a claim with a provenance: same owner, different gesture,
	// and the word is what says the agent lane is closed.
	if (brief.claimed_by !== null) {
		dim(`${brief.taken_by !== null ? 'taken' : 'claimed'} by ${brief.claimed_by}`);
	}
	switch (brief.standing) {
		// Said above, from the brief's own flat facts.
		case 'done':
		case 'question':
		case 'held':
		case 'claimed':
			break;
		case 'yours':
			dim(brief.part ? `yours — crewed ${brief.part.crew}` : 'yours — no part stamp');
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
	if (brief.branch === '@detached') dim('(detached)');
	else if (brief.branch !== null) {
		dim(`on ${brief.branch}${brief.tip !== null ? ` ${brief.tip.slice(0, 8)}` : ''}`);
	}
	if (brief.asked_at !== null) dim(`asked ${age(now, brief.asked_at)}`);
	else if (brief.last_motion !== null) dim(`moved ${age(now, brief.last_motion)}`);
	else dim(`filed ${age(now, brief.filed_at)}`);
	return phrases;
}

/// What part of its procedure this flight is, ported from cmd/brief.rs's
/// `part_line()`. Its own line rather than a phrase in the note: the note
/// is urgency ordered, and crew is not urgency.
export function partLine(part: PartStamp): string {
	const phrases = [`part ${part.id}`, part.crew];
	if (part.skill) phrases.push(`skill ${part.skill}`);
	if (part.bay) phrases.push(`bay ${part.bay}`);
	if (part.branch) phrases.push(`branch ${part.branch}`);
	phrases.push(`done ${part.done}`);
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

export type Verb = 'claim' | 'take' | 'requeue' | 'hold' | 'answer' | 'done' | 'comment';

/// The verbs this flight's state accepts, from the guards in
/// ff-tower-core/src/verb/. `done` is what `ensure_active` refuses on, so
/// a finished flight keeps only `comment` — a note on a closed record is
/// fine, and comment.rs runs no `ensure_active` for exactly that reason.
///
/// Derived from a fold that may be a frame stale, so this decides what to
/// offer and never what is allowed: the server's refusal is still the
/// word that counts.
export function allowedVerbs(brief: Brief): Verb[] {
	if (brief.done_at !== null) return ['comment'];
	const verbs: Verb[] = [];
	if (brief.claimed_by === null) verbs.push('claim');
	if (brief.taken_by === null) verbs.push('take');
	if (brief.claimed_by !== null || brief.taken_by !== null) verbs.push('requeue');
	if (brief.question === null) verbs.push('hold');
	else verbs.push('answer');
	verbs.push('done');
	verbs.push('comment');
	return verbs;
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
	'part',
	'subject',
	'body',
	'filed_by',
	'filed_at',
	'claimed_by',
	'claimed_at',
	'taken_by',
	'taken_at',
	'requeued_at',
	'routed_by',
	'routed_at',
	'because',
	'edited_by',
	'edited_at',
	'question',
	'asked_by',
	'asked_at',
	'done_by',
	'done_at',
	'branch',
	'tip',
	'held',
	'resolving',
	'current',
	'last_motion',
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
